# CoVA: Exploiting Compressed-Domain Analysis to Accelerate Video Analytics

Current version of CoVA runs inside docker image, where this cloned git repository is mounted on `/workspace` inside the docker container.

#### Tested Environment

 - NVIDIA RTX 3090
 - Ubuntu 18.04
 - CUDA 11.5.1
 - Docker 20.10
 - Nvidia container toolkit 1.4.0

## 0. Clone current repository

```shell
git clone --recurse-submodule https://github.com/jinuhwang/CoVA
cd CoVA
# or if you already cloned without submodule, 
git submodule update --init --recursive
```

## 1. Installation

### 1.1 Setup docker image for CoVA

#### 1.1.1. Build docker image from source

1. Get `nvcr.io/nvidia/deepstream:6.0-devel` image from [NVIDIA NGC](https://ngc.nvidia.com/catalog/containers/nvidia:deepstream)

2. Get TensorRT 8.2.4.2 `DEB` package from NVIDIA [webpage](https://developer.nvidia.com/tensorrt-getting-started) and place it inside `./docker`

3. Build an image on top of deepstream

   ```shell
   cd docker
   
   # Builds the image for CoVA based on ./docker/Dockerfile
   ./build.sh
   ```

#### 1.1.2 or pull image from DockerHub

The docker image is not provided in terms of DeepStream LICENSE.

### 1.2. Launch and attach to the docker container

```shell
./launch.sh CONTAINER_NAME
```

The container should be launched with current cloned repository mounted on `/workspace`.

All the following steps should be done inside (attached to) the docker container.

### 1.3. Additional setup steps inside the container

#### 1.3.1. Download pre-trained model weights for YOLOv4

```shell
cd /workspace

# Download pretrained YOLOv4 weights
pushd third_parties/tensorrt_demos/yolo
./download_yolo.sh
popd

# Build custom Deepstream parser for YOLO
pushd third_parties/DeepStream-Yolo/nvdsinfer_custom_impl_Yolo
CUDA_VER=11.4 make
popd
```

#### 1.3.2. Install entropy decoder

```shell
cd /workspace

# Build modified version of FFmpeg
pushd third_parties/FFmpeg
./configure --enable-shared --disable-static
make -j`nproc` install
popd

# Build GStreamer plugin with modified decoder
pushd third_parties/gst-libav
meson build
ninja -C build install
popd

# Check the plugin is installed correctly.
gst-inspect-1.0 avdec_h264
```

The entropy decoder is built upon [FFmpeg](https://ffmpeg.org/).

Once patched `avdec_h264` is installed, it should work as entropy decoder (partial decoder) with the combination of `metapreprocess` element.

#### 1.3.3. Install GStreamer plugins

```shell
cd /workspace

# Install all required plugins
make install
```

##### 1.3.3.1. Main CoVA plugins (from `cova-rs/gst-plugins`)

* `metapreprocess`: Preprocess metadata extracted from entropy decoder 
* `bboxcc`: Transforms BlobNet mask into bounding box using connected component algorithm 
* `sorttracker`: Tracks the bounding boxes using [SORT](https://arxiv.org/abs/1602.00763) algorithm
* `cova`: Filters frames to decode based on the tracked objects
* [For training] `tfrecordsink`: Used to pack BlobNet training data into Tensorflow TFRecord format

##### 1.3.3.2. Other auxiliary plugins (from `gst-plugins`)

* `gopsplit`: Splits encoded video stream at the GoP boundary
* `maskcopy`: Copies BlobNet output mask from GPU memory to CPU memory
* `nvdsbbox / tcpprobe`: Extracts inference information from `nvinfer`

```shell
# Check plugins all correctly installed
gst-inspect-1.0 cova
gst-inspect-1.0 gopsplit
gst-inspect-1.0 maskcopy
gst-inspect-1.0 nvdsbbox
gst-inspect-1.0 tcpprobe
```

### 2. Running the pipeline

### 2.0. Download video file

We provide the two video streams for demonstration which are the two first dataset we used in our paper.

You can download them from the following Google drive [link](https://drive.google.com/drive/folders/1TDqxjdcQkgHzXqelQ3vCBk4ongLkGFwP?usp=sharing).

Provided scripts are written assuming videos are placed under `/workspace/data/video/` like the following. So consider placing them like the following:

- /workspace/data/video/amsterdam/day1.mp4
- /workspace/data/video/amsterdam/day2.mp4
- ...

Otherwise, specify the custom path later on.

### 2.1. Naive DNN-only pipeline

```shell
cd experiment/naive

# e.g., python launch.py /workspace/data/video/archie/day1.mp4 /workspace/baseline/archie/day1
python launch.py INPUT_PATH OUTPUT_DIR
```

First time running the pipeline will take a while for building TensorRT engine from onnx weight file.

Once the conversion is done, move the created engine file to predefined path so that the engine is directly loaded so that this step can be skipped next time.

```shell
mkdir -p /workspace/model/trt_model/rnn
mv model_b2_gpu0_fp16.engine /workspace/model/trt_model/rnn/yolov4_b2_fp16.engine
```

DNN-only pipeline is required for accuracy comparison of CoVA, but running the pipeline for all dataset we used takes a lot of time, so consider downloading the result from the following Google drive [link](https://drive.google.com/drive/folders/1Wsiln__czpr04epPNUDGUPAdRnmoaheJ?usp=sharing).

Provided scripts are written assuming baseline results are placed under `/workspace/data/baseline/` like the following.

- /workspace/data/baseline/amsterdam/day1/dnn.csv
- /workspace/data/baseline/amsterdam/day2/dnn.csv
- ...

Otherwise, specify the custom path later on.

### 2.2. CoVA pipeline

### 2.2.1. Getting BlobNet ONNX file

#### 2.2.1.1. Download pretrained weights

1. Download the pretrained model from the following Google drive [link](https://drive.google.com/drive/folders/1FFRVI37-SVruK2Lt0nKkkQ8JQDnKciEE?usp=sharing). 

2. Place the downloaded file under `/workspace/model/onnx_model/blobnet/`
3. Move on  to 2.2.2.

#### 2.2.1.2. or train model from scratch

##### 2.2.1.2.1. Cut the first few minutes of video to generate training data

```
# e.g., ffmpeg -i original.mp4 -to 0:20:00 -c:v copy train.mp4
ffmpeg -i INPUT_VIDEO -to TRAIN_DUR -c:v copy OUTPUT_VIDEO
```

##### 2.2.1.2.2. Generate background subtraction results for training labels

```shell
cd /workspace/utils
# Generate MoG background subtraction based foreground mask from the video
./generate-mog.py VIDEO_PATH MOG_PATH
```

##### 2.2.1.2.3. Generate training dataset used for BlobNet training

```shell
cd /workspace/utils
# Extracts compressed metadata from video and packs (metadata, MoG label) pairs into TFRecord format dataset
./generate-record.sh VIDEO_PATH MOG_PATH RECORD_PATH
```

##### 2.2.1.2.4. Training BlobNet

```shell
cd /workspace/utils
# Train BlobNet with Tensorflow and save it as frozen model
./train-blobnet.py RECORD_PATH FROZEN_PATH
```

Place the output frozen model directory under `/workspace/model/tf_model/blobnet`.

##### 2.2.1.2.5. Convert frozen model into ONNX format

```shell
cd /workspace/model
# The following command will generate onnx file
# From /workspace/model/tf_model to /workspace/model/onnx_model
python -m invoke tf2onnx FROZEN_PATH 
```

### 2.2.2. Convert frozen model into TensorRT engine

```shell
cd /workspace/model

# The following command will generate engine file
# From /workspace/model/onnx_model to /workspace/model/trt_model
python -m invoke onnx2trt ONNX_PATH
```

### 2.2.3 Launch CoVA pipeline

```shell
cd /workspace/experiment/cova
# e.g., python launch.py /workspace/data/video/amsterdam/day1.mp4 /workspace/data/cova/amsterdam/day1 amsterdam
python launch.py INPUT_PATH OUTPUT_DIR DATASET
```

You can configure number of entropy decoder / number of concurrent models / number of model batch size in the `config.yaml`. 

The structure of resulting output directory is as the following.

```
output_dir/
    track.csv (debugging purpose): Tracked objects in compressed domain
    dnn.csv (debugging purpose): Inferenced detection in pixel domain
    assoc.csv: Final CoVA results of moving objects
    stationary.csv: Final cova results of stationary objects
    out.txt: Logs filtering rates and elapsed time
```

You can use `htop` and `nvidia-smi dmon` to confirm the pipeline is running correctly by monitoring the CPU, memory, GPU SM, and NVDEC utilization.

### 2.2.4 Parsing CoVA result

```shell
cd /workspace/parse
# e.g., python launch.py amsterdam /worksapce/data/parsed/amsterdam
python launch.py DATASET OUTPUT_DIR
```

As a result, two files will be created in the `OUTPUT_DIR` which contain the result of binary predicate query of the target object. You can check the video at the returned timestamp (showed in nanosecond) to find the object appearing. 

The main results for elapsed time (for Figure 8), filtering rate (for Table 3) and accuracy metric (for Table 4)  will be provided on the stdout.

## Demo

#### BlobNet

* Pixel Domain FG Mask Extraction: [MoG](https://ieeexplore.ieee.org/document/1333992) based object detection
* Compressed Domain Mask Extraction: BlobNet based object detection

![demo/demo.gif](https://github.com/anonymous-cova/cova/blob/master/demo/demo.gif?raw=true)

### Issue

If you have any issue while running the script, please file an issue on the GitHub page or let us know by email (contact: [jwhwang@casys.kaist.ac.kr](jwhwang@casys.kaist.ac.kr) and we will investigate and fix the issue.