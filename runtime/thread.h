#ifndef THREAD_H_
#define THREAD_H_

#define _GNU_SOURCE 1
#include <pthread.h>

static inline void thread_setname(const char *name) {
    #ifdef __GLIBC__
        pthread_t self = pthread_self();
        pthread_setname_np(self, name);
    #elif __APPLE__
        pthread_setname_np(name);
    #endif
}

static inline int thread_getname(char *name, size_t len) {
    pthread_t self = pthread_self();
    return pthread_getname_np(self, name, len);
}

#endif // THREAD_H_
