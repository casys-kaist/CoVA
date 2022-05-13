import tensorflow.keras as keras
import tensorflow as tf


@tf.function
def clip_6(x):
    return tf.clip_by_value(x, 0.0, 6.0) / 6.0


class Preprocessing(keras.Model):
    def __init__(self, **kwags):
        super(Preprocessing, self).__init__()

        self.norm = clip_6

    @tf.function
    def call(self, inp):
        return self.norm(inp)


if __name__ == "__main__":
    inp = keras.Input((4, 3, 4, 45, 80))
    print(Preprocessing()(inp).shape)
