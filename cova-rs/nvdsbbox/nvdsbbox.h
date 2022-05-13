#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#ifndef __NVDSBBOX_H__
#define __NVDSBBOX_H__
typedef struct bboxes bboxes_t;

extern "C" bboxes_t *bboxes_new();
extern "C" uint64_t bboxes_add(const bboxes_t *, float left, float top,
                               float width, float height, uint64_t timestamp,
                               uint32_t class_id, float confidence);
extern "C" uint32_t bboxes_end(const bboxes_t *, const uint8_t *out,
                               size_t len);
#endif // __NVDSBBOX_H__
