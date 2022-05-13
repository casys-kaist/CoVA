import tensorflow.keras as keras
import tensorflow as tf


class PointWiseTN(keras.Model):
    def __init__(self, channels, drop=0.2):
        super(PointWiseTN, self).__init__()

        self.inner = keras.Sequential()
        for c in channels:
            self.inner.add(keras.layers.Conv1D(c, 1, activation="relu", use_bias=False))
            self.inner.add(keras.layers.Dropout(drop))

        self.relu = keras.layers.ReLU()

    @tf.function
    def call(self, x):
        # [N, C, T, H, W] ==> [N, C, H, W, T]
        out = tf.transpose(x, (0, 1, 3, 4, 2))
        out = self.inner(out)
        # [N, C, H, W, T] ==> [N, C, T, H, W]
        out = tf.transpose(out, (0, 1, 4, 2, 3))

        out = out + x
        out = self.relu(out)
        return out
