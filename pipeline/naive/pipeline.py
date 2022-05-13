import sys, os
import gi
gi.require_version('Gst', '1.0')
from gi.repository import Gst, GLib
from ..common import Pipeline

class NaivePipeline(Pipeline):
    def __init__(self, config, f=None, debug=False):
        super(NaivePipeline, self).__init__()
        self.f = f
        self.queue_size = config['queue_size']
        self.config = config
        self.debug = debug

        self.filesrc = Gst.ElementFactory.make("filesrc", "file-source")
        self.filesrc.set_property("location", config['input_file'])

        self.qtdemux = Gst.ElementFactory.make("qtdemux", "qt-demux")
        self.qtdemux.connect("pad-added", self.on_qtdemux_pad_added)

        self.pipeline.add(self.filesrc)
        self.pipeline.add(self.qtdemux)

        self.filesrc.link(self.qtdemux)

        self.nvdecs = []

    def init_upto_last(self):
        config = self.config
        last = config['last']

        nvdec = self.create_and_append(self.h264parse,
                    "nvv4l2decoder",
                    cudadec_memtype=0,
                    num_extra_surfaces=config['nvdec_num_extra_surfaces'],
                    drop_frame_interval=config['nvdec_drop_frame_interval']
                    )

        if config['nvdec_add_queue']:
            nvdec = self.append_queue(nvdec)

        if last == 'nvdec':
            return nvdec

        nvstreammux = self.create_elem(
                    "nvstreammux",
                    width=config['width'],
                    height=config['height'],
                    batch_size=config['dnn_batch_size'],
                    buffer_pool_size=config['dnn_pool_size'],
                    nvbuf_memory_type=2,
                    batched_push_timeout=config['dnn_batched_push_timeout'],
                    )

        nvdec.get_static_pad('src').link(nvstreammux.get_request_pad('sink_0'))

        if config['nvstreammux_dnn_add_queue']:
            nvstreammux = self.append_queue(nvstreammux)

        if last == 'nvstreammux_dnn':
            return nvstreammux

        nvinfer = self.create_and_append(
                    nvstreammux,
                    "nvinfer",
                    config_file_path=config['nvinfer_dnn_config']
                    )
        if config['nvinfer_dnn_add_queue']:
            nvinfer = self.append_queue(nvinfer)

        if last == 'nvinfer_dnn':
            return nvinfer

        nvstreamdemux = self.create_and_append(nvinfer, "nvstreamdemux")

        caps = Gst.Caps.from_string("video/x-raw(memory:NVMM),format=(string)NV12")
        capsfilter = self.create_elem("capsfilter", caps=caps)

        nvstreamdemux.get_request_pad(f"src_0").link(capsfilter.get_static_pad("sink"))
        if config['nvstreamdemux_dnn_add_queue']:
            capsfilter = self.append_queue(capsfilter)

        if last == 'nvstreamdemux_dnn':
            return capsfilter

        nvdsbbox = self.create_and_append( capsfilter, "nvdsbbox")

        if config['nvdsbbox_add_queue']:
            nvdsbbox = self.append_queue(nvdsbbox)

        if last == 'nvdsbbox':
            return nvdsbbox

        assert last == 'full'

        return nvdsbbox


    def terminate(self, force=False):
        elapsed_sec = self.stop_time - self.start_time
        print('Elapsed seconds:', elapsed_sec)
        if self.f is not None:
            print('Elapsed seconds:', elapsed_sec, file=self.f)

        if force:
            exit(0)

        # free resources
        if self.pipeline:
            self.pipeline.set_state(Gst.State.NULL)
            self.pipeline = None

    def on_qtdemux_pad_added(self, qtdemux, pad):
        # print('Hi', pad.caps().to_string())
        # print(qtdemux, pad, user_data)
        if pad.get_current_caps().to_string().startswith('video/x-h264'):
            self.h264parse = Gst.ElementFactory.make("h264parse", "h264-parse")
            self.h264parse.set_property("config-interval", -1)
            self.pipeline.add(self.h264parse)
            pad.link(self.h264parse.get_static_pad("sink"))

            last_elem = self.init_upto_last()
            self.append_sink(last_elem)

            self.pipeline.set_state(Gst.State.PLAYING)


if __name__ == '__main__':
    import argparse
    import yaml

    parser = argparse.ArgumentParser()
    parser.add_argument("CONFIG_FILE")
    parser.add_argument("--debug", action='store_true', default=False)
    args = parser.parse_args()

    config = yaml.safe_load(open(args.CONFIG_FILE))
    NaivePipeline(config, debug=args.debug).start()

