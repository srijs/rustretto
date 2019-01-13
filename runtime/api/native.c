#define _GNU_SOURCE 1
#include <stdint.h>
#include <stddef.h>

#include "../lib/ref.h"
#include "../lib/object.h"
#include "../lib/array.h"
#include "../lib/monitor.h"
#include "../lib/utils.h"

void _ZN4java4lang6Object15registerNativesIu9Jed9fc4b9EEvv() {}

ref_t _ZN4java4lang6Object8getClassIu9Jd57352f6EEN4java4lang5ClassEv(ref_t _this) {
    trap_unimplemented("java.lang.Object.getClass");
    return REF_NULL;
}

uint32_t _ZN4java4lang6Object8hashCodeIu9J7c7c3589EEiv(ref_t this) {
    return REF_HASH(this);
}

ref_t _ZN4java4lang6Object5cloneIu9J117cf78dEEN4java4lang6ObjectEv(ref_t _this) {
    trap_unimplemented("java.lang.Object.clone");
    return REF_NULL;
}

void _ZN4java4lang6Object6notifyIu9Jec9f6595EEvv(ref_t this) {
    monitor_t *monitor = &OBJECT_BASE_PTR(this)->monitor;
    // TODO: ensure calling thread owns the monitor
    monitor_notify_one(monitor);
}

void _ZN4java4lang6Object9notifyAllIu9J01f1085cEEvv(ref_t this) {
    monitor_t *monitor = &OBJECT_BASE_PTR(this)->monitor;
    // TODO: ensure calling thread owns the monitor
    monitor_notify_all(monitor);
}

void _ZN4java4lang6Object4waitIu9J70446489EEvl(ref_t this, uint64_t timeout) {
    monitor_t *monitor = &OBJECT_BASE_PTR(this)->monitor;
    // TODO: ensure calling thread owns the monitor
    monitor_wait(monitor, timeout);
}

void _ZN4java4lang6System9arraycopyIu9Jb7e57d76EEvN4java4lang6ObjectEiN4java4lang6ObjectEii(ref_t src, int32_t src_pos, ref_t dest, int32_t dest_pos, int32_t length) {
    size_t width = ARRAY_BASE_PTR(src)->width;
    // TODO: properly ensure array types match
    if (width != ARRAY_BASE_PTR(dest)->width) {
        PANIC("Attempt to copy between arrays of different element widths.");
    }
    // TODO: perform bounds checks
    off_t src_off = width * src_pos;
    off_t dest_off = width * dest_pos;
    size_t length_in_bytes = width * length;
    void *src_ptr = &ARRAY_DATA_PTR(src, uint8_t)[src_off];
    void *dest_ptr = &ARRAY_DATA_PTR(dest, uint8_t)[dest_off];
    memmove(dest_ptr, src_ptr, length_in_bytes);
}

uint32_t _ZN4java4lang5Float17floatToRawIntBitsIu9Jf7687691EEif(float value) {
    typedef union {
        uint32_t i;
        float f;
    } cast;
    cast c = {.f = value};
    return c.i;
}

uint64_t _ZN4java4lang6Double19doubleToRawLongBitsIu9Jc8bf6376EEld(double value) {
    typedef union {
        uint64_t j;
        double d;
    } cast;
    cast c = {.d = value};
    return c.j;
}
