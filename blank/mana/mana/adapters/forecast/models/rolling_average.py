import tensorflow as tf


class RollingAverage(tf.keras.Model):
    def __init__(self, window_maker, horizon, rolling_average: int = 4):
        super().__init__()
        self.label_index = window_maker.target_column_index
        self.id_column_index = window_maker.id_column_index
        self.horizon = horizon
        self.rolling_average = rolling_average

    def warmup(self, inputs):
        result = tf.reshape(
            tf.reduce_mean(inputs[:, -self.rolling_average :], 1),
            [-1, 1],
        )
        return result

    def model_output(self, x):
        return tf.reshape(tf.reduce_mean(x[:, -self.rolling_average :], 1), [-1, 1])

    def call(self, inputs: tf.Tensor):

        if inputs.shape[1] == 0:
            return tf.constant([])

        if inputs.shape[1] < self.rolling_average:
            return tf.constant([])

        ids = tf.gather(inputs, indices=[self.id_column_index], axis=2)
        ids = tf.gather(ids, indices=[i for i in range(self.horizon)], axis=1)

        inputs = inputs[:, :, self.label_index]

        predictions = tf.TensorArray(dtype=tf.float32, size=0, dynamic_size=True)
        prediction = self.warmup(inputs)
        predictions = predictions.write(0, prediction)

        for i in range(1, self.horizon):
            x = tf.concat([inputs, prediction], axis=1)
            prediction = self.model_output(x)
            predictions = predictions.write(i, prediction)

        predictions = predictions.stack()
        predictions = tf.transpose(predictions, [1, 0, 2])

        # Add the ids back to the predictions
        predictions = tf.concat([ids, predictions], axis=2)
        return predictions
