#define _GNU_SOURCE 1
#include <stdint.h>
#include <stdlib.h>
#include <memory.h>

#include "../lib/ref.h"

ref_t _Jrt_new(uint64_t size, void *vtable) {
    return (ref_t) {
        .object = malloc(size),
        .vtable = vtable
    };
}
