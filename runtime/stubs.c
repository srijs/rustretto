#include <stdint.h>
#include <stdio.h>
#include <stddef.h>

#include "ref.h"

struct vtable_printstream {
    uint32_t length;
    void *padding[40];
    void *println_string;
};

static void printstream_println_string_stub(ref_t _this, ref_t string) {
    puts((const char *)string.object);
}

static struct vtable_printstream VTABLE_PRINTSTREAM = {
    .length = 41,
    .println_string = printstream_println_string_stub
};

ref_t _ZN4java4lang6System3outE = {
    .object = NULL,
    .vtable = &VTABLE_PRINTSTREAM
};

struct {} _ZTVN4java4lang13StringBuilderE = {};

struct {} _ZTVN4java4lang24IllegalArgumentExceptionE = {};

void _ZN4java4lang13StringBuilder4initIu9J8cc45093EEvv(ref_t _this) {
    // unimplemented!
}

void _ZN4java4lang24IllegalArgumentException4initIu9Jffb6fc97EEvN4java4lang6StringE(ref_t _this, ref_t _string) {
    // unimplemented!
}

ref_t _ZN4java4lang7Integer11toHexStringIu9Jab2e85aaEEN4java4lang6StringEi(int64_t _value) {
    // unimplemented!
    return REF_NULL;
}
