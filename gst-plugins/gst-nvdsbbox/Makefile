CUDA_VER=11.4
CXX:= g++

SRCS:= gstnvdsbbox.cpp

INCS:= $(wildcard *.h)
LIB:=libgstnvdsbbox.so

NVDS_VERSION:=6.0

CFLAGS+= -fPIC -DDS_VERSION=\"6.0.0\" \
	 -I/usr/local/cuda-$(CUDA_VER)/include \
	 -I/opt/nvidia/deepstream/deepstream-$(NVDS_VERSION)/sources/includes \
	 -I../../cova-rs/nvdsbbox \
	 -g -Og

GST_INSTALL_DIR?=/usr/local/lib/x86_64-linux-gnu/gstreamer-1.0
LIB_INSTALL_DIR?=/opt/nvidia/deepstream/deepstream-$(NVDS_VERSION)/lib/

LIBS := -shared -Wl,-no-undefined \
	-L/usr/local/cuda-$(CUDA_VER)/lib64/ \
	-L$(LIB_INSTALL_DIR) -lnvdsgst_helper -lnvdsgst_meta -lnvds_meta \
	-lnvbufsurface -lnvbufsurftransform -lnppist \
	-L../../cova-rs/target/debug -L../../cova-rs/target/release -lnvdsbbox \
	-ldl -lpthread \
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
