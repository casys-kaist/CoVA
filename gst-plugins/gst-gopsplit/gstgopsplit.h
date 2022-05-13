#ifndef __GST_GOPSPLIT_H__
#define __GST_GOPSPLIT_H__

#include <gst/gst.h>

/* Package and library details required for plugin_init */
#define PACKAGE "gopsplit"
#define VERSION "1.0"
#define LICENSE "LGPL"
#define DESCRIPTION "Plugin for gopsplit"
#define BINARY_PACKAGE "GopSplit"
#define URL "https://github.com/kaist-casys-internal/xvdec"

G_BEGIN_DECLS

/* Standard boilerplate stuff */
typedef struct _GstGopSplit GstGopSplit;
typedef struct _GstGopSplitClass GstGopSplitClass;

/* Standard boilerplate stuff */
#define GST_TYPE_GOPSPLIT (gst_gopsplit_get_type())
#define GST_GOPSPLIT(obj)                                                         \
  (G_TYPE_CHECK_INSTANCE_CAST((obj), GST_TYPE_GOPSPLIT, GstGopSplit))
#define GST_GOPSPLIT_CLASS(klass)                                                 \
  (G_TYPE_CHECK_CLASS_CAST((klass), GST_TYPE_GOPSPLIT, GstGopSplitClass))
#define GST_GOPSPLIT_GET_CLASS(obj)                                               \
  (G_TYPE_INSTANCE_GET_CLASS((obj), GST_TYPE_GOPSPLIT, GstGopSplitClass))
#define GST_IS_GOPSPLIT(obj) (G_TYPE_CHECK_INSTANCE_TYPE((obj), GST_TYPE_GOPSPLIT))
#define GST_IS_GOPSPLIT_CLASS(klass)                                              \
  (G_TYPE_CHECK_CLASS_TYPE((klass), GST_TYPE_GOPSPLIT))
#define GST_GOPSPLIT_CAST(obj) ((GstGopSplit *)(obj))

struct _GstGopSplit
{
  GstElement element;

  GstPad *sinkpad, *srcpad;

  /* properties for request pad */
  GHashTable     *pad_indexes;
  guint           next_pad_index;

  gboolean silent;

  /* buffer */
  GList* gops; // gops will contain bufs
  guint n_gops;
  GList* bufs;
};

// Boiler plate stuff
struct _GstGopSplitClass {
  GstElementClass parent_class;
};

GType gst_gopsplit_get_type(void);

G_END_DECLS

#endif /* __GST_GOPSPLIT_H__ */