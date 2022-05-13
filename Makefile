MAKEFLAGS += -j3

.PHONY: install

install:
	$(MAKE) -C cova-rs install
	$(MAKE) -C gst-plugins install
	ln -svf /usr/lib/python3/dist-packages/gi/_gi.cpython-36m-x86_64-linux-gnu.so /usr/lib/python3/dist-packages/gi/_gi.cpython-37m-x86_64-linux-gnu.so
