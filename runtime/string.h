#ifndef STRING_H_
#define STRING_H_

#include "stddef.h"

#include "ref.h"

static inline ref_t string_new(void *bytes) {
    // horrible hack! :(
    return (ref_t) {
        .object = bytes,
        .vtable = NULL
    };
}

#endif // STRING_H_
