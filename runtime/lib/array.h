#ifndef ARRAY_H_
#define ARRAY_H_

#define _GNU_SOURCE 1
#include <stddef.h>
#include <stdint.h>
#include <memory.h>

#include "ref.h"
#include "extern.h"
#include "object.h"

#define ARRAY_BASE_PTR(ref) (OBJECT_DATA_PTR(ref, struct array_base))
#define ARRAY_DATA_PTR(ref, typ) ((typ *)&ARRAY_BASE_PTR(ref)[1])

struct array_base {
    uint32_t length;
    uint64_t width;
};

static inline ref_t array_new(uint32_t length, uint64_t width) {
    size_t data_size = sizeof(struct array_base) + length * width;
    ref_t ref = object_new(data_size, EXTERN_VTABLE_JAVA_LANG_OBJECT);
    ARRAY_BASE_PTR(ref)->length = length;
    ARRAY_BASE_PTR(ref)->width = width;
    return ref;
}

#endif // ARRAY_H_
