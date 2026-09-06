import tensorflow as tf


def loss_mean_squared_error(y_true, y_pred, target_index=None):
    if len(y_pred.shape) > 2:
        y_pred = y_pred[:, :, 1]

    if len(y_true.shape) > 2:
        y_true = y_true[:, :, target_index]

    squared_difference = tf.keras.ops.square(y_true - y_pred)
    return tf.reduce_mean(squared_difference)


def loss_mean_absolute_error(y_true, y_pred, target_index=None):
    if len(y_pred.shape) > 2:
        y_pred = y_pred[:, :, 1]

    if len(y_true.shape) > 2:
        y_true = y_true[:, :, target_index]

    absolute_difference = tf.keras.ops.abs(y_true - y_pred)
    return tf.reduce_mean(absolute_difference)


def loss_mean_absolute_percentage_error(y_true, y_pred, target_index=None):
    if len(y_pred.shape) > 2:
        y_pred = y_pred[:, :, 1]

    if len(y_true.shape) > 2:
        y_true = y_true[:, :, target_index]

    absolute_percentage_difference = tf.keras.ops.abs(
        (y_true - y_pred)
        / tf.keras.ops.clip_by_value(tf.keras.ops.abs(y_true), 1, None),
    )
    return tf.reduce_mean(absolute_percentage_difference)


def loss_mean_absolute_error_weighted(y_true, y_pred, target_index=None):
    """Idea to be tried. Will prioritize the immediate future more than the distant future"""
    if len(y_pred.shape) > 2:
        y_pred = y_pred[:, :, 1]

    if len(y_true.shape) > 2:
        y_true = y_true[:, :, target_index]

    horizon_length = y_true.shape[1]

    weights = 1 / tf.range(1, horizon_length + 1, dtype=tf.float32)
    absolute_difference = tf.keras.ops.abs(y_true - y_pred)
    absolute_difference = tf.math.multiply(absolute_difference, weights)
    return tf.reduce_mean(absolute_difference)
