#!/bin/bash

LOCATION=/workspace/demo/1m.mp4

# Test time stacking
# GST_DEBUG=metapreprocess:7,"*BUFFER*:7" \
GST_PLUGIN_PATH=../../target/debug/ \
    gst-launch-1.0 \
    filesrc location=$LOCATION \
        ! qtdemux ! h264parse ! avdec_h264 max-threads=1 \
        ! metapreprocess timestep=4 \
        ! filesink location=/tmp/metapreprocess.dump
