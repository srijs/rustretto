#ifndef REF_H_
#define REF_H_

typedef struct {
  void *object;
  void *vtable;
} ref_t;

#define REF_NULL ((ref_t) { .object = NULL, .vtable = NULL })
#define REF_HASH(ref) ((uint32_t)(intptr_t)ref.object)

#endif // REF_H_
