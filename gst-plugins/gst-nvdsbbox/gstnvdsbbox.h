#ifndef __GST_NVDSBBOX_H__
#define __GST_NVDSBBOX_H__

#include "gstnvdsmeta.h"
#include "nvdsbbox.h"
#include <gst/base/gstbasetransform.h>
#include <gst/gst.h>
#include <gst/video/video.h>
#include <gst/gstbuffer.h>
#include <gst/gstcaps.h>
#include <gst/gstmemory.h>
#include <gst/gstvalue.h>
#include <stdio.h>

/* Package and library details required for plugin_init */
#define PACKAGE "CoVA"
#define VERSION "1.0"
#define LICENSE "MIT"
#define DESCRIPTION "Template of Base probe"
#define BINARY_PACKAGE "nvdsbbox"
#define URL "https://github.com/casys-kaist-internal/cova.code"

G_BEGIN_DECLS
/* Standard boilerplate stuff */
typedef struct _GstNvdsBbox GstNvdsBbox;
typedef struct _GstNvdsBboxClass GstNvdsBboxClass;

/* Standard boilerplate stuff */
#define GST_TYPE_NVDSBBOX (gst_nvdsbbox_get_type())
#define GST_NVDSBBOX(obj)                                                      \
  (G_TYPE_CHECK_INSTANCE_CAST((obj), GST_TYPE_NVDSBBOX, GstNvdsBbox))
#define GST_NVDSBBOX_CLASS(klass)                                              \
  (G_TYPE_CHECK_CLASS_CAST((klass), GST_TYPE_NVDSBBOX, GstNvdsBboxClass))
#define GST_NVDSBBOX_GET_CLASS(obj)                                            \
  (G_TYPE_INSTANCE_GET_CLASS((obj), GST_TYPE_NVDSBBOX, GstNvdsBboxClass))
#define GST_IS_NVDSBBOX(obj)                                                   \
  (G_TYPE_CHECK_INSTANCE_TYPE((obj), GST_TYPE_NVDSBBOX))
#define GST_IS_NVDSBBOX_CLASS(klass)                                           \
  (G_TYPE_CHECK_CLASS_TYPE((klass), GST_TYPE_NVDSBBOX))
#define GST_NVDSBBOX_CAST(obj) ((GstNvdsBbox *)(obj))

struct _GstNvdsBbox {
  GstBaseTransform base_trans;
};

// Boiler plate stuff
struct _GstNvdsBboxClass {
  GstBaseTransformClass parent_class;
};

GType gst_nvdsbbox_get_type(void);

G_END_DECLS
#endif /* __GST_NVDSBBOX */
