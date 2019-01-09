#define _GNU_SOURCE 1
#include <stdint.h>
#include <stdlib.h>

#include "../lib/ref.h"
#include "../lib/array.h"

ref_t _Jrt_array_new(uint32_t count, uint64_t component_size) {
    return array_new(count, component_size);
}

uint32_t _Jrt_array_length(ref_t ref) {
    return ARRAY_BASE_PTR(ref)->length;
}

void *_Jrt_array_element_ptr(ref_t ref) {
    return ARRAY_DATA_PTR(ref, void);
}
