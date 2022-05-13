// This software contains source code provided by NVIDIA Corporation.
#include "gstmaskcopy.h"
#include <fstream>
#include <iostream>
#include <ostream>
#include <sstream>
#include <string.h>
#include <string>
#include <sys/time.h>

#include "gst/gstcaps.h"
#include "gst/gstinfo.h"
#include "gst/gstmemory.h"
#include "gst/gstpadtemplate.h"
#include "gst/video/video-info.h"
#include "gstnvdsinfer.h"
#include "gstnvdsmeta.h"
#include "nvbufsurface.h"

GST_DEBUG_CATEGORY_STATIC(gst_maskcopy_debug);
#define GST_CAT_DEFAULT gst_maskcopy_debug

static GQuark _dsmeta_quark = 0;

/* Enum to identify properties */
enum {
  PROP_0,
  PROP_UNIQUE_ID,
  PROP_GPU_DEVICE_ID,
  PROP_TIMESTEP,
};

/* Default values for properties */
#define DEFAULT_UNIQUE_ID 0
#define DEFAULT_GPU_ID 0
#define DEFAULT_TIMESTEP 4

#define GST_CAPS_FEATURE_MEMORY_NVMM "memory:NVMM"
static GstStaticPadTemplate gst_maskcopy_sink_template =
    GST_STATIC_PAD_TEMPLATE("sink", GST_PAD_SINK, GST_PAD_ALWAYS,
                            GST_STATIC_CAPS(GST_VIDEO_CAPS_MAKE_WITH_FEATURES(
                                "memory:NVMM", "{ NV12, RGBA }")));

static GstStaticPadTemplate gst_maskcopy_src_template =
    GST_STATIC_PAD_TEMPLATE("src", GST_PAD_SRC, GST_PAD_ALWAYS,
                            GST_STATIC_CAPS(GST_VIDEO_CAPS_MAKE("GRAY8")));

G_DEFINE_TYPE(GstMaskCopy, gst_maskcopy, GST_TYPE_BASE_TRANSFORM);

static void gst_maskcopy_set_property(GObject *object, guint prop_id,
                                      const GValue *value, GParamSpec *pspec);
static void gst_maskcopy_get_property(GObject *object, guint prop_id,
                                      GValue *value, GParamSpec *pspec);

static gboolean gst_maskcopy_start(GstBaseTransform *btrans);
static gboolean gst_maskcopy_stop(GstBaseTransform *btrans);

static GstFlowReturn gst_maskcopy_transform(GstBaseTransform *btrans,
                                            GstBuffer *inbuf,
                                            GstBuffer *outbuf);
static gboolean gst_maskcopy_set_caps(GstBaseTransform *btrans, GstCaps *incaps,
                                      GstCaps *outcaps);
static GstCaps *gst_maskcopy_transform_caps(GstBaseTransform *trans,
                                            GstPadDirection direction,
                                            GstCaps *caps, GstCaps *filter);
static gboolean gst_maskcopy_transform_size(GstBaseTransform *btrans,
                                            GstPadDirection direction,
                                            GstCaps *caps, gsize size,
                                            GstCaps *othercaps,
                                            gsize *othersize);

/* Install properties, set sink and src pad capabilities, override the required
 * functions of the base class, These are common to all instances of the
 * element.
 */
static void gst_maskcopy_class_init(GstMaskCopyClass *klass) {
  GObjectClass *gobject_class;
  GstElementClass *gstelement_class;
  GstBaseTransformClass *gstbasetransform_class;
  gobject_class = (GObjectClass *)klass;
  gstelement_class = (GstElementClass *)klass;
  gstbasetransform_class = (GstBaseTransformClass *)klass;

  /* Overide base class functions */
  gobject_class->set_property = GST_DEBUG_FUNCPTR(gst_maskcopy_set_property);
  gobject_class->get_property = GST_DEBUG_FUNCPTR(gst_maskcopy_get_property);

  gstbasetransform_class->start = GST_DEBUG_FUNCPTR(gst_maskcopy_start);
  gstbasetransform_class->stop = GST_DEBUG_FUNCPTR(gst_maskcopy_stop);

  gstbasetransform_class->set_caps = GST_DEBUG_FUNCPTR(gst_maskcopy_set_caps);

  gstbasetransform_class->transform = GST_DEBUG_FUNCPTR(gst_maskcopy_transform);
  gstbasetransform_class->transform_size =
      GST_DEBUG_FUNCPTR(gst_maskcopy_transform_size);
  gstbasetransform_class->transform_caps =
      GST_DEBUG_FUNCPTR(gst_maskcopy_transform_caps);

  gstbasetransform_class->passthrough_on_same_caps = FALSE;

  /* Install properties */
  g_object_class_install_property(
      gobject_class, PROP_UNIQUE_ID,
      g_param_spec_uint(
          "unique-id", "Unique ID",
          "Unique ID for the element. Can be used to identify output of the"
          " element",
          0, G_MAXUINT, DEFAULT_UNIQUE_ID,
          GParamFlags(G_PARAM_READWRITE | G_PARAM_STATIC_STRINGS |
                      GST_PARAM_MUTABLE_READY)));

  g_object_class_install_property(
      gobject_class, PROP_GPU_DEVICE_ID,
      g_param_spec_uint("gpu-id", "Set GPU Device ID", "Set GPU Device ID", 0,
                        G_MAXUINT, 0,
                        GParamFlags(G_PARAM_READWRITE | G_PARAM_STATIC_STRINGS |
                                    GST_PARAM_MUTABLE_READY)));

  g_object_class_install_property(
      gobject_class, PROP_TIMESTEP,
      g_param_spec_uint(
          "timestep", "Timestep",
          "Number of frames stacked in temporal domain.", 1, G_MAXUINT,
          DEFAULT_TIMESTEP,
          (GParamFlags)(G_PARAM_READWRITE | G_PARAM_STATIC_STRINGS)));

  /* Set sink and src pad capabilities */
  gst_element_class_add_pad_template(
      gstelement_class,
      gst_static_pad_template_get(&gst_maskcopy_src_template));
  gst_element_class_add_pad_template(
      gstelement_class,
      gst_static_pad_template_get(&gst_maskcopy_sink_template));

  /* Set metadata describing the element */
  gst_element_class_set_details_simple(
      gstelement_class, "maskcopy", "maskcopy",
      "GStreamer plugin to copy segmentation result from Deepstream",
      "Jinwoo Hwang<jwhwang@casys.kaist.ac.kr>");
}

static void gst_maskcopy_init(GstMaskCopy *maskcopy) {
  maskcopy->sinkcaps =
      gst_static_pad_template_get_caps(&gst_maskcopy_sink_template);
  maskcopy->srccaps =
      gst_static_pad_template_get_caps(&gst_maskcopy_src_template);

  /* Initialize all property variables to default values */
  maskcopy->unique_id = DEFAULT_UNIQUE_ID;
  maskcopy->gpu_id = DEFAULT_GPU_ID;
  maskcopy->timestep = DEFAULT_TIMESTEP;

  maskcopy->cuda_mem_type = NVBUF_MEM_CUDA_DEVICE;

  /* This quark is required to identify NvDsMeta when iterating through
   * the buffer metadatas */
  if (!_dsmeta_quark)
    _dsmeta_quark = g_quark_from_static_string(NVDS_META_STRING);
}

/* Function called when a property of the element is set. Standard boilerplate.
 */
static void gst_maskcopy_set_property(GObject *object, guint prop_id,
                                      const GValue *value, GParamSpec *pspec) {
  GstMaskCopy *maskcopy = GST_MASKCOPY(object);
  switch (prop_id) {
  case PROP_UNIQUE_ID:
    maskcopy->unique_id = g_value_get_uint(value);
    break;
  case PROP_GPU_DEVICE_ID:
    maskcopy->gpu_id = g_value_get_uint(value);
    break;
  case PROP_TIMESTEP:
    maskcopy->timestep = g_value_get_uint(value);
    break;
  default:
    G_OBJECT_WARN_INVALID_PROPERTY_ID(object, prop_id, pspec);
    break;
  }
}

/* Function called when a property of the element is requested. Standard
 * boilerplate.
 */
static void gst_maskcopy_get_property(GObject *object, guint prop_id,
                                      GValue *value, GParamSpec *pspec) {
  GstMaskCopy *maskcopy = GST_MASKCOPY(object);
  switch (prop_id) {
  case PROP_UNIQUE_ID:
    g_value_set_uint(value, maskcopy->unique_id);
    break;
  case PROP_GPU_DEVICE_ID:
    g_value_set_uint(value, maskcopy->gpu_id);
    break;
  case PROP_TIMESTEP:
    g_value_set_uint(value, maskcopy->timestep);
    break;
  default:
    G_OBJECT_WARN_INVALID_PROPERTY_ID(object, prop_id, pspec);
    break;
  }
}

/**
 * Initialize all resources and start the output thread
 */
static gboolean gst_maskcopy_start(GstBaseTransform *btrans) {
  GstMaskCopy *maskcopy = GST_MASKCOPY(btrans);
  GST_DEBUG_OBJECT(maskcopy, "gst_maskcopy_start\n");
  return TRUE;
}

/**
 * Stop the output thread and free up all the resources
 */
static gboolean gst_maskcopy_stop(GstBaseTransform *btrans) {
  GstMaskCopy *maskcopy = GST_MASKCOPY(btrans);

  GST_DEBUG_OBJECT(maskcopy, "gst_maskcopy_stop\n");
  return TRUE;
}

/**
 * Called when source / sink pad capabilities have been negotiated.
 */
static void copymask(int *mask, unsigned char *buffer, int height, int width) {
  for (int pix_id = 0; pix_id < width * height; pix_id++) {
    buffer[pix_id] = mask[pix_id] + 1;
  }
}

/**
 * Called when the plugin works in non-passthough mode
 */
static GstFlowReturn gst_maskcopy_transform(GstBaseTransform *btrans,
                                            GstBuffer *inbuf,
                                            GstBuffer *outbuf) {
  GstMaskCopy *maskcopy = GST_MASKCOPY(btrans);
  GstFlowReturn flow_ret = GST_FLOW_OK;
  gpointer state = NULL;
  gboolean of_metadata_found = FALSE;
  GstMeta *gst_meta = NULL;
  NvDsMeta *dsmeta = NULL;
  NvDsBatchMeta *batch_meta = NULL;
  guint i = 0;

  if (cudaSetDevice(maskcopy->gpu_id) != cudaSuccess) {
    g_printerr("Error: failed to set GPU to %d\n", maskcopy->gpu_id);
    return GST_FLOW_ERROR;
  }

  while ((gst_meta = gst_buffer_iterate_meta(inbuf, &state))) {
    if (gst_meta_api_type_has_tag(gst_meta->info->api, _dsmeta_quark)) {
      dsmeta = (NvDsMeta *)gst_meta;
      if (dsmeta->meta_type == NVDS_BATCH_GST_META) {
        batch_meta = (NvDsBatchMeta *)dsmeta->meta_data;
        break;
      }
    }
  }

  if (batch_meta == NULL) {
    g_print(
        "batch_meta not found, skipping optical flow visual draw execution\n");
    return GST_FLOW_ERROR;
  }

  for (i = 0; i < batch_meta->num_frames_in_batch; i++) {
    NvDsFrameMeta *frame_meta =
        nvds_get_nth_frame_meta(batch_meta->frame_meta_list, i);
    if (frame_meta->frame_user_meta_list) {
      NvDsFrameMetaList *fmeta_list = NULL;
      NvDsUserMeta *of_user_meta = NULL;

      for (fmeta_list = frame_meta->frame_user_meta_list; fmeta_list != NULL;
           fmeta_list = fmeta_list->next) {
        of_user_meta = (NvDsUserMeta *)fmeta_list->data;
        if (of_user_meta &&
            of_user_meta->base_meta.meta_type == NVDSINFER_SEGMENTATION_META) {
          NvDsInferSegmentationMeta *segmeta =
              (NvDsInferSegmentationMeta *)(of_user_meta->user_meta_data);
          GST_TRACE("classes/width/height=%d/%d/%d\n", segmeta->classes,
                    segmeta->width, segmeta->height);
          GstMapInfo map;
          if (!gst_buffer_map(outbuf, &map, GST_MAP_WRITE)) {
            g_print("%s output buf map failed\n", __func__);
            return GST_FLOW_ERROR;
          }
          copymask(segmeta->class_map, map.data, segmeta->height,
                   segmeta->width);
          of_metadata_found = TRUE;
          gst_buffer_unmap(outbuf, &map);
        }
      }
    }
  }

  if (of_metadata_found == FALSE) {
    GST_WARNING_OBJECT(maskcopy, "SEG METADATA NOT FOUND\n");
  }
  return flow_ret;
}

static gboolean gst_maskcopy_set_caps(GstBaseTransform *btrans, GstCaps *incaps,
                                      GstCaps *outcaps) {
  GstMaskCopy *maskcopy = GST_MASKCOPY(btrans);

  /* Save the input video information, since this will be required later. */
  gst_video_info_from_caps(&maskcopy->video_info, incaps);
  return TRUE;
}

static GstCaps *gst_maskcopy_transform_caps(GstBaseTransform *btrans,
                                            GstPadDirection direction,
                                            GstCaps *caps, GstCaps *filter) {
  GstMaskCopy *maskcopy = GST_MASKCOPY(btrans);

  GstCaps *new_caps = NULL;
  GstCapsFeatures *feature = NULL;

  if (direction == GST_PAD_SRC) {
    new_caps = gst_static_pad_template_get_caps(&gst_maskcopy_sink_template);
    /* new_caps = gst_caps_copy(caps); */
    /* feature = gst_caps_features_new("memory:NVMM", NULL); */
    /* gst_caps_set_features(new_caps, 0, feature); */
  } else if (direction == GST_PAD_SINK) {
    // FIXME: Do I need to unref this?
    new_caps = gst_static_pad_template_get_caps(&gst_maskcopy_src_template);
    if (gst_caps_is_fixed(caps)) {
      new_caps = gst_caps_make_writable(new_caps);
      GstVideoInfo video_info;
      gst_video_info_from_caps(&video_info, caps);

      int width = video_info.width;
      int height = video_info.height / maskcopy->timestep;
      gst_caps_set_simple(new_caps, "width", G_TYPE_INT, width, "height",
                          G_TYPE_INT, height, NULL);
    }
  }
  GST_LOG("Caps transformed from %" GST_PTR_FORMAT " %" GST_PTR_FORMAT, caps,
          new_caps);

  return new_caps;
}

static gboolean gst_maskcopy_transform_size(GstBaseTransform *btrans,
                                            GstPadDirection direction,
                                            GstCaps *caps, gsize size,
                                            GstCaps *othercaps,
                                            gsize *othersize) {
  GstMaskCopy *maskcopy = GST_MASKCOPY(btrans);
  *othersize = maskcopy->video_info.width * maskcopy->video_info.height / 4;
  return true;
}

/**
 * Boiler plate for registering a plugin and an element.
 */
static gboolean maskcopy_plugin_init(GstPlugin *plugin) {
  GST_DEBUG_CATEGORY_INIT(gst_maskcopy_debug, "maskcopy", 0, "maskcopy plugin");

  return gst_element_register(plugin, "maskcopy", GST_RANK_PRIMARY,
                              GST_TYPE_MASKCOPY);
}

GST_PLUGIN_DEFINE(GST_VERSION_MAJOR, GST_VERSION_MINOR, maskcopy, DESCRIPTION,
                  maskcopy_plugin_init, "5.0", LICENSE, BINARY_PACKAGE, URL)
