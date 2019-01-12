#define _GNU_SOURCE 1
#include <stdint.h>
#include <stddef.h>

#include "../lib/ref.h"
#include "../lib/object.h"
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
