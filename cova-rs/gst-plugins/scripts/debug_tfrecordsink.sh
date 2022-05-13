#!/bin/bash

LOCATION=/ssd1/archie/day1-10s.mp4
GT=/ssd2/h264/archie/train/day1_gt.dump

GST_DEBUG=tfrecordsink:7 \
GST_PLUGIN_PATH=../../target/debug/ \
    gst-launch-1.0 \
    filesrc location=$LOCATION \
        ! qtdemux ! h264parse ! avdec_h264 max-threads=1 \
        ! metapreprocess \
        ! queue max-size-buffers=1 \
        ! tfrecordsink sync=false gt=$GT location=/tmp/debug.tfrecord

