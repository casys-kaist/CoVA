#!/bin/bash

LOCATION=/ssd1/archie/day1-10m.mp4

GST_DEBUG=cova:6 \
GST_PLUGIN_PATH=../target/debug/ \
gst-launch-1.0 \
    filesrc location=${LOCATION} ! qtdemux \
        ! h264parse config-interval=-1 ! queue max-size-buffers=0 max-size-bytes=0 max-size-time=0 \
            ! tee name=t0 \
            t0.src_0 \
                ! queue max-size-buffers=1024 max-size-bytes=0 max-size-time=0 \
                ! c.sink_enc cova name=c \
                    sort-maxage=30 sort-iou=0.5 sort-minhits=50 \
                    alpha=1 beta=1 port=0 \
                ! nvv4l2decoder \
                ! fakesink \
            t0.src_1 \
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
                ! c.sink_mask
