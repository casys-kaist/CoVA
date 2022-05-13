#!/usr/bin/env python

import argparse
import cv2 as cv
from matplotlib import pyplot as plt
import numpy as np
import os
from rich.progress import Progress
import pathlib

parser = argparse.ArgumentParser()
parser.add_argument("VIDEO_PATH")
parser.add_argument("MOG_PATH")
args = parser.parse_args()

def contour_fill(img):
    """Apply contour filling

    Arguments:
    img -- binary image to fill of value [0, 1]
    """
    tmp = img.copy()
    contour, hier = cv.findContours(tmp, cv.RETR_EXTERNAL, cv.CHAIN_APPROX_SIMPLE)
    tmp = cv.drawContours(tmp, contour, -1, 1, cv.FILLED)

    return tmp

backSub = cv.createBackgroundSubtractorMOG2(
    history=30 * 60 * 5, varThreshold=32, detectShadows=False
)


capture = cv.VideoCapture(args.VIDEO_PATH)
frame_cnt = capture.get(cv.CAP_PROP_FRAME_COUNT)

cl_kernel = np.ones((4, 4))
op_kernel = np.ones((6, 6))

frames = []

with open(args.MOG_PATH, "wb") as f:
    with Progress() as progress:
        task = progress.add_task("Generating BG label", total=frame_cnt)
        while True:
            ret, frame = capture.read()
            if not ret:
                break

            frame = cv.resize(frame, (1280 // 2, 720 // 2))

            fgMask = backSub.apply(frame)

            fg_all = (fgMask > 0).astype(np.uint8)

            cl = cv.morphologyEx(fg_all, cv.MORPH_CLOSE, cl_kernel)
            cl_op = cv.morphologyEx(cl, cv.MORPH_OPEN, op_kernel)
            cl_op_fill = contour_fill(cl_op)
            cl_op_fill = cl_op_fill[::8, ::8]
            cl_op_fill.tofile(f)
            progress.update(task, advance=1)
