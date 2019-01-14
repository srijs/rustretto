#define _GNU_SOURCE 1
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <execinfo.h>
#include <unwind.h>

#include "../lib/ref.h"
#include "../lib/array.h"
#include "../lib/string.h"
#include "../lib/thread.h"

static uint64_t EXCEPTION_CLASS = (uint64_t)"__JRT_EXCEPTION";

static int THREADNAME_MAX_LEN = 32;
static int BACKTRACE_MAX_LEN = 64;

struct exception {
    struct _Unwind_Exception unwind;
    int backtrace_len;
    char **backtrace;
};

static void exception_cleanup(_Unwind_Reason_Code reason, struct _Unwind_Exception *exc) {
    free(((struct exception *)exc)->backtrace);
    free(exc);
}

static void exception_print(struct exception *exc) {
    char thread_name[THREADNAME_MAX_LEN];
    if (thread_name_get(thread_name, THREADNAME_MAX_LEN) == 0) {
        fprintf(stderr, "Exception in thread \"%s\"\n", thread_name);
    } else {
        fprintf(stderr, "Exception in unknown thread\n");
    }

    int i;
    for (i = 0; i < exc->backtrace_len; i++) {
        fprintf(stderr, "%s\n", exc->backtrace[i]);
    } 
}

void _Jrt_throw(ref_t _throwable) {
    // allocate and initialize exception
    struct exception *exc = malloc(sizeof(struct exception));
    exc->unwind.exception_class = EXCEPTION_CLASS;
    exc->unwind.exception_cleanup = exception_cleanup;

    // capture stack trace
    void *stack[BACKTRACE_MAX_LEN];
    int stack_len = backtrace(stack, BACKTRACE_MAX_LEN);
    exc->backtrace_len = stack_len;
    exc->backtrace = backtrace_symbols(stack, stack_len);

    // unwind and handle any errors
    _Unwind_Reason_Code code = _Unwind_RaiseException((struct _Unwind_Exception *)exc);
    switch (code) {
    case _URC_END_OF_STACK:
        exception_print(exc);
        exit(EXIT_FAILURE);
    default:
        PANIC("Unknown error occurred during unwinding. Aborting.\n");
    }
}

void _Jrt_abstract() {
    PANIC("Invoked abstract method. Aborting.\n");
}
