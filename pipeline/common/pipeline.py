import sys
import gi
gi.require_version('Gst', '1.0')
from timeit import default_timer as timer
from gi.repository import Gst, GLib

class Pipeline:
    def __init__(self):
        Gst.init(sys.argv)
        self.pipeline = Gst.Pipeline.new("pipeline")
        bus = self.pipeline.get_bus()
        bus.add_signal_watch()
        bus.connect("message", self.on_message)

        self.state = Gst.State.NULL
        self.loop = GLib.MainLoop()


    def start(self):
        ret = self.pipeline.set_state(Gst.State.PAUSED)
        if ret == Gst.StateChangeReturn.FAILURE:
            print("ERROR: Unable to set the pipeline to the playing state")
            sys.exit(1)
        self.loop.run()


    def create_elem(self, elem_name, **kwargs):
        elem = Gst.ElementFactory.make(elem_name)
        assert elem is not None
        for k, v in kwargs.items():
            elem.set_property(k.replace("_", "-"), v)
        self.pipeline.add(elem)
        return elem

    def create_and_append(self, prev_elem, elem_name, **kwargs):
        assert prev_elem is not None
        next_elem = self.create_elem(elem_name, **kwargs)
        prev_elem.link(next_elem)
        return  next_elem

    def prepend_queue(self, elem):
        queue = self.create_elem(
                "queue",
                max_size_buffers=self.queue_size,
                max_size_bytes=0,
                max_size_time=0
        )
        queue.link(elem)
        return queue

    def append_queue(self, elem):
        queue = self.create_elem(
                "queue",
                max_size_buffers=self.queue_size,
                max_size_bytes=0,
                max_size_time=0
        )
        elem.link(queue)
        return queue

    def append_sink(self, elem):
        sink = self.config['sink']
        kwargs = { 'sync': False }
        if sink != 'fakesink':
            kwargs['location'] = self.config['sink_location']
        self.create_and_append(elem, sink, **kwargs)


    def on_message(self, bus, message):
        t = message.type
        if t == Gst.MessageType.STREAM_START:
            self.start_time = timer()
            print('Started running pipeline')
        elif t == Gst.MessageType.ERROR:
            print('Error caught, terminating')
            self.loop.quit()
        elif message.src == self.pipeline:
            if t == Gst.MessageType.EOS:
                self.stop_time = timer()
                print('Ended running pipeline')
                self.terminate()
                self.loop.quit()

        # TODO: Fix termination
        #     elif t == Gst.MessageType.STATE_CHANGED:
        #         old, new, _ = Gst.Message.parse_state_changed(message)
        #         if old == Gst.State.READY and new == Gst.State.PAUSED:
        #             self.start_time = timer()
        #             print('Started Running Pipeline')

        if self.debug:
            print(f'{t} from {message.src}')
            if t == Gst.MessageType.EOS:
                print('EOS from', message.src)
            if t == Gst.MessageType.STREAM_STATUS:
                print(Gst.Message.parse_stream_status(message))

