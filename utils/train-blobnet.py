#!/usr/bin/env python

import pandas as pd
import numpy as np
import yaml
import math
import pickle
import os
import argparse
import pathlib
import shutil

import tensorflow as tf
import tensorflow.keras as keras

from data import load_dataset
from model import BlobNet

import signal, os

flag = False

def handler(signum, frame):
    global flag
    if not flag:
        print(
            "Training will stop after current batch, send SIGINT again to exit immediately."
        )
        flag = True
    else:
        print("Second SIGINT caught, terminating.")
        exit(0)

signal.signal(signal.SIGINT, handler)


class TerminateOnFlag(keras.callbacks.Callback):
    def on_batch_end(self, batch, logs=None):
        global flag
        if flag == True:
            self.model.stop_training = True


# https://stackoverflow.com/a/50832690/10361854
def jaccard_distance_loss(y_true, y_pred, smooth=100):
    """Calculates mean of Jaccard distance as a loss function"""
    y_true = tf.squeeze(y_true)
    y_pred = tf.squeeze(y_pred)
    intersection = tf.reduce_sum(y_true * y_pred, axis=(-2, -1))
    sum_ = tf.reduce_sum(y_true + y_pred, axis=(-2, -1))
    jac = (intersection + smooth) / (sum_ - intersection + smooth)
    jd = (1 - jac) * smooth
    return tf.reduce_mean(jd)


def train_model(train_ds):
    model = BlobNet(
        input_shape=[3, 4, 45, 80],
        # Encoder
        encode_c_s=[[16], [32], [64], [128]],
        encode_k_s=[1, 3, 3],
        encode_c_t=[[4, 4], [4, 4], [4, 4], [4, 4]],
        encode_a='relu',
        encode_bn=True,
        # Decoder
        decode_c_u=[64, 32, 16, 16],
        decode_k_u=[1, 4, 4],
        decode_k_s=[1, 3, 3],
    )

    def scheduler(epoch, lr):
        if epoch < 10:
            return lr
        else:
            return lr * tf.math.exp(-0.1)

    lr_callback = tf.keras.callbacks.LearningRateScheduler(scheduler)
    tof_callack = TerminateOnFlag()

    opt = tf.keras.optimizers.Adam()

    model.compile(
        optimizer=opt,
        loss=jaccard_distance_loss,
        metrics=[
            tf.keras.metrics.Precision(name="precision"),
            tf.keras.metrics.Recall(name="recall"),
        ],
    )

    model.fit(
        train_ds,
        epochs=20,
        batch_size=4,
        callbacks=[lr_callback, tof_callack],
    )

    return model


def run(record_path, frozen_path):
    # Log general information
    print(tf.__version__)

    # Load training dataset
    train_ds = load_dataset(record_path)
    print("Training Dataset:", train_ds)

    # Train BlobNet
    model = train_model(train_ds)

    # Save trained model into frozen model
    inp = keras.Input((3, 4*45, 80))
    tmp = keras.layers.Reshape((3, 4, 45, 80))(inp)
    out = model(tmp)
    model = keras.Model(inp, out)
    keras.models.save_model(
        model, frozen_path, include_optimizer=False
    )


if __name__ == "__main__":
    # Recieves the path to the parser and
    parser = argparse.ArgumentParser()
    parser.add_argument("RECORD_PATH")
    parser.add_argument("FROZEN_PATH")
    args = parser.parse_args()

    run(args.RECORD_PATH, args.FROZEN_PATH)
