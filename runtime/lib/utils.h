#ifndef UTILS_H_
#define UTILS_H_

#define _GNU_SOURCE 1
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <sys/time.h>

#define PANIC(...) {\
    fprintf(stderr, __VA_ARGS__);\
    abort();\
}

static inline void ensure(int errcode) {
    if (errcode != 0) {\
        PANIC("Internal operation failed. Aborting.");\
    }
}

static inline void trap_unimplemented(const char *symbol_name) {
    PANIC("Invoked unimplemented method %s. Aborting.", symbol_name);
}

static inline void timespec_now(struct timespec *ts) {
#ifdef __MACH__
    struct timeval tv;
    ensure(gettimeofday(&tv, NULL));
    ts->tv_sec = tv.tv_sec;
    ts->tv_nsec = tv.tv_usec * 1000;
#else
    ensure(clock_gettime(CLOCK_REALTIME, ts));
#endif
}

static inline void timespec_add_msec(struct timespec *ts, uint64_t msec) {
    int sec = msec / 1000;
    msec = msec - sec * 1000;
    ts->tv_nsec += msec * 1000000;
	ts->tv_sec += ts->tv_nsec / 1000000000 + sec;
    ts->tv_nsec = ts->tv_nsec % 1000000000;
}

#endif // UTILS_H_
