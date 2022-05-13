#!/bin/bash
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"
IMAGE=jinwooh/cova
TAG=latest

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 CONTAINER_NAME"
    exit 0
fi

set -x

docker run -it --net=host \
    --cap-add=SYS_PTRACE --security-opt seccomp=unconfined \
    --gpus 'all,"capabilities=compute,utility,video"' \
    -v ${SCRIPT_DIR}/..:/workspace \
    --shm-size="47G" \
    --name $1 \
    ${IMAGE}:${TAG} bash
