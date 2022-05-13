#include "gsttcpprobe.h"
#include <fstream>
#include <iostream>
#include <ostream>
#include <sstream>
#include <string.h>
#include <string>
#include <sys/time.h>
#include <unistd.h>

#include <netinet/in.h>
#include <sys/socket.h>
#include <arpa/inet.h>

GST_DEBUG_CATEGORY_STATIC(gst_tcpprobe_debug);
#define GST_CAT_DEFAULT gst_tcpprobe_debug

/* Enum to identify properties */
enum { PROP_0, PROP_PORT };

static GstStaticPadTemplate gst_tcpprobe_sink_template =
GST_STATIC_PAD_TEMPLATE("sink", GST_PAD_SINK, GST_PAD_ALWAYS,
        GST_STATIC_CAPS_ANY);

static GstStaticPadTemplate gst_tcpprobe_src_template =
GST_STATIC_PAD_TEMPLATE("src", GST_PAD_SRC, GST_PAD_ALWAYS,
        GST_STATIC_CAPS_ANY);

/* Define our element type. Standard GObject/GStreamer boilerplate stuff */
#define gst_tcpprobe_parent_class parent_class
G_DEFINE_TYPE(GstTcpProbe, gst_tcpprobe, GST_TYPE_BASE_TRANSFORM);

static void gst_tcpprobe_set_property(GObject *object, guint prop_id,
        const GValue *value, GParamSpec *pspec);
static void gst_tcpprobe_get_property(GObject *object, guint prop_id,
        GValue *value, GParamSpec *pspec);

static gboolean gst_tcpprobe_start(GstBaseTransform *btrans);
static gboolean gst_tcpprobe_stop(GstBaseTransform *btrans);

static GstFlowReturn gst_tcpprobe_transform_ip(GstBaseTransform *btrans,
        GstBuffer *inbuf);

static gboolean gst_tcpprobe_sink_event(GstPad *pad, GstObject *parent,
        GstEvent *event);

/* Install properties, set sink and src pad capabilities, override the required
 * functions of the base class, These are common to all instances of the
 * element.
 */
static void gst_tcpprobe_class_init(GstTcpProbeClass *klass) {
    GObjectClass *gobject_class;
    GstElementClass *gstelement_class;
    GstBaseTransformClass *gstbasetransform_class;

    gobject_class = (GObjectClass *)klass;
    gstelement_class = (GstElementClass *)klass;
    gstbasetransform_class = (GstBaseTransformClass *)klass;

    /* Overide base class functions */
    gobject_class->set_property = GST_DEBUG_FUNCPTR(gst_tcpprobe_set_property);
    gobject_class->get_property = GST_DEBUG_FUNCPTR(gst_tcpprobe_get_property);

    g_object_class_install_property(
            gobject_class, PROP_PORT,
            g_param_spec_uint("port", "TCP Port Number", "Port number for TCP socket", 0,
                G_MAXUINT, 0,
                (GParamFlags)(G_PARAM_READWRITE |
                    G_PARAM_STATIC_STRINGS |
                    GST_PARAM_MUTABLE_READY)));

    gstbasetransform_class->start = GST_DEBUG_FUNCPTR(gst_tcpprobe_start);
    gstbasetransform_class->stop = GST_DEBUG_FUNCPTR(gst_tcpprobe_stop);

    gstbasetransform_class->transform_ip =
        GST_DEBUG_FUNCPTR(gst_tcpprobe_transform_ip);

    /* Install properties */

    /* Set sink and src pad capabilities */
    gst_element_class_add_pad_template(
            gstelement_class,
            gst_static_pad_template_get(&gst_tcpprobe_src_template));
    gst_element_class_add_pad_template(
            gstelement_class,
            gst_static_pad_template_get(&gst_tcpprobe_sink_template));

    /* Set metadata describing the element */
    gst_element_class_set_details_simple(
            gstelement_class, "TCP Probe Plugin", "TCP Probe Plugin",
            "Export bounding box information from Deepstream", "Anonymous CoVA");
}

static void gst_tcpprobe_init(GstTcpProbe *tcpprobe) {
    GstBaseTransform *btrans = GST_BASE_TRANSFORM(tcpprobe);

    /* We will not be generating a new buffer. Just adding / updating
     * metadata. */
    gst_base_transform_set_in_place(GST_BASE_TRANSFORM(btrans), TRUE);
    /* We do not want to change the input caps. Set to passthrough. transform_ip
     * is still called. */
    gst_base_transform_set_passthrough(GST_BASE_TRANSFORM(btrans), TRUE);

    /* Initialize all property variables to default values */
    tcpprobe->socket = 0;
    tcpprobe->count = 0;
}

/* Function called when a property of the element is set. Standard boilerplate.
*/
static void gst_tcpprobe_set_property(GObject *object, guint prop_id,
        const GValue *value,
        GParamSpec *pspec) {
    GstTcpProbe *tcpprobe = GST_TCPPROBE(object);
    switch (prop_id) {
        case PROP_PORT:
            tcpprobe->port = g_value_get_uint(value);
            break;
        default:
            G_OBJECT_WARN_INVALID_PROPERTY_ID(object, prop_id, pspec);
            break;
    }
}

/* Function called when a property of the element is requested. Standard
 * boilerplate.
 */
static void gst_tcpprobe_get_property(GObject *object, guint prop_id,
        GValue *value, GParamSpec *pspec) {
    GstTcpProbe *tcpprobe = GST_TCPPROBE(object);

    switch (prop_id) {
        case PROP_PORT:
            g_value_set_uint(value, tcpprobe->port);
            break;
        default:
            G_OBJECT_WARN_INVALID_PROPERTY_ID(object, prop_id, pspec);
            break;
    }
}

/**
 * Initialize all resources and start the output thread
 */
static gboolean gst_tcpprobe_start(GstBaseTransform *btrans) {
    GstTcpProbe *tcpprobe = GST_TCPPROBE(btrans);

    struct sockaddr_in server_addr;
    if (tcpprobe->port) {
        if((tcpprobe->socket = socket(PF_INET, SOCK_STREAM, 0)) < 0) {
            printf("Failed to open socket");
            goto error;
        }
        memset(&server_addr, 0, sizeof(server_addr));
        server_addr.sin_family = AF_INET;
        server_addr.sin_port = htons(tcpprobe->port);
        server_addr.sin_addr.s_addr = inet_addr("127.0.0.1");
        if(connect(tcpprobe->socket, (struct sockaddr *)&server_addr, sizeof(struct sockaddr_in)) == -1){
            close(tcpprobe->socket);
            printf("Failed to connect to server");
            goto error;
        }
    }
    return TRUE;
error:
    return FALSE;
}

/**
 * Stop the output thread and free up all the resources
 */
static gboolean gst_tcpprobe_stop(GstBaseTransform *btrans) {
    GstTcpProbe *tcpprobe = GST_TCPPROBE(btrans);

    if (tcpprobe->port)
        close(tcpprobe->socket);

    return TRUE;
}

/**
 * Called when element recieves an input buffer from upstream element.
 */
static GstFlowReturn gst_tcpprobe_transform_ip(GstBaseTransform *btrans,
        GstBuffer *inbuf) {

    GstTcpProbe *tcpprobe = GST_TCPPROBE(btrans);

    if (!tcpprobe->port)
        return GST_FLOW_OK;

    NvDsFrameMeta *frame_meta = NULL;
    NvDsObjectMeta *obj_meta = NULL;
    NvDsClassifierMeta *class_meta = NULL;
    NvDsLabelInfo *label_meta = NULL;
    NvDsMetaList *l_frame = NULL;
    NvDsMetaList *l_obj = NULL;

    NvDsBatchMeta *batch_meta = gst_buffer_get_nvds_batch_meta(inbuf);

    uint8_t buffer[24];

    int i = 0;

    for (l_frame = batch_meta->frame_meta_list; l_frame != NULL;
            l_frame = l_frame->next) {
        frame_meta = (NvDsFrameMeta *)(l_frame->data);
        uint64_t timestamp = frame_meta->buf_pts;
        GST_TRACE_OBJECT(tcpprobe, "Timestamp: %llu", timestamp);

        for (l_obj = frame_meta->obj_meta_list; l_obj != NULL;
                l_obj = l_obj->next) {
            i++;

            obj_meta = (NvDsObjectMeta *)(l_obj->data);
            float left = obj_meta->detector_bbox_info.org_bbox_coords.left;
            float top = obj_meta->detector_bbox_info.org_bbox_coords.top;
            float width = obj_meta->detector_bbox_info.org_bbox_coords.width;
            float height = obj_meta->detector_bbox_info.org_bbox_coords.height;
            int class_id = obj_meta->class_id;
            uint64_t object_id = obj_meta->object_id;

            char socket_buffer[100];
            int n = sprintf(socket_buffer, "%lu,%f,%f,%f,%f,%d\n", timestamp, left, top, width, height, class_id);
            int remain = n;
            while (remain > 0) {
                int sent_len = send(tcpprobe->socket, socket_buffer + (n - remain), remain, 0);
                remain -= sent_len;
            }
        }
    }

    tcpprobe->count++;

    return GST_FLOW_OK;

error:
    return GST_FLOW_ERROR;
}

/**
 * Boiler plate for registering a plugin and an element.
 */
static gboolean tcpprobe_plugin_init(GstPlugin *plugin) {
    GST_DEBUG_CATEGORY_INIT(gst_tcpprobe_debug, "tcpprobe", 0,
            "tcpprobe plugin");

    return gst_element_register(plugin, "tcpprobe", GST_RANK_PRIMARY,
            GST_TYPE_TCPPROBE);
}

GST_PLUGIN_DEFINE(GST_VERSION_MAJOR, GST_VERSION_MINOR, tcpprobe, DESCRIPTION,
        tcpprobe_plugin_init, "5.0", LICENSE, BINARY_PACKAGE, URL)
