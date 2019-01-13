#define _GNU_SOURCE 1
#include <stdint.h>
#include <stdio.h>
#include <stddef.h>
#include <math.h>

#include "../lib/ref.h"
#include "../lib/utils.h"

struct vtable_printstream {
    uint32_t length;
    void *padding[43];
    void (*println_string)(ref_t, ref_t);
};

static void printstream_println_string_stub(ref_t _this, ref_t string) {
    puts((const char *)string.object);
}

static struct vtable_printstream VTABLE_PRINTSTREAM = {
    .length = 44,
    .println_string = printstream_println_string_stub
};

ref_t _ZN4java4lang6System3outE = {
    .object = NULL,
    .vtable = &VTABLE_PRINTSTREAM
};

struct ref_vtable_base _ZTVN4java4lang13StringBuilderE;

struct ref_vtable_base _ZTVN4java4lang24IllegalArgumentExceptionE;

void _ZN4java4lang13StringBuilder4initIu9J8cc45093EEvv(ref_t _this) {
    trap_unimplemented("java.lang.StringBuilder.<init>");
}

void _ZN4java4lang24IllegalArgumentException4initIu9Jffb6fc97EEvN4java4lang6StringE(ref_t _this, ref_t _string) {
    trap_unimplemented("java.lang.IllegalArgumentException.<init>");
}

ref_t _ZN4java4lang7Integer11toHexStringIu9Jab2e85aaEEN4java4lang6StringEi(int64_t _value) {
    trap_unimplemented("java.lang.Integer.toHexString");
    return REF_NULL;
}

uint32_t _ZN4java4lang5Float5isNaNIu9Jbff373faEEu7booleanf(float value) {
    return isnan(value);
}

uint32_t _ZN4java4lang6Double5isNaNIu9J0cf9d461EEu7booleand(double value) {
    return isnan(value);
}
