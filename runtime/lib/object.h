#ifndef OBJECT_H_
#define OBJECT_H_

#define _GNU_SOURCE 1
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

#include "ref.h"
#include "monitor.h"

#define OBJECT_BASE_PTR(ref) ((struct object_base *)ref.object)
#define OBJECT_DATA_PTR(ref, typ) ((typ *)&OBJECT_BASE_PTR(ref)[1])

struct object_base {
    monitor_t monitor;
};

static inline ref_t object_new(uint32_t data_size, void *vtable) {
    size_t size = sizeof(struct object_base) + data_size;
    ref_t ref = {
        .object = malloc(size),
        .vtable = vtable,
    };
    OBJECT_BASE_PTR(ref)->monitor = (monitor_t)MONITOR_INITIALIZER;
    return ref;
}

#endif // OBJECT_H_
