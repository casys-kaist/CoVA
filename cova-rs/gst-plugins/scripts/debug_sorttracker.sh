#!/bin/bash

# if [[ $# -ne 1 ]]; then
#     echo "Usage: $0 BATCH"
#     exit -1
# fi

LOCATION=/ssd1/VIRAT/transcoded/VIRAT_S_000001.mp4

RUST_BACKTRACE=1 \
GST_DEBUG=sorttracker:3 \
GST_PLUGIN_PATH=../target/debug/ \
gst-launch-1.0 \
    filesrc location=${LOCATION} ! qtdemux \
        ! h264parse config-interval=-1 \
        ! nvv4l2decoder cudadec-memtype=0 \
        ! m.sink_0 nvstreammux name=m width=1280 height=720 batch-size=2 \
            buffer-pool-size=1024 nvbuf-memory-type=2 \
        ! nvinfer config-file-path=/workspace/config/rnn/yolov4_b2.txt \
        ! nvstreamdemux name=d d.src_0 \
        ! nvdsbbox \
        ! queue \
        ! sorttracker \
        ! bboxsink location=/tmp/debug.csv sync=false
