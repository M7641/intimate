import shutil
from pathlib import Path

import numpy as np
import pandas as pd
import tensorflow as tf
from metis.utility_tools.model_tools import (return_class_weights,
                                             return_number_of_classes,
                                             return_response_variable_type)

import keras_tuner as kt  # isort: skip


class FeedForwardConstructor:
    def fit(self, data: dict, target: pd.DataFrame):
        """
        Trains a neural network model using the given data.

        Returns:
            model: The trained neural network model.
        """
        self.output_type = return_response_variable_type(target)
        self.number_of_classes = (
            return_number_of_classes(target)
            if self.output_type in ["multi_classifier"]
            else None
        )
        self.input_features = data["data_to_use_train"].shape[1]

        class_weights = (
            return_class_weights(target)
            if self.output_type in ["binary_classifier", "multi_classifier"]
            else None
        )

        objective = [("val_loss", "min"), ("loss", "min")]
        training_callbacks = [
            tf.keras.callbacks.EarlyStopping(
                monitor=objective[0][0],
                patience=4,
            ),
            tf.keras.callbacks.LearningRateScheduler(self.scheduler),
        ]

        training_data = np.asarray(data["data_to_use_train"]).astype("float32")
        training_target = np.asarray(data["target_train"]).astype("float32")
        test_data = np.asarray(data["data_to_use_test"]).astype("float32")
        test_target = np.asarray(data["target_test"]).astype("float32")

        tuner_file_name = Path(__file__).parent / "saved_model/"
        tuner = kt.BayesianOptimization(
            hypermodel=self.build,
            objective=kt.Objective(*(objective[0])),
            max_trials=3,
            project_name=tuner_file_name,
        )

        tuner.search(
            training_data,
            training_target,
            batch_size=64,
            epochs=30,
            validation_data=(test_data, test_target),
            class_weight=class_weights,
            callbacks=training_callbacks,
            verbose=0,
        )

        best_model = self.build(
            tuner.get_best_hyperparameters()[0],
        )

        best_model.fit(
            training_data,
            training_target,
            validation_split=0.2,
            batch_size=64,
            epochs=30,
            verbose=2,
            class_weight=class_weights,
            callbacks=training_callbacks,
        )

        if tuner_file_name.is_dir():
            shutil.rmtree(str(tuner_file_name))

        return best_model

    def build(self, hp=None):
        """
        Builds and compiles a TensorFlow model for regression, classification, or multi-class classification.

        Returns:
            model (tf.keras.Model): The compiled TensorFlow model.

        Raises:
            ValueError: If the `output_type` is not one of the valid options: "regression", "classifier", or "multi_classifier".
        """

        if hp is not None:
            learning_rate = hp.Float(
                "lr",
                min_value=1e-5,
                max_value=1e-2,
                sampling="log",
            )
        else:
            learning_rate = 1e-4

        if isinstance(self.input_features, int) is True:
            self.input_features = (self.input_features,)

        inputs = tf.keras.layers.Input(shape=self.input_features)

        x = tf.keras.layers.Dense(
            units=256,
            activation="tanh",
            name="Projector_Dense_1",
        )(inputs)
        x = tf.keras.layers.BatchNormalization(name="Projector_BNorm_1")(x)
        x = tf.keras.layers.Dropout(rate=0.33, name="Projector_Dropout_1")(x)
        x = tf.keras.layers.Dense(
            units=256,
            activation="tanh",
            name="Projector_Dense_2",
        )(x)
        x = tf.keras.layers.BatchNormalization(name="Projector_BNorm_2")(x)
        x = tf.keras.layers.Dropout(rate=0.33, name="Projector_Dropout_2")(x)
        x = tf.keras.layers.Dense(
            units=128,
            activation="tanh",
            name="Projector_Dense_3",
        )(x)

        output_layer = self.return_output_layer(x)

        model = tf.keras.Model(inputs=inputs, outputs=output_layer)

        model.compile(
            loss=self.return_loss_function(),
            optimizer=self.return_optimizer(learning_rate=learning_rate),
            metrics=self.return_model_metrics(),
        )
        return model

    def return_loss_function(self):
        """
        Returns the loss function based on the output type specified.

        Parameters:
            - self: The instance of the class.

        Returns:
            The loss function based on the output type specified.

        Raises:
            ValueError: If the output type is not one of the valid options.
        """
        loss_functions = {
            "regression": tf.keras.losses.MeanSquaredError(),
            "binary_classifier": tf.keras.losses.BinaryCrossentropy(),
            "multi_classifier": tf.keras.losses.SparseCategoricalCrossentropy(),
        }

        loss_function = loss_functions.get(self.output_type)
        if loss_function is None:
            msg = "Only valid options for target type are: regression, classifier, and multi_classifier"
            raise ValueError(
                msg,
            )

        return loss_function

    @staticmethod
    def return_optimizer(learning_rate: float):
        """
        Create and return a Nadam optimizer with the specified learning rate.

        Parameters:
            learning_rate (float): The learning rate for the optimizer.

        Returns:
            tf.keras.optimizers.Nadam: The Nadam optimizer with the specified learning rate.
        """
        return tf.keras.optimizers.Nadam(learning_rate=learning_rate)

    def return_model_metrics(self) -> list:
        """
        Return the model metrics for the given output type.

        :return: A list of model metrics.
        :rtype: list
        """
        metrics_mapping = {
            "regression": [
                tf.keras.metrics.MeanSquaredError(name="mse"),
                tf.keras.metrics.RootMeanSquaredError(name="root_mean_squared_error"),
                tf.keras.metrics.MeanAbsoluteError(name="mean_absolute_error"),
            ],
            "binary_classifier": [
                tf.keras.metrics.BinaryAccuracy(name="accuracy"),
                tf.keras.metrics.Precision(name="precision"),
                tf.keras.metrics.Recall(name="recall"),
                tf.keras.metrics.AUC(name="auc"),
            ],
            "multi_classifier": [
                tf.keras.metrics.SparseCategoricalAccuracy(name="accuracy"),
                tf.keras.metrics.SparseCategoricalCrossentropy(name="cross_entropy"),
            ],
        }

        return metrics_mapping.get(self.output_type, [])

    @staticmethod
    def scheduler(epoch, lr):
        if epoch < 5:
            return lr
        else:
            return lr * tf.math.exp(-0.1)

    def return_output_layer(self, previous_tf_layer: tf.Tensor) -> tf.Tensor:
        """
        Returns the output layer for the model.

        Args:
            previous_tf_layer: The previous TensorFlow layer that serves as input for the output layer.

        Returns:
            The output layer for the model.

        Raises:
            ValueError: If the target type is not one of the valid options: regression, binary_classifier, or multi_classifier.
        """
        activation_functions = {
            "regression": "linear",
            "binary_classifier": "sigmoid",
            "multi_classifier": "softmax",
        }
        activation = activation_functions.get(self.output_type, None)

        if activation is None:
            raise ValueError(
                "Only valid options for target type are: regression, binary_classifier, and multi_classifier"
            )

        output_terminate = tf.keras.layers.Dense(
            units=(
                self.number_of_classes if self.output_type == "multi_classifier" else 1
            ),
            activation=activation,
            name="Output",
        )(previous_tf_layer)

        return output_terminate
