#ifndef REF_H_
#define REF_H_

typedef struct {
    void *object;
    void *vtable;
} ref_t;

struct ref_vtable_base {
    uint32_t length;
    void *methods[];
};

struct ref_itable_entry {
    void *interface;
    uint32_t offset;
};

struct ref_itable_base {
    uint32_t length;
    struct ref_itable_entry entries[];
};

#define REF_NULL ((ref_t) { .object = NULL, .vtable = NULL })
#define REF_HASH(ref) ((uint32_t)(intptr_t)ref.object)

#define REF_OBJECT_PTR(ref) (ref.object)
#define REF_VTABLE_PTR(ref) ((struct ref_vtable_base *)ref.vtable)
#define REF_ITABLE_PTR(ref) ((struct ref_itable_base *)&REF_VTABLE_PTR(ref)->methods[REF_VTABLE_PTR(ref)->length])

#endif // REF_H_
