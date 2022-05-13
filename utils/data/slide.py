import tensorflow as tf


@tf.function
def slide_dataset(ds, timestep, skip=True):
    """Timely stacks dataset in inversed order
    When input is given as [(x0, y0), (x1, y1), (x2, y2), ...],
    and the timestep is 2, the returned dataset will produce
    [({x1, x0}, y1), ({x2, x1}, y2), ({x3, x2}, y3), ...],
    given that {} denotes tf.concat(axis=1)

    Keyword arguments:
    skip -- produces [({x1, x0}, y1), ({x3, x2}, y3), ...] instead (default True)
    """
    print("Sliding applied to:", ds)

    x_ds = ds.map(lambda x, y: x)
    y_ds = ds.map(lambda x, y: y)

    mod_timestep = lambda i, _: (i % timestep) == 0
    x_ds = (
        x_ds.batch(timestep)
        .map(lambda t: tf.reverse(t, axis=[0]))  # Reverse the stack
        .map(lambda t: tf.transpose(t, (1, 0, 2, 3)))
    )  # T, C, H, W => C, T, H, W

    y_ds = y_ds.skip(timestep - 1).enumerate().filter(mod_timestep).map(lambda _, t: t)

    if not skip:
        for i in range(1, timestep):
            tmp_x_ds = x_ds.skip(i).batch(timestep)
            tmp_y_ds = (
                y_ds.skip(i)
                .skip(timestep - 1)
                .enumerate()
                .filter(mod_timestep)
                .map(lambda _, y: tf.expand_dims(y, 0))
            )
            x_ds.concatenate(tmp_x_ds)
            y_ds.concatenate(tmp_y_ds)

    ds = tf.data.Dataset.zip((x_ds, y_ds))

    print("Resulted:", ds, "\n")
    return ds
