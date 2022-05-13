#ifndef __GST_TCPPROBE_H__
#define __GST_TCPPROBE_H__

#include <gst/gst.h>
#include "gstnvdsmeta.h"
#include <stdio.h>
#include <gst/base/gstbasetransform.h>
#include <gst/video/video.h>

/* Package and library details required for plugin_init */
#define PACKAGE "probe filter"
#define VERSION "1.0"
#define LICENSE "MIT"
#define DESCRIPTION "Template of Base probe"
#define BINARY_PACKAGE "probe filter"
#define URL "https://github.com/anonymous-cova/cova"

G_BEGIN_DECLS
/* Standard boilerplate stuff */
typedef struct _GstTcpProbe GstTcpProbe;
typedef struct _GstTcpProbeClass GstTcpProbeClass;

/* Standard boilerplate stuff */
#define GST_TYPE_TCPPROBE (gst_tcpprobe_get_type())
#define GST_TCPPROBE(obj)                                                    \
  (G_TYPE_CHECK_INSTANCE_CAST((obj), GST_TYPE_TCPPROBE, GstTcpProbe))
#define GST_TCPPROBE_CLASS(klass)                                            \
  (G_TYPE_CHECK_CLASS_CAST((klass), GST_TYPE_TCPPROBE, GstTcpProbeClass))
#define GST_TCPPROBE_GET_CLASS(obj)                                          \
  (G_TYPE_INSTANCE_GET_CLASS((obj), GST_TYPE_TCPPROBE, GstTcpProbeClass))
#define GST_IS_TCPPROBE(obj)                                                 \
  (G_TYPE_CHECK_INSTANCE_TYPE((obj), GST_TYPE_TCPPROBE))
#define GST_IS_TCPPROBE_CLASS(klass)                                         \
  (G_TYPE_CHECK_CLASS_TYPE((klass), GST_TYPE_TCPPROBE))
#define GST_TCPPROBE_CAST(obj) ((GstTcpProbe *)(obj))

struct _GstTcpProbe {
  GstBaseTransform base_trans;

  guint32 socket;
  uint64_t count;

  guint32 port;
};

// Boiler plate stuff
struct _GstTcpProbeClass {
  GstBaseTransformClass parent_class;
};

GType gst_tcpprobe_get_type(void);

G_END_DECLS
#endif /* __GST_TCPPROBE */
