import tensorflow.keras as keras
import tensorflow as tf

from .pointwise import PointWiseTN


class Encoder(tf.keras.Model):
    def __init__(
        self,
        channels_s,
        channels_t,
        kernel_size_s,
        padding="same",
        drop=0.1,
        activation="relu",
        use_bias=True,
        use_bn=True,
    ):
        super(Encoder, self).__init__()

        assert len(kernel_size_s) == 3
        assert len(channels_s) == len(channels_t)

        self.contracting = []

        """
        [[64, 64], [128, 128], [256, 256], [512, 512], [1024, 1024]]
        will reproduce original U-Net
        """
        for ch_s, ch_t in zip(channels_s, channels_t):
            conv_s = keras.Sequential()
            # TODO: Consider training separate Siamese head for Vector vs Scalar
            for c in ch_s:
                conv_s.add(
                    keras.layers.Conv3D(
                        filters=c,
                        kernel_size=kernel_size_s,
                        padding=padding,
                        data_format="channels_first",
                        use_bias=use_bias,
                        activation=activation,
                        kernel_initializer="he_normal",
                    )
                )
            if use_bn:
                bn = keras.layers.BatchNormalization(axis=1)
            else:
                bn = keras.layers.Layer()

            pool = keras.layers.MaxPool3D(
                (1, 2, 2), data_format="channels_first", padding="valid"
            )
            conv_t = PointWiseTN(channels=ch_t)

            self.contracting.append((conv_s, bn, pool, conv_t))

    @tf.function
    def call(self, x, training=None, mask=None):
        # [N, C, T, H, W]
        ret = []

        for conv_s, bn, pool, conv_t in self.contracting:
            x = conv_s(x)  # Convs
            s = x.shape
            x = bn(x)  # Batch Norm
            x = pool(x)  # Max Pool

            # Add padding if not factor of 2
            if s[-2] % 2:
                x = keras.layers.ZeroPadding3D(
                    (0, (1, 0), (0, 0)), data_format="channels_first"
                )(x)
            if s[-1] % 2:
                x = keras.layers.ZeroPadding3D(
                    (0, (0, 0), (1, 0)), data_format="channels_first"
                )(x)
            # Temp. domain conv.
            x = conv_t(x)
            ret.append(x)
        return ret


# inp = keras.Input((32, 250, 45, 80))
# [t.shape for t in Encoder((64, 128, 256, 512), padding='same')(inp)]
