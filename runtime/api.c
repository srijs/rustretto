#include <stdint.h>
#include <stdlib.h>
#include <memory.h>

#include "ref.h"

extern struct {} _ZTVN4java4lang6ObjectE;

ref_t _Jrt_new(uint64_t size, void *vtable) {
    void *object = malloc(size);
    return (ref_t) {
        .object = object,
        .vtable = vtable
    };
}

ref_t _Jrt_new_array(uint32_t count, uint64_t component_size) {
    size_t size = 4 + count * component_size;
    void *object = malloc(size);
    *((uint32_t *)object) = count;
    return (ref_t) {
        .object = object,
        .vtable = &_ZTVN4java4lang6ObjectE,
    };
}

void _Jrt_throw(ref_t _throwable) {
    abort();
}

ref_t _Jrt_ldstr(int32_t _len, void *bytes) {
    // horrible hack! :(
    return (ref_t) {
        .object = bytes,
        .vtable = NULL
    };
}
