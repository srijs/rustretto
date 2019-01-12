#ifndef MONITOR_H_
#define MONITOR_H_

#define _GNU_SOURCE 1
#include <pthread.h>

typedef pthread_mutex_t monitor_t;

#ifdef __GLIBC__
#define MONITOR_INITIALIZER PTHREAD_RECURSIVE_MUTEX_INITIALIZER_NP
#elif __APPLE__
#define MONITOR_INITIALIZER PTHREAD_RECURSIVE_MUTEX_INITIALIZER
#endif

#endif // MONITOR_H_
