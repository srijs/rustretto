#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <memory.h>
#include <execinfo.h>

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

static int BACKTRACE_MAX_LEN = 64;

void _Jrt_throw(ref_t _throwable) {
    void *stack[BACKTRACE_MAX_LEN];
    int size = backtrace(stack, BACKTRACE_MAX_LEN);
    char **symbols = backtrace_symbols(stack, size);
    int i;
    for (i = 0; i < size; i++) {
        fprintf(stderr, "%s\n", symbols[i]);
    }
    exit(EXIT_FAILURE);
}

ref_t _Jrt_ldstr(int32_t _len, void *bytes) {
    // horrible hack! :(
    return (ref_t) {
        .object = bytes,
        .vtable = NULL
    };
}
