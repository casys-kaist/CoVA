#!/usr/bin/env python
# coding: utf-8
import pandas as pd
import numpy as np
import os
import yaml
import re
import pathlib
import sys
import ray
import parse
import common
import argparse

config = yaml.safe_load(open('./config.yaml'))

parser = argparse.ArgumentParser()
parser.add_argument("DATASET", choices=config.keys())
parser.add_argument("OUTPUT_DIR")
args = parser.parse_args()

label_helper = common.LabelHelper('../third_parties/DeepStream-Yolo/labels.txt')

key = []
ret = []

d = config[args.DATASET]
days = d['days']
exclude = eval(d['exclude'])
targets_str = '-'.join(d['targets'])
targets = [label_helper.label_to_num(t) for t in d['targets']]
region = d['region']

total_elapsed= 0
total_dropped = 0
total_dependency = 0
total_inference = 0

for day, d in days.items():
    elapsed, dropped, dependency, inference = parse.parse_txt(d['cova_path'])
    total_elapsed += elapsed
    total_dropped += dropped
    total_dependency += dependency
    total_inference += inference
    key.append(day)

    ret.append(parse.parse_query.remote(
        d['video_path'],
        d['gt_path'],
        pathlib.Path(d['cova_path']),
        exclude,
        targets,
        region
    ))
# Parse scripts in parallel
ret = ray.get(ret)

# Calculate accumulated metrics across all day
bp_acc = 0
gc_acc = 0
bpl_acc = 0
gcl_acc = 0
for k, v in zip(key, ret):
    day = k
    (
        ts_len,
        gt_df, cova_df, gt_bp,
        bp_accuracy, gc_err,
        gt_df_local, cova_df_local, gt_bp_local,
        bp_accuracy_local, gc_err_local
    ) = v

    bp_acc += bp_accuracy
    gc_acc += gc_err
    bpl_acc += bp_accuracy_local
    gcl_acc += gc_err_local


total_decoded = total_dependency + total_inference
total_frames = total_dropped + total_decoded
decode_rate = total_decoded / total_frames
inference_rate = total_inference / total_frames
print('Elapsed seconds:', total_elapsed)
print(f'Decode filter rate: {(1 - decode_rate)*100:.02f}%')
print(f'Inference filter rate: {(1 - inference_rate)*100:.02f}%')

# Report averaged metric
NUM_DAYS = len(days)
print('BP', bp_acc / NUM_DAYS)
print('GC', gc_acc / NUM_DAYS)
print('BPL', bpl_acc / NUM_DAYS)
print('GCL', gcl_acc / NUM_DAYS)

output_dir = pathlib.Path(args.OUTPUT_DIR)
output_dir.mkdir(exist_ok=True, parents=True)

with open(output_dir / f'{targets_str}.txt', 'w') as f:
    for ts in gt_bp.index[np.where(gt_bp)[0]]:
        print(ts, file=f)

with open(output_dir / f'{targets_str}_{region}.txt', 'w') as f:
    for ts in gt_bp_local.index[np.where(gt_bp_local)[0]]:
        print(ts, file=f)
