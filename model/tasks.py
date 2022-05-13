import errno
import os
from invoke import task
from pathlib import Path

config = {
    "trt": "/usr/src/tensorrt/bin/trtexec",
    "path": {
        "tf": "tf_model",
        "onnx": "onnx_model",
        "trt": "trt_model",
    }
}


@task
def tf2onnx(ctx, model_path, cuda=0):
    model_path = Path(model_path).resolve()
    out_path = Path(str(model_path).replace(config["path"]["tf"], config["path"]["onnx"]))
    out_path = out_path.with_suffix('.onnx')

    out_path.parent.mkdir(exist_ok=True, parents=True)

    cmd = f"CUDA_VISIBLE_DEVICES={cuda} python -m tf2onnx.convert"
    cmd += f" --saved-model {str(model_path)} --output {str(out_path)}"
    cmd += f" --rename-inputs input --rename-outputs output"
    cmd += " --opset 12"

    ctx.run(cmd, echo=True)
    return out_path


@task
def onnx2trt(ctx, model_path, cuda=0, batch=512, build_only=False):
    bin_path = config["trt"]

    model_path = Path(model_path).resolve()
    out_path = Path(str(model_path).replace(config["path"]["onnx"], config["path"]["trt"]))
    out_path = out_path.with_suffix('.engine')
    out_path.parent.mkdir(exist_ok=True, parents=True)

    out_path = out_path.parent / f'{out_path.stem}_b{batch}.engine'

    input_shape = '3x180x80'

    cmd = f"CUDA_VISIBLE_DEVICES={cuda} {bin_path} --onnx={model_path} --verbose"
    cmd += f" --explicitBatch --minShapes=input:{batch}x{input_shape}"
    cmd += f" --optShapes=input:{batch}x{input_shape} --maxShapes=input:{batch}x{input_shape}"
    cmd += f" --workspace=8500 --fp16 --saveEngine={out_path}"
    if build_only:
        cmd += " --buildOnly"

    ctx.run(cmd, echo=True)
    return out_path


@task
def tf2trt(ctx, model_path, cuda=0, batch=512):
    onnx_path = tf2onnx(ctx, model_path, cuda)
    trt_path = onnx2trt(ctx, onnx_path, cuda, batch=batch)
