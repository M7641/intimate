import tensorflow as tf
from keras.src.losses.loss import squeeze_or_expand_to_same_rank


def mean_absolute_error(y_true, y_pred):
    if len(y_pred.shape) > 2:
        y_pred = y_pred[:, :, 1]

    y_pred = tf.keras.ops.convert_to_tensor(y_pred)
    y_true = tf.keras.ops.convert_to_tensor(y_true, dtype=y_pred.dtype)
    y_true, y_pred = squeeze_or_expand_to_same_rank(y_true, y_pred)
    return tf.keras.ops.mean(tf.keras.ops.abs(y_true - y_pred), axis=-1)
