#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <memory.h>
#include <execinfo.h>

#include "ref.h"
#include "array.h"
#include "string.h"
#include "thread.h"

uint32_t _Jrt_start(uint32_t argc, char **argv, void (*static_main_method)(ref_t args)) {
    thread_name_set("main");

    ref_t args;
    if (argc > 0) {
        args = array_new(argc - 1, sizeof(ref_t));
        ref_t *elements = ARRAY_ELEMENTS_PTR(args, ref_t);
        int i;
        for (i = 0; i < argc - 1; i++) {
            elements[i] = string_new(argv[i + 1]);
        }
    } else {
        args = array_new(0, sizeof(ref_t));
    }

    static_main_method(args);

    return 0;
}

ref_t _Jrt_new(uint64_t size, void *vtable) {
    return (ref_t) {
        .object = malloc(size),
        .vtable = vtable
    };
}

ref_t _Jrt_new_array(uint32_t count, uint64_t component_size) {
    return array_new(count, component_size);
}

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

ref_t _Jrt_ldstr(void *bytes) {
    return string_new(bytes);
}
