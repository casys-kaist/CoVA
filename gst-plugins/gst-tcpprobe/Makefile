CUDA_VER=11.4
CXX:= g++

SRCS:= gsttcpprobe.cpp

INCS:= $(wildcard *.h)
LIB:=libgsttcpprobe.so

NVDS_VERSION:=6.0

CFLAGS+= -fPIC -DDS_VERSION=\"6.0.0\" \
	 -I/usr/local/cuda-$(CUDA_VER)/include \
	 -I/opt/nvidia/deepstream/deepstream-$(NVDS_VERSION)/sources/includes \
	 -g -Og

GST_INSTALL_DIR?=/usr/local/lib/x86_64-linux-gnu/gstreamer-1.0
LIB_INSTALL_DIR?=/opt/nvidia/deepstream/deepstream-$(NVDS_VERSION)/lib/

LIBS := -shared -Wl,-no-undefined \
	-L/usr/local/cuda-$(CUDA_VER)/lib64/ -lcudart -ldl \
	-lnppc -lnppig -lnpps -lnppicc -lnppidei \
	-L$(LIB_INSTALL_DIR) -lnvdsgst_helper -lnvdsgst_meta -lnvds_meta \
	-lnvbufsurface -lnvbufsurftransform -lnppist \
	-Wl,-rpath,$(LIB_INSTALL_DIR)


OBJS:= $(SRCS:.cpp=.o)
PKGS:= gstreamer-1.0 gstreamer-base-1.0 gstreamer-video-1.0

CFLAGS+=$(shell pkg-config --cflags $(PKGS))
LIBS+=$(shell pkg-config --libs $(PKGS))

all: $(LIB)

%.o: %.cpp $(INCS) Makefile
	@echo $(CFLAGS)
	$(CXX) -c -o $@ $(CFLAGS) $<

$(LIB): $(OBJS) Makefile
	@echo $(CFLAGS)
	$(CXX) -o $@ $(OBJS) $(LIBS)

install: $(LIB)
	cp -rv $(LIB) $(GST_INSTALL_DIR)

clean:
	rm -rf $(OBJS) $(LIB)
