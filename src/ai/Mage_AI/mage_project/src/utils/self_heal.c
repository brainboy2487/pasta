/* self_heal.c - Simple auto-healing service implementation
 * Monitors artifact health (MPHF, generated vocab) and performs reflex
 * actions like regenerating vocab and rebuilding MPHF. Logs actions to
 * data/self_heal.log and creates periodic snapshots of key artifacts.
 */

#define _POSIX_C_SOURCE 200809L

#include "mage/self_heal.h"
#include "mage/api.h"
#include "mage/subproc.h"
#include "mage/tool.h"
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <stdarg.h>

static pthread_t heal_thread;
static int heal_running = 0;

static void log_line(const char* fmt, ...) {
    FILE* f = fopen("data/self_heal.log", "a");
    if (!f) return;
    time_t t = time(NULL);
    struct tm tm; localtime_r(&t, &tm);
    char ts[64]; strftime(ts, sizeof(ts), "%Y-%m-%d %H:%M:%S", &tm);
    fprintf(f, "[%s] ", ts);
    va_list ap; va_start(ap, fmt); vfprintf(f, fmt, ap); va_end(ap);
    fprintf(f, "\n");
    fclose(f);
}

static void snapshot_state(void) {
    /* Create a simple snapshot by copying key artifacts into data/snapshots/ */
    char cmd[1024];
    snprintf(cmd, sizeof(cmd), "mkdir -p data/snapshots && cp -f data/mphf.bin data/snapshots/mphf.bin.$$ && cp -f data/vocab/generated_vocab.jsonl data/snapshots/generated_vocab.jsonl.$$ || true");
    subproc_run_shell(cmd, 30000);
}

static int file_nonempty(const char* path) {
    FILE* f = fopen(path, "r");
    if (!f) return 0;
    int c = fgetc(f);
    fclose(f);
    return c != EOF;
}

static void* heal_main(void* _arg) {
    (void)_arg;
    log_line("self-heal: starting");
    while (heal_running) {
        /* 1) Check MPHF */
        int mphf_ok = mage_api_mphf_probe("data/mphf.bin");
        if (!mphf_ok) {
            log_line("self-heal: MPHF missing or invalid — attempting rebuild");
            /* ensure vocab present */
            if (!file_nonempty("data/vocab/generated_vocab.jsonl")) {
                log_line("self-heal: generated vocab missing — attempting to build from sample");
                int wrote = mage_api_build_vocab_from_file("data/train_sample.txt", "data/vocab/generated_vocab.jsonl", 1);
                log_line("self-heal: built vocab (%d tokens)", wrote);
            }
            /* attempt to regenerate MPHF using tools helper — prefer CHD generator */
            char cmd[1024];
            char* tool = NULL;
            /* Try CHD generator first */
            tool = mage_find_tool("tools/gen_mphf_chd");
            if (tool) {
                snprintf(cmd, sizeof(cmd), "%s data/vocab/generated_vocab.jsonl data/mphf.bin", tool);
                free(tool);
            } else {
                /* Fallback to legacy generator */
                tool = mage_find_tool("tools/gen_mphf");
                if (tool) {
                    snprintf(cmd, sizeof(cmd), "%s data/vocab/generated_vocab.jsonl data/mphf.bin", tool);
                    free(tool);
                } else {
                    /* Try relative paths as last resort */
                    if (access("./tools/gen_mphf_chd", X_OK) == 0) {
                        snprintf(cmd, sizeof(cmd), "./tools/gen_mphf_chd data/vocab/generated_vocab.jsonl data/mphf.bin");
                    } else {
                        snprintf(cmd, sizeof(cmd), "./tools/gen_mphf data/vocab/generated_vocab.jsonl data/mphf.bin");
                    }
                }
            }
            int rc = subproc_run_shell(cmd, 120000);
            if (rc == 0) log_line("self-heal: regenerated MPHF (rc=0)"); else log_line("self-heal: failed to regenerate MPHF (rc=%d)", rc);
        }

        /* 2) Check generated vocab format and presence */
        if (!file_nonempty("data/vocab/generated_vocab.jsonl")) {
            log_line("self-heal: generated_vocab.jsonl empty or missing — attempting build");
            int wrote = mage_api_build_vocab_from_file("data/train_sample.txt", "data/vocab/generated_vocab.jsonl", 1);
            log_line("self-heal: build_vocab wrote %d tokens", wrote);
        }

        /* 3) Snapshot key artifacts periodically */
        snapshot_state();

        /* Sleep for a short period (5s) with nanosleep for portability */
        struct timespec ts = {.tv_sec = 5, .tv_nsec = 0};
        nanosleep(&ts, NULL);
    }
    log_line("self-heal: stopping");
    return NULL;
}

int mage_self_heal_start(void) {
    if (heal_running) return 0;
    heal_running = 1;
    if (pthread_create(&heal_thread, NULL, heal_main, NULL) != 0) {
        heal_running = 0; return -1;
    }
    return 0;
}

void mage_self_heal_stop(void) {
    if (!heal_running) return;
    heal_running = 0;
    pthread_join(heal_thread, NULL);
}

/* Start automatically when library is loaded, stop on unload. Best-effort. */
static void __attribute__((constructor)) self_heal_lib_init(void) {
    /* ignore return; best-effort */
    (void)mage_self_heal_start();
}

static void __attribute__((destructor)) self_heal_lib_fini(void) {
    mage_self_heal_stop();
}
