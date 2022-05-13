import tensorflow as tf
from .balance import balance_dataset
from .parse import parse_example_ds
from .slide import slide_dataset


def load_dataset(record_path):
    PROC_SIZE = 16  # Preprocessing Size

    # Load dataset
    ds = tf.data.TFRecordDataset(
        [record_path],
        buffer_size=100_000,
        num_parallel_reads=8,
    ).batch(PROC_SIZE)
    train_ds = parse_example_ds(ds, 45, 80)

    # Apply sliding
    train_ds = slide_dataset(train_ds.unbatch(), 4)
    train_ds = train_ds.batch(PROC_SIZE)

    # Apply balancing
    # train_ds = balance_dataset(train_ds.unbatch(), 100)
    # train_ds = train_ds.batch(PROC_SIZE)

    # Apply batching
    train_ds = train_ds.unbatch()
    train_ds = train_ds.batch(4)
    train_ds = train_ds.prefetch(tf.data.AUTOTUNE)
    # train_ds = train_ds.take(500)
    return train_ds


if __name__ == "__main__":
    import argparse
    import yaml

    parser = argparse.ArgumentParser()
    parser.add_argument("CONFIG", help="Path to the config YAML file")
    args = parser.parse_args()
    config = yaml.load(open(args.CONFIG), yaml.FullLoader)

    train_ds, val_ds, *_ = load_dataset(config["data"])
    x, y = list(train_ds.take(1))[0]
    print(x.shape, y.shape)
