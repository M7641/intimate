import tensorflow as tf
from pydantic import BaseModel, Field

from .loss import loss_mean_absolute_error


class LSTMConfig(BaseModel):
    id_column: str = Field(default="unique_id")
    target_column: str = Field(default="y")
    static_features_columns: list[str]
    dynamic_feature_columns: list[str]
    column_indices: dict[str, int]
    columns_cardinality: dict[str, int]


class LSTM(tf.keras.Model):
    """Other ideas:
    - Adding rolling average features
    - Adding lagged features
    """

    def __init__(self, lstm_config, horizon):
        super().__init__()
        self.config = LSTMConfig(**lstm_config)

        self.column_indices = self.config.column_indices
        self.label_index = self.column_indices[self.config.target_column]
        self.id_column_index = self.column_indices[self.config.id_column]
        self.dynamic_feature_columns_indicies = [
            v
            for k, v in self.column_indices.items()
            if k in self.config.dynamic_feature_columns
        ]
        self.indicies_to_embed = self.config.columns_cardinality

        self.horizon = horizon
        self.model_lstm = tf.keras.layers.LSTM(64, return_state=True)
        self.model_final_layer = tf.keras.Sequential(
            [
                tf.keras.layers.Dense(units=128, activation="tanh"),
                tf.keras.layers.Dense(units=1, activation="softplus"),
            ],
        )
        self.loss = loss_mean_absolute_error

    def return_optimiser(self):
        learning_scheduler = self.schedule_some_learning()
        return tf.keras.optimizers.Lion(learning_rate=learning_scheduler)

    def schedule_some_learning(
        self,
        initial_learning_rate=0.001,
        decay_steps=10000,
        decay_rate=0.99,
        staircase=True,
    ):
        return tf.keras.optimizers.schedules.ExponentialDecay(
            initial_learning_rate=initial_learning_rate,
            decay_steps=decay_steps,
            decay_rate=decay_rate,
            staircase=staircase,
        )

    def collect_some_lovely_callbacks(self):
        return [
            tf.keras.callbacks.EarlyStopping(
                monitor="val_loss",
                patience=6,
            ),
        ]

    def compute_loss(self, x=None, y=None, y_pred=None, sample_weight=None):
        """Overwrite the compute_loss method to use the custom loss function.

        We did this as we need to add a way to trim out the extra dimensions
        tthat are added to the y_pred tensor by the model.
        """
        return self.loss(y, y_pred, target_index=self.label_index)

    def model_output(self, x, state):
        """This has gotten a bit cursed. We are slicing the input tensor to apply unique
        layers to each index. This allows us to initially embed chosen columns and
        in the future it would allow some creative feature engineering.

        Is there a way of making this one single model so we can see the full summary?
        """
        starting_blocks = []
        for k, v in self.column_indices.items():
            if v == self.label_index:
                continue

            if v in self.indicies_to_embed.keys():
                cut_out_slice = tf.gather(x, indices=[v], axis=2)
                cut_out_slice = tf.cast(cut_out_slice, dtype=tf.int32)
                layer = tf.keras.layers.Embedding(
                    input_dim=self.indicies_to_embed[v],
                    output_dim=8,
                )(cut_out_slice)

            else:
                layer = tf.keras.layers.Identity()(tf.gather(x, indices=[v], axis=2))

            starting_blocks.append(layer)

        x = tf.keras.layers.concatenate(starting_blocks)

        x, *state = self.model_lstm(x, initial_state=state)
        prediction = self.model_final_layer(x)
        return prediction, state

    def take_most_recent_slice(self, tensor: tf.Tensor):
        """This does assume the bottom value is the most recent"""
        tensor = tf.reverse(tensor, axis=[1])
        tensor = tf.gather(tensor, indices=[0], axis=1)
        return tensor

    def lag_dimension(self, inputs: tf.Tensor, lag: int):
        save_initial_dimensions = inputs.shape
        pad_gather_target = tf.pad(
            inputs,
            paddings=[[0, 0], [0, 1], [0, 0]],
            mode="CONSTANT",
        )
        roll_forard_one = tf.roll(pad_gather_target, shift=lag, axis=1)
        return_to_initial_dimensions = tf.gather(
            roll_forard_one,
            indices=[i for i in range(save_initial_dimensions[1])],
            axis=1,
        )
        return return_to_initial_dimensions

    def call(self, inputs: tf.Tensor, training=None):
        ids = tf.gather(inputs["train"], indices=[self.id_column_index], axis=2)
        ids = tf.gather(ids, indices=[i for i in range(self.horizon)], axis=1)

        static_features_single = inputs["static_features"]

        if len(self.dynamic_feature_columns_indicies) > 0:
            future_dynamic_features = inputs["future_dynamic_features"]
            dynamic_features = tf.gather(
                inputs["train"],
                indices=self.dynamic_feature_columns_indicies,
                axis=2,
            )
        else:
            future_dynamic_features = tf.zeros_like(inputs["train"])[:, :, 0:0]
            dynamic_features = tf.zeros_like(inputs["train"])[:, :, 0:0]

        static_features = tf.repeat(
            static_features_single,
            repeats=inputs["train"].shape[1],
            axis=1,
        )
        target_lag_1 = self.lag_dimension(
            tf.gather(inputs["train"], indices=[self.label_index], axis=2),
            1,
        )
        tensor_to_use = tf.concat(
            [static_features, target_lag_1, dynamic_features],
            axis=2,
        )

        prediction, state = self.model_output(tensor_to_use, None)

        predictions = tf.TensorArray(dtype=tf.float32, size=0, dynamic_size=True)

        for i in range(self.horizon):
            prediction = tf.reshape(prediction, [-1, 1, 1])
            prediction = tf.concat(
                [
                    static_features_single,
                    prediction,
                    tf.gather(future_dynamic_features, indices=[i], axis=1),
                ],
                axis=2,
            )
            prediction, state = self.model_output(prediction, state)
            predictions = predictions.write(i, prediction)

        predictions = predictions.stack()
        predictions = tf.transpose(predictions, [1, 0, 2])

        # Add the ids back to the predictions
        predictions = tf.concat([ids, predictions], axis=2)

        return predictions

    def sort_tensor(self, tensor: tf.Tensor):
        if len(tensor.shape) < 3:
            raise ValueError("Tensor must have at least 3 dimensions")

        return tf.stack(sorted(tensor, key=lambda x: x[0, self.id_column_index]))

    def predict_step(
        self,
        future_dynamic_features: tf.Tensor,
        final_target_value: tf.Tensor,
        static_features_single: tf.Tensor,
    ):

        if future_dynamic_features is None:
            raise ValueError("Future dates must be provided!")

        ids = tf.gather(future_dynamic_features, indices=[self.id_column_index], axis=2)
        ids = self.sort_tensor(ids)

        future_dynamic_features = self.sort_tensor(future_dynamic_features)
        future_dynamic_features = future_dynamic_features[
            :,
            :,
            (self.id_column_index + 1) :,
        ]

        predictions = tf.TensorArray(dtype=tf.float32, size=0, dynamic_size=True)

        # sorted in the method when ids existed. Just need to make it consistent
        # and obious across all three tensors.
        prediction = self.sort_tensor(final_target_value)
        state = None
        for i in range(self.horizon):
            prediction = tf.reshape(prediction, [-1, 1, 1])
            input = tf.concat(
                [
                    static_features_single,
                    prediction,
                    tf.gather(future_dynamic_features, indices=[i], axis=1),
                ],
                axis=2,
            )
            prediction, state = self.model_output(input, state)
            predictions = predictions.write(i, prediction)

        predictions = predictions.stack()
        predictions = tf.transpose(predictions, [1, 0, 2])
        predictions = tf.concat([ids, predictions], axis=2)

        return predictions
