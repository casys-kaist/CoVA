import tensorflow as tf


def balance_dataset(ds, threshold):
    print("Balancing applied to:", ds)

    @tf.function
    def _above_threshold(_, y):
        return tf.math.reduce_sum(y) >= threshold

    @tf.function
    def _below_threshold(_, y):
        return tf.math.reduce_sum(y) < threshold

    large_ds = ds.filter(_above_threshold).repeat()
    small_ds = ds.filter(_below_threshold).repeat()

    ds = tf.data.Dataset.sample_from_datasets(
        [large_ds, small_ds], weights=[0.5, 0.5], stop_on_empty_dataset=True
    )
    print("Resulted:", ds, "\n")
    return ds
