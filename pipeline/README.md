There should be two files in each directory

* `pipeline.py:` Python script to build and launch GStreamer pipeline
* `config.yaml`: Default configuration file for the pipeline



One can test out the pipeline with

```
python pipeline.py config.yaml
or with debug flag
GST_DEBUG=3 python pipeline.py config.yaml
```


Note that the `analysis-aggregator` should manually be launched.

For the ones looking for full working example, look into `experiment` directory in the root directory
