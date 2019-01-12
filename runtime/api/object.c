#define _GNU_SOURCE 1
#include <stdint.h>
#include <stdlib.h>
#include <memory.h>

#include "../lib/ref.h"
#include "../lib/object.h"
#include "../lib/monitor.h"

ref_t _Jrt_object_new(uint64_t size, void *vtable) {
    return object_new(size, vtable);
}

void *_Jrt_object_field_ptr(ref_t ref) {
    return OBJECT_DATA_PTR(ref, void);
}

void *_Jrt_object_vtable_lookup(ref_t ref, uint64_t index) {
    return REF_VTABLE_PTR(ref)->methods[index];
}

void *_Jrt_object_itable_lookup(ref_t ref, void *iface, uint64_t index) {
    struct ref_itable_base *table = REF_ITABLE_PTR(ref);
    uint32_t i;
    for (i = 0; i < table->length; i++) {
        if (table->entries[i].interface == iface) {
            uint32_t offset = table->entries[i].offset;
            return REF_VTABLE_PTR(ref)->methods[offset + index];
        }
    }
    return NULL;
}

void _Jrt_object_monitorenter(ref_t ref) {
    monitor_t *monitor = &OBJECT_BASE_PTR(ref)->monitor;
    monitor_enter(monitor);
}

void _Jrt_object_monitorexit(ref_t ref) {
    monitor_t *monitor = &OBJECT_BASE_PTR(ref)->monitor;
    // TODO: ensure calling thread owns the monitor
    monitor_exit(monitor);
}
