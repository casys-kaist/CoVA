import tensorflow.keras as keras
import tensorflow as tf


def build_upsample_block(
    channel, kernel, input_shape, desired_shape, use_dropout=False, use_relu=False
):
    expanding = keras.models.Sequential()
    if use_relu:
        expanding.add(keras.layers.ReLU())
    if use_dropout:
        expanding.add(keras.layers.Dropout(0.2))
    expanding.add(
        keras.layers.Conv3DTranspose(
            filters=channel,
            kernel_size=kernel,
            strides=(1, 2, 2),
            kernel_initializer="he_normal",
            padding="valid",
            data_format="channels_first",
            output_padding=(0, 0, 0),
            use_bias=True,
        )
    )

    # Calculate shape after Upsampling
    tmp = keras.layers.Conv3DTranspose(
        filters=channel,
        kernel_size=kernel,
        strides=(1, 2, 2),
        kernel_initializer="he_normal",
        padding="valid",
        data_format="channels_first",
        output_padding=(0, 0, 0),
        use_bias=True,
    )
    input_shape = list(input_shape)
    if input_shape[0] == None:
        input_shape[0] = 1
    resulted_shape = tuple(tmp(tf.zeros(input_shape)).shape)
    del tmp

    h_pad = resulted_shape[-2] - desired_shape[-2]
    w_pad = resulted_shape[-1] - desired_shape[-1]

    # They should have the same sign
    assert h_pad * w_pad >= 0

    if h_pad > 0 or w_pad > 0:
        expanding.add(
            keras.layers.Cropping3D(
                (
                    0,
                    (h_pad // 2 + h_pad % 2, h_pad // 2),
                    (w_pad // 2 + w_pad % 2, w_pad // 2),
                ),
                data_format="channels_first",
            )
        )

    elif h_pad < 0 or w_pad < 0:
        h_pad = -h_pad
        w_pad = -w_pad
        expanding.add(
            keras.layers.ZeroPadding3D(
                (
                    0,
                    (h_pad // 2 + h_pad % 2, h_pad // 2),
                    (w_pad // 2 + w_pad % 2, w_pad // 2),
                ),
                data_format="channels_first",
            )
        )

    return expanding


class Decoder(tf.keras.Model):
    def __init__(
        self,
        shapes,
        channels_u,
        kernel_size_u,
        kernel_size_s,
        padding="same",
        drop=0.1,
        use_relu=True,
        use_dropout=True,
    ):
        super(Decoder, self).__init__()

        assert len(shapes) == len(channels_u) + 1
        assert len(kernel_size_u) == 3
        assert len(kernel_size_s) == 3

        self.expanding = []
        for i, ch_u in enumerate(channels_u):
            up = build_upsample_block(
                ch_u,
                kernel_size_u,
                shapes[i],
                shapes[i + 1],
                use_relu=use_relu,
                use_dropout=use_dropout,
            )
            if i == len(channels_u) - 1:
                bn, convs = None, None

            else:
                bn = keras.layers.BatchNormalization(axis=1)

                convs = keras.Sequential()

                if len(convs.layers) == 0:
                    convs.add(keras.layers.Layer())

            self.expanding.append((up, bn, convs))
        self.final = keras.layers.Conv3D(1, 1, data_format="channels_first")
        self.activation = keras.layers.Activation("sigmoid", dtype="float32")

    @tf.function
    def call(self, inputs):
        x = inputs[0]

        for inp, (up, bn, convs) in zip(inputs[1:], self.expanding):
            x = up(x)  # ConvT + Crop
            x = bn(x)  # BatchNorm
            x = keras.layers.concatenate([x, inp], axis=1)
            x = convs(x)  # Extra convolutions

        x = self.expanding[-1][0](x)  # Last ConvT + Crop
        x = self.final(x)
        out = self.activation(x)

        return out

