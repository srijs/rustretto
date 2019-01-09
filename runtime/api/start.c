#define _GNU_SOURCE 1
#include <stdint.h>
#include <stdlib.h>

#include "../lib/ref.h"
#include "../lib/array.h"
#include "../lib/string.h"
#include "../lib/thread.h"

uint32_t _Jrt_start(uint32_t argc, char **argv, void (*static_main_method)(ref_t args)) {
    thread_name_set("main");

    ref_t args;
    if (argc > 0) {
        args = array_new(argc - 1, sizeof(ref_t));
        ref_t *data = ARRAY_DATA_PTR(args, ref_t);
        int i;
        for (i = 0; i < argc - 1; i++) {
            data[i] = string_new(argv[i + 1]);
        }
    } else {
        args = array_new(0, sizeof(ref_t));
    }

    static_main_method(args);

    return 0;
}
