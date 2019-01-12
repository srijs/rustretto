#ifndef MONITOR_H_
#define MONITOR_H_

#define _GNU_SOURCE 1
#include <pthread.h>
#include <time.h>
#include <errno.h>

#include "utils.h"

typedef struct {
    pthread_mutex_t mutex;
    pthread_cond_t condvar;
} monitor_t;

static inline void monitor_init(monitor_t *monitor) {
#ifdef __GLIBC__
    monitor->mutex = (pthread_mutex_t)PTHREAD_RECURSIVE_MUTEX_INITIALIZER_NP;
#elif __APPLE__
    monitor->mutex = (pthread_mutex_t)PTHREAD_RECURSIVE_MUTEX_INITIALIZER;
#endif
    monitor->condvar = (pthread_cond_t)PTHREAD_COND_INITIALIZER;
}

static inline void monitor_enter(monitor_t *monitor) {
    ensure(pthread_mutex_lock(&monitor->mutex));
}

static inline void monitor_exit(monitor_t *monitor) {
    ensure(pthread_mutex_unlock(&monitor->mutex));
}

static inline void monitor_wait(monitor_t *monitor, uint64_t timeout) {
    int err;

    if (timeout > 0) {
        struct timespec abstime;
        timespec_now(&abstime);
        timespec_add_msec(&abstime, timeout);

        err = pthread_cond_timedwait(&monitor->condvar, &monitor->mutex, &abstime);
    } else {
        err = pthread_cond_wait(&monitor->condvar, &monitor->mutex);
    }

    if (err == ETIMEDOUT) {
        return;
    }

    if (err != 0) {
        PANIC("Encountered error when waiting on conditional variable. Aborting.");
    }
}

static inline void monitor_notify_one(monitor_t *monitor) {
    ensure(pthread_cond_signal(&monitor->condvar));
}

static inline void monitor_notify_all(monitor_t *monitor) {
    ensure(pthread_cond_broadcast(&monitor->condvar));
}

#endif // MONITOR_H_
