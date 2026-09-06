import shutil

import tensorflow as tf
import tensorflow_models as tfm
from proteus.config.pydantic_models import ModelConfigSingular

from .s3_tools import upload_directory_to_s3


class ModelConstructor:
    def __init__(
        self,
        model_config: ModelConfigSingular,
    ) -> None:
        self.model_configuration = model_config
        self.data_structure_configuration = model_config.data_structure_config

    def save_model_to_s3(self, best_model, model_name, save_dir) -> None:
        """Saves the model to an S3 bucket.

        Args:
        ----
            best_model (object): The best model to be saved.
            model_name (str): The name of the model.
            save_dir (str): The directory where the model will be saved.

        Returns:
        -------
            None
        """
        model_save_dir = save_dir / "model_save"
        model_save_dir.mkdir(parents=True, exist_ok=True)
        best_model.save(str(model_save_dir))
        upload_directory_to_s3(model_save_dir, model_name, "tf_model")
        shutil.rmtree(model_save_dir)

    def order_data_structures(self):
        """Sorts the data_structure_configuration dictionary based on the input_order values of its items.

        Returns
        -------
            dict: A new dictionary containing the items of the data_structure_configuration dictionary, sorted by input_order.

        """
        return dict(
            sorted(
                self.data_structure_configuration.items(),
                key=lambda item: item[1]["input_order"],
            ),
        )

    def build_model_single_row_relational(
        self,
        structure_name: str,
    ) -> tuple[tf.Tensor, tf.Tensor]:
        """Builds a single row relational model for the given structure name.

        Args:
        ----
            structure_name (str): The name of the structure.

        Returns:
        -------
            tuple[tf.Tensor, tf.Tensor]: A tuple containing the input tensor and the output tensor of the model.
        """
        input_shape = self.data_structure_configuration[structure_name]["data_shape"]
        inputs = tf.keras.Input(
            shape=(input_shape[1],),
            name=f"inputs_{structure_name}",
        )
        linear_layer_1 = tf.keras.layers.Dense(
            units=512,
            activation="tanh",
            name=f"Projector_Dense_{structure_name}_1",
        )(inputs)
        norm_layer = tf.keras.layers.BatchNormalization(
            name=f"Projector_BNorm_{structure_name}",
        )(linear_layer_1)
        dropout_layer = tf.keras.layers.Dropout(
            rate=0.33,
            name=f"Projector_Dropout_{structure_name}",
        )(norm_layer)
        linear_layer_2 = tf.keras.layers.Dense(
            units=512,
            activation="tanh",
            name=f"Projector_Dense_{structure_name}_2",
        )(dropout_layer)

        return inputs, linear_layer_2

    def singular_extra_long_range_encoder_block(
        self,
        input_tensor: tf.Tensor,
        structure_name: str,
        n_layers: int = 1,
        current_n: int = 1,
        n_units: int = 128,
        num_heads: int = 3,
        dropout_rate: float = 0.1,
    ) -> tf.Tensor:
        attention_layer = tf.keras.layers.MultiHeadAttention(
            num_heads=num_heads,
            key_dim=input_tensor.shape[2],
            dropout=dropout_rate,
            name=f"Encoder_MultiAttention_{current_n}_{structure_name}",
        )(input_tensor, input_tensor)

        combined_input_attention_layer = tf.keras.layers.Lambda(
            lambda x: x[0] + x[1],
            name=f"Encoder_Resid_1_{current_n}_{structure_name}",
        )([input_tensor, attention_layer])

        norm_layer_1 = tf.keras.layers.LayerNormalization(
            name=f"Encoder_LNorm_1_{current_n}_{structure_name}",
        )(combined_input_attention_layer)

        drop_out_layer_1 = tf.keras.layers.Dropout(
            rate=dropout_rate,
            name=f"Encoder_Dropout_1_{current_n}_{structure_name}",
        )(norm_layer_1)

        linear_layer_1 = tf.keras.layers.Dense(
            units=n_units,
            activation="relu",
            name=f"Encoder_Dense_1_{current_n}_{structure_name}",
        )(drop_out_layer_1)

        norm_layer_2 = tf.keras.layers.LayerNormalization(
            name=f"Encoder_LNorm_2_{current_n}_{structure_name}",
        )(linear_layer_1)

        drop_out_layer_2 = tf.keras.layers.Dropout(
            rate=dropout_rate,
            name=f"Encoder_Dropout_2_{current_n}_{structure_name}",
        )(norm_layer_2)

        combined_output_layer = tf.keras.layers.Add(
            name=f"Encoder_Resid_2_{current_n}_{structure_name}",
        )([drop_out_layer_1, drop_out_layer_2])

        norm_layer_3 = tf.keras.layers.LayerNormalization(
            name=f"Encoder_LNorm_3_{current_n}_{structure_name}",
        )(combined_output_layer)

        current_n += 1
        n_layers -= 1
        if n_layers > 0:
            return self.singular_extra_long_range_encoder_block(
                input_tensor=norm_layer_3,
                structure_name=structure_name,
                n_layers=n_layers,
                current_n=current_n,
            )
        return norm_layer_3

    def build_model_attention_layer(self, structure_name: str):
        sequence_length = self.data_structure_configuration[structure_name][
            "data_shape"
        ][1]
        total_features = self.data_structure_configuration[structure_name][
            "total_features"
        ]

        input_tensor = tf.keras.Input(
            shape=(sequence_length, total_features),
            name=f"inputs_{structure_name}",
        )

        linear_layer = tf.keras.layers.Dense(
            units=128,
            activation="relu",
            name=f"Projector_Dense_{structure_name}",
        )(input_tensor)

        norm_layer = tf.keras.layers.BatchNormalization(
            name=f"Projector_BNorm_{structure_name}",
        )(
            linear_layer,
        )

        drop_out_layer = tf.keras.layers.Dropout(
            rate=0.2,
            name=f"Projector_Dropout_{structure_name}",
        )(norm_layer)

        embeddings = tfm.nlp.layers.PositionEmbedding(max_length=100)(drop_out_layer)

        attention_preprocessing_layer = tf.keras.layers.Add(
            name=f"Preprocess_CombinePosEncode_{structure_name}",
        )([drop_out_layer, embeddings])

        outputs = self.singular_extra_long_range_encoder_block(
            input_tensor=attention_preprocessing_layer,
            n_layers=1,
            n_units=128,
            num_heads=3,
            structure_name=structure_name,
        )

        re_shape_outputs = tf.keras.layers.Flatten(
            name=f"Reshape_outputs_{structure_name}",
        )(outputs)

        re_shape_outputs = tf.keras.layers.Dense(
            units=128,
            activation="relu",
            name=f"pre_output_{structure_name}",
        )(re_shape_outputs)

        return input_tensor, re_shape_outputs

    def return_output_layer(self, previous_tf_layer: tf.Tensor) -> tf.Tensor:
        """Returns the output layer for the model.

        Args:
        ----
            previous_tf_layer: The previous TensorFlow layer that serves as input for the output layer.

        Returns:
        -------
            The output layer for the model.

        Raises:
        ------
            ValueError: If the target type is not one of the valid options: regression, binary_classifier, or multi_classifier.
        """
        activation_functions = {
            "regression": "linear",
            "binary_classifier": "sigmoid",
            "multi_classifier": "softmax",
        }
        activation = activation_functions.get(self.model_configuration.model_type, None)

        if activation is None:
            raise ValueError(
                "Only valid options for target type are: regression, binary_classifier, and multi_classifier",
            )

        output_terminate = tf.keras.layers.Dense(
            units=self.model_configuration.number_of_classes,
            activation=activation,
            name="Output",
        )(previous_tf_layer)

        return output_terminate

    def return_loss_function(self) -> tf.keras.losses.Loss:
        """Returns the loss function based on the target type specified in the model configuration.

        :return: A tf.keras.losses.Loss object representing the loss function.
        :raises ValueError: If the target type specified in the model configuration is not valid.
        """
        loss_functions = {
            "regression": tf.keras.losses.MeanSquaredError,
            "binary_classifier": tf.keras.losses.BinaryCrossentropy,
            "multi_classifier": tf.keras.losses.SparseCategoricalCrossentropy,
        }

        loss_function = loss_functions.get(self.model_configuration.model_type, None)

        if loss_function is None:
            raise ValueError(
                "Only valid options for target type are: regression, binary_classifier, and multi_classifier",
            )

        return loss_function()

    def return_optimizer(self, learning_rate: float):
        return tf.keras.optimizers.Nadam(learning_rate=learning_rate)

    def return_metrics(self) -> list:
        """Returns a list of metrics based on the target type specified in the model configuration.

        :return: A list of metrics.
        :rtype: list
        """
        metric_mapping = {
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
        metrics = metric_mapping.get(self.model_configuration.model_type, None)

        if metrics is None:
            raise ValueError(
                "Only valid options for target type are: regression, binary_classifier, and multi_classifier",
            )

        return metrics

    def construct_model(self, hp=None):
        order_blocks = self.order_data_structures()

        if hp is not None:
            learning_rate = hp.Float(
                "lr",
                min_value=1e-5,
                max_value=1e-2,
                sampling="log",
            )
        else:
            learning_rate = 1e-4

        model_input_list = []
        model_list = []

        for i in order_blocks:
            structure_type = self.data_structure_configuration[i]["data_structure_type"]
            if structure_type == "single_row_relational":
                model_input, model = self.build_model_single_row_relational(
                    structure_name=i,
                )
            elif structure_type == "event_as_array":
                model_input, model = self.build_model_attention_layer(structure_name=i)

            model_input_list.append(model_input)
            model_list.append(model)

        if len(model_input_list) == 1 or len(model_list) == 1:
            model_input_list = model_input_list[0]
            join_blocks_model = model_list[0]
        else:
            join_blocks_model = tf.keras.layers.concatenate(model_list)

        projection_1 = tf.keras.layers.Dense(
            units=256,
            activation="tanh",
            name="Projector_Dense_Major_1",
        )(join_blocks_model)
        projection_2 = tf.keras.layers.Dense(
            units=128,
            activation="tanh",
            name="Projector_Dense_Major_2",
        )(projection_1)

        output_layer = self.return_output_layer(projection_2)

        ultimate_model = tf.keras.Model(inputs=model_input_list, outputs=output_layer)

        ultimate_model.compile(
            loss=self.return_loss_function(),
            optimizer=self.return_optimizer(learning_rate),
            metrics=self.return_metrics(),
        )

        return ultimate_model
