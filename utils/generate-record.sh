#!/bin/bash

if [[ $# -ne 3 ]]; then
    echo "Usage: $0 VIDEO_PATH GT_PATH RECORD_PATH"
    exit 0
fi

gst-launch-1.0 \
    filesrc location=$1 \
        ! qtdemux \
        ! h264parse \
        ! avdec_h264 max-threads=1 \
        ! metapreprocess \
        ! tfrecordsink sync=false gt=$2 location=$3
