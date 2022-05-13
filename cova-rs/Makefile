.PHONY: install

CUR_DIR:=$(shell dirname $(realpath $(firstword $(MAKEFILE_LIST))))

install:
	cargo build --release
	ln -sfn ${CUR_DIR}/target/release/libgstcova.so /usr/local/lib/x86_64-linux-gnu/gstreamer-1.0/libgstcova.so

.PHONY: install-debug
install-debug:
	cargo build
	ln -sfn ${CUR_DIR}/target/debug/libgstcova.so /usr/local/lib/x86_64-linux-gnu/gstreamer-1.0/libgstcova.so
