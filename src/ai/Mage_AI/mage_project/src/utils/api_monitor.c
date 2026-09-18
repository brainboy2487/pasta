/* api_monitor.c - simple monitor/watchdog thread for critical modules
 * Implements registration, heartbeat and restart callback invocation.
 */

#define _POSIX_C_SOURCE 200809L
#include "mage/api_monitor.h"
#include <pthread.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <time.h>
#include <unistd.h>

struct api_monitor_handle {
    char *name;
    unsigned int timeout_ms;
    void *ctx;
    int (*restart_fn)(void*);
    struct timespec last_beat;
    int active;
};

static pthread_t monitor_thread;
static pthread_mutex_t monitor_lock = PTHREAD_MUTEX_INITIALIZER;
static struct api_monitor_handle **modules = NULL;
static size_t modules_cap = 0;
static size_t modules_len = 0;
static int monitor_running = 0;

static void current_time(struct timespec *ts) {
    clock_gettime(CLOCK_MONOTONIC, ts);
}

static long diff_ms(const struct timespec *a, const struct timespec *b) {
    long s = a->tv_sec - b->tv_sec;
    long ns = a->tv_nsec - b->tv_nsec;
    return s*1000 + ns/1000000;
}

static void* monitor_loop(void *arg) {
    (void)arg;
    while (1) {
        pthread_mutex_lock(&monitor_lock);
        if (!monitor_running) { pthread_mutex_unlock(&monitor_lock); break; }
        struct timespec now; current_time(&now);
        for (size_t i = 0; i < modules_len; ++i) {
            struct api_monitor_handle *h = modules[i];
            if (!h || !h->active) continue;
            long delta = diff_ms(&now, &h->last_beat);
            if ((unsigned long)delta > h->timeout_ms) {
                /* attempt restart outside lock to avoid deadlocks */
                pthread_mutex_unlock(&monitor_lock);
                fprintf(stderr, "[api_monitor] Timeout detected for module '%s' (delta %ld ms). Restarting...\n", h->name, delta);
                int rc = 0;
                if (h->restart_fn) rc = h->restart_fn(h->ctx);
                pthread_mutex_lock(&monitor_lock);
                if (rc == 0) {
                    /* on success, update heartbeat */
                    current_time(&h->last_beat);
                    fprintf(stderr, "[api_monitor] Restart successful for '%s'\n", h->name);
                } else {
                    fprintf(stderr, "[api_monitor] Restart FAILED for '%s' (rc=%d)\n", h->name, rc);
                }
            }
        }
        pthread_mutex_unlock(&monitor_lock);
        /* sleep a short while */
        struct timespec ts = { .tv_sec = 0, .tv_nsec = 250 * 1000000 };
        (void)nanosleep(&ts, NULL);
    }
    return NULL;
}

int api_monitor_init(void) {
    pthread_mutex_lock(&monitor_lock);
    if (monitor_running) { pthread_mutex_unlock(&monitor_lock); return 0; }
    monitor_running = 1;
    modules_cap = 8; modules_len = 0;
    modules = calloc(modules_cap, sizeof(*modules));
    if (!modules) { monitor_running = 0; pthread_mutex_unlock(&monitor_lock); return -1; }
    if (pthread_create(&monitor_thread, NULL, monitor_loop, NULL) != 0) {
        free(modules); modules = NULL; monitor_running = 0; pthread_mutex_unlock(&monitor_lock); return -1; }
    pthread_mutex_unlock(&monitor_lock);
    return 0;
}

void api_monitor_shutdown(void) {
    pthread_mutex_lock(&monitor_lock);
    if (!monitor_running) { pthread_mutex_unlock(&monitor_lock); return; }
    monitor_running = 0;
    pthread_mutex_unlock(&monitor_lock);
    pthread_join(monitor_thread, NULL);
    pthread_mutex_lock(&monitor_lock);
    for (size_t i = 0; i < modules_len; ++i) {
        if (modules[i]) {
            free(modules[i]->name);
            free(modules[i]);
            modules[i] = NULL;
        }
    }
    free(modules); modules = NULL; modules_cap = modules_len = 0;
    pthread_mutex_unlock(&monitor_lock);
}

api_monitor_handle_t* api_monitor_register_module(const char* name, unsigned int timeout_ms, void* ctx, int (*restart_fn)(void* ctx)) {
    if (!name || timeout_ms == 0) return NULL;
    api_monitor_handle_t* h = calloc(1, sizeof(*h));
    if (!h) return NULL;
    h->name = strdup(name);
    h->timeout_ms = timeout_ms;
    h->ctx = ctx;
    h->restart_fn = restart_fn;
    h->active = 1;
    current_time(&h->last_beat);
    pthread_mutex_lock(&monitor_lock);
    if (!monitor_running) {
        /* lazy init if needed */
        api_monitor_init();
    }
    if (modules_len + 1 > modules_cap) {
        size_t nc = modules_cap * 2;
        struct api_monitor_handle **n = realloc(modules, nc * sizeof(*modules));
        if (!n) { pthread_mutex_unlock(&monitor_lock); free(h->name); free(h); return NULL; }
        modules = n; modules_cap = nc;
    }
    modules[modules_len++] = h;
    pthread_mutex_unlock(&monitor_lock);
    return h;
}

int api_monitor_unregister_module(api_monitor_handle_t* h) {
    if (!h) return -1;
    pthread_mutex_lock(&monitor_lock);
    for (size_t i = 0; i < modules_len; ++i) {
        if (modules[i] == h) {
            free(h->name); h->name = NULL; h->active = 0; free(h);
            /* compact array */
            for (size_t j = i; j + 1 < modules_len; ++j) modules[j] = modules[j+1];
            modules_len--;
            pthread_mutex_unlock(&monitor_lock);
            return 0;
        }
    }
    pthread_mutex_unlock(&monitor_lock);
    return -1;
}

int api_monitor_heartbeat(api_monitor_handle_t* h) {
    if (!h) return -1;
    pthread_mutex_lock(&monitor_lock);
    current_time(&h->last_beat);
    pthread_mutex_unlock(&monitor_lock);
    return 0;
}
