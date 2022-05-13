// This software contains source code provided by NVIDIA Corporation.
#include "gstnvdsbbox.h"
#include <fstream>
#include <iostream>
#include <ostream>
#include <sstream>
#include <string.h>
#include <string>
#include <sys/time.h>
#include <unistd.h>

GST_DEBUG_CATEGORY_STATIC(gst_nvdsbbox_debug);
#define GST_CAT_DEFAULT gst_nvdsbbox_debug

/* Enum to identify properties */
enum { PROP_0 };

static GstStaticPadTemplate gst_nvdsbbox_sink_template =
    GST_STATIC_PAD_TEMPLATE("sink", GST_PAD_SINK, GST_PAD_ALWAYS,
                            GST_STATIC_CAPS_ANY);

static GstStaticPadTemplate gst_nvdsbbox_src_template = GST_STATIC_PAD_TEMPLATE(
    "src", GST_PAD_SRC, GST_PAD_ALWAYS,
    GST_STATIC_CAPS(
        "bbox,width=(int)[0,2147483647],height=(int)[0,2147483647]"));

/* Define our element type. Standard GObject/GStreamer boilerplate stuff */
#define gst_nvdsbbox_parent_class parent_class
G_DEFINE_TYPE(GstNvdsBbox, gst_nvdsbbox, GST_TYPE_BASE_TRANSFORM);

static void gst_nvdsbbox_set_property(GObject *object, guint prop_id,
                                      const GValue *value, GParamSpec *pspec);
static void gst_nvdsbbox_get_property(GObject *object, guint prop_id,
                                      GValue *value, GParamSpec *pspec);

static gboolean gst_nvdsbbox_start(GstBaseTransform *btrans);
static gboolean gst_nvdsbbox_stop(GstBaseTransform *btrans);

static GstFlowReturn gst_nvdsbbox_transform(GstBaseTransform *btrans,
                                            GstBuffer *inbuf,
                                            GstBuffer *outbuf);

static gboolean gst_nvdsbbox_sink_event(GstPad *pad, GstObject *parent,
                                        GstEvent *event);

static GstCaps *gst_nvdsbbox_transform_caps(GstBaseTransform *btrans,
                                            GstPadDirection direction,
                                            GstCaps *caps, GstCaps *filter);

static gboolean gst_nvdsbbox_transform_size(GstBaseTransform *btrans,
                                            GstPadDirection direction,
                                            GstCaps *caps, gsize size,
                                            GstCaps *othercaps,
                                            gsize *othersize);

/* Install properties, set sink and src pad capabilities, override the required
 * functions of the base class, These are common to all instances of the
 * element.
 */
static void gst_nvdsbbox_class_init(GstNvdsBboxClass *klass) {
  GObjectClass *gobject_class;
  GstElementClass *gstelement_class;
  GstBaseTransformClass *gstbasetransform_class;

  gobject_class = (GObjectClass *)klass;
  gstelement_class = (GstElementClass *)klass;
  gstbasetransform_class = (GstBaseTransformClass *)klass;

  /* Overide base class functions */
  gobject_class->set_property = GST_DEBUG_FUNCPTR(gst_nvdsbbox_set_property);
  gobject_class->get_property = GST_DEBUG_FUNCPTR(gst_nvdsbbox_get_property);

  gstbasetransform_class->start = GST_DEBUG_FUNCPTR(gst_nvdsbbox_start);
  gstbasetransform_class->stop = GST_DEBUG_FUNCPTR(gst_nvdsbbox_stop);
  gstbasetransform_class->transform_caps =
      GST_DEBUG_FUNCPTR(gst_nvdsbbox_transform_caps);
  gstbasetransform_class->transform_size =
      GST_DEBUG_FUNCPTR(gst_nvdsbbox_transform_size);

  gstbasetransform_class->transform = GST_DEBUG_FUNCPTR(gst_nvdsbbox_transform);

  /* Set sink and src pad capabilities */
  gst_element_class_add_pad_template(
      gstelement_class,
      gst_static_pad_template_get(&gst_nvdsbbox_src_template));
  gst_element_class_add_pad_template(
      gstelement_class,
      gst_static_pad_template_get(&gst_nvdsbbox_sink_template));

  /* Set metadata describing the element */
  gst_element_class_set_details_simple(
      gstelement_class, "NVDS BBox Plugin", "Deepstream bounding box exporter",
      "Export bounding box information from Deepstream", "Jinwoo Hwang");
}

static void gst_nvdsbbox_init(GstNvdsBbox *nvdsbbox) {
  GstBaseTransform *btrans = GST_BASE_TRANSFORM(nvdsbbox);

  gst_base_transform_set_in_place(GST_BASE_TRANSFORM(btrans), FALSE);
  gst_base_transform_set_passthrough(GST_BASE_TRANSFORM(btrans), FALSE);
}

/* Function called when a property of the element is set. Standard boilerplate.
 */
static void gst_nvdsbbox_set_property(GObject *object, guint prop_id,
                                      const GValue *value, GParamSpec *pspec) {
  GstNvdsBbox *nvdsbbox = GST_NVDSBBOX(object);
  switch (prop_id) {
  default:
    G_OBJECT_WARN_INVALID_PROPERTY_ID(object, prop_id, pspec);
    break;
  }
}

/* Function called when a property of the element is requested. Standard
 * boilerplate.
 */
static void gst_nvdsbbox_get_property(GObject *object, guint prop_id,
                                      GValue *value, GParamSpec *pspec) {
  GstNvdsBbox *nvdsbbox = GST_NVDSBBOX(object);

  switch (prop_id) {
  default:
    G_OBJECT_WARN_INVALID_PROPERTY_ID(object, prop_id, pspec);
    break;
  }
}

/**
 * Initialize all resources and start the output thread
 */
static gboolean gst_nvdsbbox_start(GstBaseTransform *btrans) {
  GstNvdsBbox *nvdsbbox = GST_NVDSBBOX(btrans);
  return TRUE;
error:
  return FALSE;
}

/**
 * Stop the output thread and free up all the resources
 */
static gboolean gst_nvdsbbox_stop(GstBaseTransform *btrans) {
  GstNvdsBbox *nvdsbbox = GST_NVDSBBOX(btrans);

  return TRUE;
}

/**
 * Called when element recieves an input buffer from upstream element.
 */
static GstFlowReturn gst_nvdsbbox_transform(GstBaseTransform *btrans,
                                            GstBuffer *inbuf,
                                            GstBuffer *outbuf) {

  GstNvdsBbox *nvdsbbox = GST_NVDSBBOX(btrans);

  NvDsFrameMeta *frame_meta = NULL;
  NvDsObjectMeta *obj_meta = NULL;
  NvDsClassifierMeta *class_meta = NULL;
  NvDsLabelInfo *label_meta = NULL;
  NvDsMetaList *l_frame = NULL;
  NvDsMetaList *l_obj = NULL;

  NvDsBatchMeta *batch_meta = gst_buffer_get_nvds_batch_meta(inbuf);

  bboxes_t *bboxes = bboxes_new();

  for (l_frame = batch_meta->frame_meta_list; l_frame != NULL;
       l_frame = l_frame->next) {
    frame_meta = (NvDsFrameMeta *)(l_frame->data);

    uint64_t timestamp = frame_meta->buf_pts;
    for (l_obj = frame_meta->obj_meta_list; l_obj != NULL;
         l_obj = l_obj->next) {

      obj_meta = (NvDsObjectMeta *)(l_obj->data);
      float left = obj_meta->detector_bbox_info.org_bbox_coords.left;
      float top = obj_meta->detector_bbox_info.org_bbox_coords.top;
      float width = obj_meta->detector_bbox_info.org_bbox_coords.width;
      float height = obj_meta->detector_bbox_info.org_bbox_coords.height;
      int class_id = obj_meta->class_id;
      float confidence = obj_meta->confidence;

      bboxes_add(bboxes, left, top, width, height, timestamp, class_id,
                 confidence);
    }
  }

  const int BUFFER_LEN = 1000000;
  uint8_t buffer[BUFFER_LEN];
  int len = bboxes_end(bboxes, buffer, BUFFER_LEN);
  GstMemory* memory = gst_allocator_alloc(NULL, len, NULL);
  gst_buffer_replace_all_memory(outbuf, memory);
  gst_buffer_set_size(outbuf, len);

  GstMapInfo map;
  gst_buffer_map(outbuf, &map, GST_MAP_WRITE);
  memcpy(map.data, buffer, len);
  GST_TRACE_OBJECT(nvdsbbox, "Serialized bounding boxes into %d bytes", len);

  return GST_FLOW_OK;

error:
  return GST_FLOW_ERROR;
}
static GstCaps *gst_nvdsbbox_transform_caps(GstBaseTransform *btrans,
                                            GstPadDirection direction,
                                            GstCaps *caps, GstCaps *filter) {
  GstNvdsBbox *nvdsbbox = GST_NVDSBBOX(btrans);

  GstCaps *new_caps = NULL;
  GstCapsFeatures *feature = NULL;

  int width, height;

  if (direction == GST_PAD_SRC) {
    new_caps = gst_static_pad_template_get_caps(&gst_nvdsbbox_sink_template);
  } else if (direction == GST_PAD_SINK) {
    GstStructure *structure;
    structure = gst_caps_get_structure(caps, 0);
    new_caps = gst_static_pad_template_get_caps(&gst_nvdsbbox_src_template);
    if (gst_structure_get_int(structure, "width", &width)) {
      new_caps = gst_caps_make_writable(new_caps);
      gst_structure_get_int(structure, "height", &height);
      gst_caps_set_simple(new_caps, "width", G_TYPE_INT, width, "height",
                          G_TYPE_INT, height, NULL);
    }
  }
  GST_LOG("Caps transformed from %" GST_PTR_FORMAT " to %" GST_PTR_FORMAT, caps,
          new_caps);

  return new_caps;
}

static gboolean gst_nvdsbbox_transform_size(GstBaseTransform *btrans,
                                            GstPadDirection direction,
                                            GstCaps *caps, gsize size,
                                            GstCaps *othercaps,
                                            gsize *othersize) {

  *othersize = 1 << (10 + 2);
  return TRUE;
}
/**
 * Boiler plate for registering a plugin and an element.
 */
static gboolean nvdsbbox_plugin_init(GstPlugin *plugin) {
  GST_DEBUG_CATEGORY_INIT(gst_nvdsbbox_debug, "nvdsbbox", 0, "nvdsbbox plugin");

  return gst_element_register(plugin, "nvdsbbox", GST_RANK_PRIMARY,
                              GST_TYPE_NVDSBBOX);
}

GST_PLUGIN_DEFINE(GST_VERSION_MAJOR, GST_VERSION_MINOR, nvdsbbox, DESCRIPTION,
                  nvdsbbox_plugin_init, VERSION, LICENSE, BINARY_PACKAGE, URL)
