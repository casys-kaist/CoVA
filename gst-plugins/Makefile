# FIXME: make a link when building docker image
DS_INSTALL_DIR=/opt/nvidia/deepstream/deepstream/lib/gst-plugins
GST_INSTALL_DIR=/usr/local/lib/x86_64-linux-gnu/gstreamer-1.0

.PHONY: install
install:
	make -C gst-gopsplit install
	make -C gst-maskcopy install
	make -C gst-tcpprobe install
	make -C gst-nvdsbbox install
	ln -sfn  $(DS_INSTALL_DIR) $(GST_INSTALL_DIR)/deepstream

