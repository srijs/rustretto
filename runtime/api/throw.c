#define _GNU_SOURCE 1
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <execinfo.h>

#include "../lib/ref.h"
#include "../lib/array.h"
#include "../lib/string.h"
#include "../lib/thread.h"

static int THREADNAME_MAX_LEN = 32;
static int BACKTRACE_MAX_LEN = 64;

void _Jrt_throw(ref_t _throwable) {
    char thread_name[THREADNAME_MAX_LEN];
    if (thread_name_get(thread_name, THREADNAME_MAX_LEN) == 0) {
        fprintf(stderr, "Exception in thread \"%s\"\n", thread_name);
    } else {
        fprintf(stderr, "Exception in unknown thread\n");
    }

    void *stack[BACKTRACE_MAX_LEN];
    int size = backtrace(stack, BACKTRACE_MAX_LEN);
    char **symbols = backtrace_symbols(stack, size);
    int i;
    for (i = 0; i < size; i++) {
        fprintf(stderr, "%s\n", symbols[i]);
    }

    exit(EXIT_FAILURE);
}
