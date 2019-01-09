#define _GNU_SOURCE 1
#include <stdint.h>
#include <stdlib.h>

#include "../lib/ref.h"
#include "../lib/string.h"

ref_t _Jrt_ldstr(void *bytes) {
    return string_new(bytes);
}
