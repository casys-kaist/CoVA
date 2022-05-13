# Models
Specialized models
```
model_type/pipeline/dataset/object

model
|--tf_model                Directory containing frozen Tensorflow model
|    |
|    |--cova               CoVA related models
|
|--onnx_model              Directory containing ONNX converted model     (via tf2onnx)
|--trt_model               Directory containing TensorRT converted model (via onnx2trt)

```

### Requirements
Python `invoke` package

### Usage
```
inv tf2onnx cova/taipei-hires/car
inv onnx2trt cova/taipei-hires/car
inv tf2trt cova/taipei-hires/car
```
