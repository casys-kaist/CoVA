#!/bin/bash

os="ubuntu1804"
tag="cuda11.4-trt8.2.4.2-ga-20220324"

dpkg -i nv-tensorrt-repo-${os}-${tag}_1-1_amd64.deb
apt-key add /var/nv-tensorrt-repo-${os}-${tag}/7fa2af80.pub

apt-get update
apt-get install -y tensorrt
apt-get install -y python3-libnvinfer-dev uff-converter-tf onnx-graphsurgeon
