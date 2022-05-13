#!/usr/bin/env python
# coding: utf-8
import pandas as pd
import numpy as np
import cv2
import os
import yaml
import re
import pathlib
import sys
import ray

import common
import multiprocessing

def load_gt(gt_path):
    gt_csv = pathlib.Path(gt_path)
    gt_df = pd.read_csv(gt_csv)

    gt_df.set_index('timestamp', drop=True, inplace=True)
    gt_df.sort_index(inplace=True)

    return gt_df

def load_cova(cova_path):
    cova_root = pathlib.Path(cova_path)

    assoc_csv = cova_root / 'assoc.csv'
    stationary_csv = cova_root / 'stationary.csv'

    assoc_df = pd.read_csv(assoc_csv)
    stationary_df = pd.read_csv(stationary_csv)

    cova_df = pd.concat([assoc_df, stationary_df])
    cova_df.set_index('timestamp', drop=True, inplace=True)
    cova_df.sort_index(inplace=True)

    return cova_df


# In[10]:


def get_ts_range(video_path, gt_df):
    # Getting max timestamp from video header
    cap = cv2.VideoCapture(video_path)
    assert cap.isOpened()

    fps = cap.get(cv2.CAP_PROP_FPS)
    frame_count = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
    duration_sec = frame_count/fps
    SEC_TO_NSEC = 1_000_000_000

    video_ts_max = int(duration_sec * SEC_TO_NSEC)
    gt_ts_max = gt_df.index.max()

    ts_max = max(gt_ts_max, video_ts_max)
    ts_range = common.arange_ts(0, ts_max)

    return ts_range

# While inspecting YOLOv4 result, we noticed some small objects resulting
# unstable detection even when they are not moving at all.
# Even when CoVA is able to cover static objects, such noises from the baseline
# hinders the evaluation metric.
# To this end, we exclude small region from both YOLOv4 baseline and CoVA.
def get_exclude_df(df, exclude):
    df = df.copy()
    df['right'] = df['left'] + df['width']
    df['bottom'] = df['top'] + df['height']
    for ((left, top), (right, bottom)) in exclude:
        # We exclude only the detection "fully" inside the small region so that
        # no other detections are discarded
        exclude_idx = df['left'] >= left
        exclude_idx &= df['top'] >= top
        exclude_idx &= df['right'] <= right
        exclude_idx &= df['bottom'] <= bottom

        df = df[~exclude_idx].copy()

    return df


def get_local_df(df, region):
    df = df.copy()
    df['right'] = df['left'] + df['width']
    df['bottom'] = df['top'] + df['height']

    if region == 'upper left':
        idx = df['right'] <= 1280 / 2
        idx &= df['bottom'] <= 640 / 2
    if region == 'upper right':
        idx = df['left'] >= 1280 / 2
        idx &= df['bottom'] <= 640 / 2
    if region == 'lower left':
        idx = df['right'] <= 1280 / 2
        idx &= df['top'] >= 640 / 2
    if region == 'lower right':
        idx = df['left'] <= 1280 / 2
        idx &= df['top'] >= 640 / 2

    df = df[idx]
    return df

def calculate_query(df, ts_range, targets):
    grouped_series = df.groupby(df.index)["class_id"].agg(list)

    # Calculate BP
    target_binary_series = grouped_series.apply(lambda l: np.isin(l, targets).any())

    bp = pd.DataFrame(False, index=ts_range, columns=['class_id'])
    bp.update(pd.DataFrame(target_binary_series))
    bp.fillna(method="ffill", inplace=True)
    bp.fillna(method="bfill", inplace=True)

    # Calculate GC
    target_count_series = grouped_series.apply(lambda l: np.isin(l, targets).sum())

    gc_df = pd.DataFrame(0, index=ts_range, columns=['class_id'])
    gc_df.update(pd.DataFrame(target_count_series))
    gc = gc_df.values.mean()

    return bp, gc

@ray.remote
def parse_query(video_path, gt_path, cova_path, exclude, s, region):
    # Load ground truth
    gt_df = load_gt(gt_path)
    # Load CoVA
    cova_df = load_cova(cova_path)
    # Calculate timestamp range
    ts_range = get_ts_range(video_path, gt_df)

    # Apply exclude region
    gt_df = get_exclude_df(gt_df, exclude)
    cova_df = get_exclude_df(cova_df, exclude)

    # Calculate metric for BP and GC
    gt_bp, gt_gc = calculate_query(gt_df, ts_range, s)
    cova_bp, cova_gc = calculate_query(cova_df, ts_range, s)
    correct_bp = gt_bp == cova_bp
    num_correct = correct_bp.values.sum()
    bp_accuracy = num_correct / len(gt_bp)
    gc_err = abs(gt_gc - cova_gc)

    # Calculate metric for local BP and GC
    gt_df_local = get_local_df(gt_df, region)
    cova_df_local = get_local_df(cova_df, region)

    # Caculate BP and GC
    gt_bp_local, gt_gc_local = calculate_query(gt_df_local, ts_range, s)
    cova_bp_local, cova_gc_local = calculate_query(cova_df_local, ts_range, s)
    correct_bp_local = gt_bp_local == cova_bp_local
    num_correct_local = correct_bp_local.values.sum()
    bp_accuracy_local = num_correct_local / len(gt_bp_local)
    gc_err_local = abs(gt_gc_local - cova_gc_local)

    return (
        len(ts_range),
        gt_df, cova_df, gt_bp,
        bp_accuracy, gc_err,
        gt_df_local, cova_df_local, gt_bp_local,
        bp_accuracy_local, gc_err_local
    )

def parse_txt(cova_path):
    cova_root = pathlib.Path(cova_path)
    out_txt = cova_root / 'out.txt'
    with out_txt.open() as f:
        for line in f:
            if 'Elapsed' in line:
                elapsed = float(line.split()[-1])
            elif 'dropped:' in line:
                dropped = int(line.split()[-1])
            elif 'dependency:' in line:
                dependency = int(line.split()[-1])
            elif 'inference:' in line:
                inference = int(line.split()[-1])

    return elapsed, dropped, dependency, inference
