#ifndef UTILS_H_
#define UTILS_H_

#define _GNU_SOURCE 1
#include <stdio.h>
#include <stdlib.h>

static inline void trap_unimplemented(const char *symbol_name) {
    fprintf(stderr, "Invoked unimplemented method %s. Aborting.", symbol_name);
    abort();
}

#endif // UTILS_H_