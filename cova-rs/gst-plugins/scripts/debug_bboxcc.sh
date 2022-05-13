#!/bin/bash

# if [[ $# -ne 1 ]]; then
#     echo "Usage: $0 BATCH"
#     exit -1
# fi

LOCATION=/ssd2/h264/archie/train/day1.mp4

GST_DEBUG=bboxcc:7 \
GST_PLUGIN_PATH=../target/debug/ \
gst-launch-1.0 \
    filesrc location=${LOCATION} ! qtdemux \
        ! h264parse config-interval=-1 ! queue max-size-buffers=0 max-size-bytes=0 max-size-time=0 \
        ! queue max-size-buffers=1024 max-size-bytes=0 max-size-time=0 \
        ! avdec_h264 max-threads=1 \
        ! metapreprocess timestep=4 \
        ! nvvideoconvert nvbuf-memory-type=2 output-buffers=128 \
        ! 'video/x-raw(memory:NVMM),format=(string)RGBA'  \
        ! m_fcn0.sink_0 nvstreammux name=m_fcn0 width=80 height=180 batch-size=128 \
            buffer-pool-size=1024 nvbuf-memory-type=2 \
        ! nvinfer config-file-path=/workspace/config/fcn/archie_b128.txt \
        ! queue max-size-buffers=1024 max-size-bytes=0 max-size-time=0 \
        ! nvstreamdemux name=d_fcn0 d_fcn0.src_0 \
        ! maskcopy \
        ! bboxcc cc-threshold=3 \
        ! fakesink
