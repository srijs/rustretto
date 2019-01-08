#ifndef ARRAY_H_
#define ARRAY_H_

#define _GNU_SOURCE 1
#include <stddef.h>
#include <stdint.h>
#include <memory.h>

#include "ref.h"
#include "extern.h"

#define ARRAY_LENGTH_PTR(ref) ((uint32_t *)ref.object)
#define ARRAY_ELEMENTS_PTR(ref, typ) ((typ *)&((uint32_t *)ref.object)[1])

static inline ref_t array_new(uint32_t count, uint64_t component_size) {
    size_t size = sizeof(uint32_t) + count * component_size;
    ref_t ref = {
        .object = malloc(size),
        .vtable = EXTERN_VTABLE_JAVA_LANG_OBJECT,
    };
    *ARRAY_LENGTH_PTR(ref) = count;
    return ref;
}

#endif // ARRAY_H_
