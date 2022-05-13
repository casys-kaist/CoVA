# syntax = docker/dockerfile:1.0-experimental
###
#
# Base Builder
#
###
FROM nvcr.io/nvidia/deepstream:6.0-devel AS base-builder
MAINTAINER Jinwoo Hwang

USER root

RUN apt-key adv --keyserver keyserver.ubuntu.com --recv-keys A4B469963BF863CC

RUN apt-get update && apt-get install -y \
        automake \
        autotools-dev \
        bison \
        curl \
        flex \
        libtool \
        gtk-doc-tools \
        software-properties-common \
        unzip \
        zip \
        python3-setuptools \
        python3-wheel \
        ninja-build \
        libgirepository1.0-dev

# Install python3.7 and pip
WORKDIR /root
RUN add-apt-repository ppa:deadsnakes/ppa -y && apt install -y python3.7 python3.7-distutils python3.7-dev
RUN ln -s -f /usr/bin/python3.7 /usr/bin/python
RUN curl -sSLO https://bootstrap.pypa.io/get-pip.py && python get-pip.py && python -m pip install --upgrade pip setuptools

WORKDIR /root/nvidia
COPY nv-tensorrt-repo-ubuntu1804-cuda11.4-trt8.2.4.2-ga-20220324_1-1_amd64.deb nv-tensorrt-repo-ubuntu1804-cuda11.4-trt8.2.4.2-ga-20220324_1-1_amd64.deb
COPY install_trt.sh install_trt.sh
RUN ./install_trt.sh
RUN rm -rf /root/nvidia

# Install common Python packages
RUN python -m pip install meson six numpy wheel setuptools mock future

# Will install newest gstreamer later
RUN apt-get remove --purge -y "*orc*"

###
#
# Build Tensorflow
#
###
FROM base-builder AS tf-builder

# Install bazel
WORKDIR /usr/local/lib/bazel/bin
RUN curl -fL https://releases.bazel.build/4.2.1/release/bazel-4.2.1-linux-x86_64 -o bazel \
    && chmod +x bazel


# Build Tensorflow
WORKDIR /build/tf
RUN git clone https://github.com/tensorflow/tensorflow.git
WORKDIR tensorflow
RUN git checkout v2.8.0-rc1

COPY tf_configure.bazelrc .tf_configure.bazelrc
RUN /usr/local/lib/bazel/bin/bazel build --config=opt //tensorflow/tools/pip_package:build_pip_package
RUN ./bazel-bin/tensorflow/tools/pip_package/build_pip_package /mnt

###
#
# Build GStreamer
#
###
FROM base-builder AS gst-builder

WORKDIR /build/gst

RUN python -m pip install PyGObject

RUN git config --global http.sslVerify false
RUN git clone https://gitlab.freedesktop.org/gstreamer/gstreamer.git
WORKDIR /build/gst/gstreamer
RUN git checkout 1.19.3
RUN meson -Dlibav=disabled -Dpython=enabled build
RUN ninja -C build

###
#
# Build CMake
#
###
FROM base-builder AS cmake-builder

WORKDIR /build/cmake

RUN curl -sSLO https://github.com/Kitware/CMake/releases/download/v3.20.1/cmake-3.20.1.tar.gz && tar xvzf cmake-3.20.1.tar.gz
WORKDIR /build/cmake/cmake-3.20.1
RUN ./bootstrap && make -j`nproc`

###
#
# Create final image
#
###
FROM base-builder

# Install GStreamer
COPY --from=gst-builder /build/gst/gstreamer /build/gst/gstreamer
WORKDIR /build/gst/gstreamer
RUN ninja -C build install

# Install Tensorflow
COPY --from=tf-builder /mnt/tensorflow-2.8.0rc1-cp37-cp37m-linux_x86_64.whl /mnt/tensorflow-2.8.0rc1-cp37-cp37m-linux_x86_64.whl
RUN python -m pip install /mnt/tensorflow-2.8.0rc1-cp37-cp37m-linux_x86_64.whl && rm /mnt/tensorflow-2.8.0rc1-cp37-cp37m-linux_x86_64.whl

# Install CMake
WORKDIR /build/cmake/cmake-3.20.1
COPY --from=cmake-builder /build/cmake/cmake-3.20.1 /build/cmake/cmake-3.20.1
RUN make install -C /build/cmake/cmake-3.20.1 && rm -rf /build/cmake

# Install Rust
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

RUN apt-get update && apt-get install -y \
        ffmpeg \
        htop \
        libx264-dev \
        yasm \
        libclang-dev \
        clang \
        && rm -rf /var/lib/apt/lists/*

# Install python modules
RUN python -m pip install \
        ray \
        notebook \
        tf2onnx \
        pandas \
        matplotlib \
        fabric2 \
        PyYaml \
	rich \
        opencv-python


WORKDIR /workspace
SHELL ["/bin/bash", "-c"]
RUN source /root/.cargo/env && rustup override set nightly

ENV LD_LIBRARY_PATH /usr/local/lib:/usr/local/lib/x86_64-linux-gnu:$LD_LIBRARY_PATH

