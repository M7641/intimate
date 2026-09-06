import tensorflow as tf


class Naive(tf.keras.Model):
    def __init__(self, window_maker, horizon):
        super().__init__()
        self.label_index = window_maker.target_column_index
        self.id_column_index = window_maker.id_column_index
        self.horizon = horizon

    def warmup(self, inputs):
        result = inputs[:, :, self.label_index]
        return result[:, -1, tf.newaxis]

    def model_output(self, x):
        return x

    def call(self, inputs: tf.Tensor):

        ids = tf.gather(inputs, indices=[self.id_column_index], axis=2)
        ids = tf.gather(ids, indices=[i for i in range(self.horizon)], axis=1)

        predictions = tf.TensorArray(dtype=tf.float32, size=0, dynamic_size=True)
        prediction = self.warmup(inputs)
        predictions = predictions.write(0, prediction)

        for i in range(1, self.horizon):
            x = prediction
            prediction = self.model_output(x)
            predictions = predictions.write(i, prediction)

        predictions = predictions.stack()
        predictions = tf.transpose(predictions, [1, 0, 2])

        # Add the ids back to the predictions
        predictions = tf.concat([ids, predictions], axis=2)
        return predictions
