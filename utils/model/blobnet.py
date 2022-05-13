from .encoder import Encoder
from .decoder import Decoder
from .preprocessing import Preprocessing
import tensorflow.keras as keras
import tensorflow as tf


def BlobNet(
    input_shape,
    encode_c_s, encode_c_t, encode_k_s, encode_a, encode_bn,
    decode_c_u, decode_k_u, decode_k_s,
):
    # Start building network
    inp = keras.Input(shape=input_shape)
    # [N, T, H, W, C] => [N, C, T, H, W]
    # x_tmp = tf.transpose(inp, (0, 4, 1, 2, 3))

    preprocessing = Preprocessing()
    x_pre = preprocessing(inp)

    encoder = Encoder(
        channels_s=encode_c_s,
        channels_t=encode_c_t,
        kernel_size_s=encode_k_s,
        padding="same",
        activation=encode_a,
        use_bias=True,
        use_bn=encode_bn,
    )
    x_enc = encoder(x_pre)

    x_rev = [x[:, :, :1] for x in reversed(x_enc)]

    shapes = [x.shape for x in x_rev]
    shapes.append(inp.shape)
    decoder = Decoder(
        shapes=shapes,
        channels_u=decode_c_u,
        kernel_size_u=decode_k_u,
        kernel_size_s=decode_k_s,
    )

    x_dec = decoder(x_rev)
    out = tf.squeeze(x_dec, axis=1)

    model = keras.Model(inputs=[inp], outputs=[out])

    return model


# input_shape = (250, 45, 80, 89)
# inp = keras.Input(input_shape)
# model = BlobNet(input_shape, (64, 128, 256, 512), 16, 16)
# # model.model((250, 45, 80, 89)).summary()
# model(inp).shape
