import tensorflow as tf


def parse_example_ds(ds, height, width):
    feature_description = {
        "mb_type": tf.io.FixedLenFeature([1], tf.string),
        "mv_x": tf.io.FixedLenFeature([1], tf.string),
        "mv_y": tf.io.FixedLenFeature([1], tf.string),
        "gt": tf.io.FixedLenFeature([1], tf.string),
    }

    def _parse_feature_fn(example_proto):
        return tf.io.parse_example(example_proto, feature_description)

    @tf.function
    def _decode_raw_fn(parsed_feature):
        mb_type = tf.io.decode_raw(parsed_feature["mb_type"], tf.uint8)
        mv_x = tf.io.decode_raw(parsed_feature["mv_x"], tf.uint8)
        mv_y = tf.io.decode_raw(parsed_feature["mv_y"], tf.uint8)
        gt = tf.io.decode_raw(parsed_feature["gt"], tf.uint8)

        mv_x = tf.cast(tf.reshape(mv_x, (-1, 1, height, width)), tf.float32)
        mv_y = tf.cast(tf.reshape(mv_y, (-1, 1, height, width)), tf.float32)

        # Apply one-hot
        # mb_type = tf.one_hot(mb_type, 8)
        mb_type = tf.cast(tf.reshape(mb_type, (-1, 1, height, width)), tf.float32)

        gt = tf.cast(tf.reshape(gt, (-1, 1, height, width)), tf.float32)

        return tf.concat([mb_type, mv_x, mv_y], axis=1), gt

    parsed_ds = ds.map(
        _parse_feature_fn,
        num_parallel_calls=4,
    ).prefetch(tf.data.AUTOTUNE)
    decoded_ds = parsed_ds.map(
        _decode_raw_fn,
        num_parallel_calls=4,
    ).prefetch(tf.data.AUTOTUNE)
    return decoded_ds
