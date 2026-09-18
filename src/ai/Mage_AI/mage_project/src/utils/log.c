/* log.c - simple thread-safe leveled logger
 */
#define _POSIX_C_SOURCE 200809L

#include "mage/log.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <time.h>
#include <pthread.h>

static FILE* logf = NULL;
static mage_log_level_t cur_level = MAGE_LOG_INFO;
static pthread_mutex_t log_lock = PTHREAD_MUTEX_INITIALIZER;

int mage_log_init(mage_log_level_t level, const char* path) {
    pthread_mutex_lock(&log_lock);
    cur_level = level;
    if (path) {
        logf = fopen(path, "a");
        if (!logf) { pthread_mutex_unlock(&log_lock); return -1; }
    } else {
        logf = stderr;
    }
    pthread_mutex_unlock(&log_lock);
    return 0;
}

void mage_log_close(void) {
    pthread_mutex_lock(&log_lock);
    if (logf && logf != stderr) fclose(logf);
    logf = NULL;
    pthread_mutex_unlock(&log_lock);
}

void mage_log_set_level(mage_log_level_t level) { cur_level = level; }
mage_log_level_t mage_log_get_level(void) { return cur_level; }

void mage_log(mage_log_level_t level, const char* fmt, ...) {
    if (level > cur_level) return;
    pthread_mutex_lock(&log_lock);
    FILE* f = logf ? logf : stderr;
    time_t t = time(NULL);
    struct tm tm; localtime_r(&t, &tm);
    char ts[32]; strftime(ts, sizeof(ts), "%Y-%m-%d %H:%M:%S", &tm);
    const char* lev = "?";
    switch (level) { case MAGE_LOG_ERROR: lev="ERROR"; break; case MAGE_LOG_WARN: lev="WARN"; break; case MAGE_LOG_INFO: lev="INFO"; break; case MAGE_LOG_DEBUG: lev="DEBUG"; break; }
    fprintf(f, "[%s] %s: ", ts, lev);
    va_list ap; va_start(ap, fmt); vfprintf(f, fmt, ap); va_end(ap);
    fprintf(f, "\n"); fflush(f);
    pthread_mutex_unlock(&log_lock);
}
