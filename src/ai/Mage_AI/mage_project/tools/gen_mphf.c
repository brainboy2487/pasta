/* tools/gen_mphf.c
 * Small CLI to build an MPHF from a vocabulary file (one token per line)
 * and serialize it to disk using `mphf_serialize`.
 */

#define _POSIX_C_SOURCE 200809L
#include <sys/types.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include "mage/mphf.h"

static char* trim(char* s) {
    if (!s) return s;
    // trim leading
    while (*s && (*s == ' ' || *s == '\t' || *s == '\r' || *s == '\n')) ++s;
    if (*s == '\0') return s;
    // trim trailing
    char* end = s + strlen(s) - 1;
    while (end > s && (*end == ' ' || *end == '\t' || *end == '\r' || *end == '\n')) { *end = '\0'; --end; }
    return s;
}

int main(int argc, char** argv) {
    if (argc < 3) {
        fprintf(stderr, "Usage: %s <vocab_file> <out_mphf_file>\n", argv[0]);
        return 2;
    }

    const char* vocab_path = argv[1];
    const char* out_path = argv[2];

    FILE* f = fopen(vocab_path, "r");
    if (!f) { perror("fopen"); return 1; }

    size_t cap = 1024;
    size_t n = 0;
    char** keys = malloc(sizeof(char*) * cap);
    if (!keys) { fclose(f); return 1; }

    char* line = NULL;
    size_t len = 0;
    ssize_t read;
    while ((read = getline(&line, &len, f)) != -1) {
        char* t = trim(line);
        if (!t || *t == '\0') continue;
        /* If the line looks like a quoted token, extract between quotes. */
        if (*t == '"') {
            char* q = strchr(t+1, '"');
            if (q) { *q = '\0'; t++; }
        } else if (*t == '{') {
            /* Attempt a very small JSONL extraction for the "key" field.
             * We perform a simple substring search to avoid pulling in a JSON
             * dependency; this handles lines like: {"id":"hello","key":"hello",...}
             */
            const char* key_marker = "\"key\"";
            char* p = strstr(t, key_marker);
            if (p) {
                /* move to the colon after "key" */
                char* colon = strchr(p + strlen(key_marker), ':');
                if (colon) {
                    /* skip whitespace */
                    char* q = colon + 1;
                    while (*q && (*q == ' ' || *q == '\t')) ++q;
                    if (*q == '"') {
                        q++;
                        char* endq = strchr(q, '"');
                        if (endq) {
                            *endq = '\0';
                            t = q;
                        }
                    }
                }
            }
        }
        if (n + 1 > cap) {
            cap *= 2;
            char** nk = realloc(keys, sizeof(char*) * cap);
            if (!nk) { perror("realloc"); break; }
            keys = nk;
        }
        keys[n] = strdup(t);
        if (!keys[n]) { perror("strdup"); break; }
        n++;
    }
    free(line);
    fclose(f);

    if (n == 0) {
        fprintf(stderr, "No keys found in %s\n", vocab_path);
        free(keys);
        return 1;
    }

    printf("Building MPHF for %zu keys...\n", n);
    mphf_t* m = mphf_build((const char* const*)keys, n);
    if (!m) {
        fprintf(stderr, "mphf_build failed\n");
        for (size_t i = 0; i < n; ++i) free(keys[i]);
        free(keys);
        return 1;
    }

    printf("Serializing MPHF to %s...\n", out_path);
    if (mphf_serialize(m, out_path) != 0) {
        fprintf(stderr, "mphf_serialize failed\n");
        mphf_destroy(m);
        for (size_t i = 0; i < n; ++i) free(keys[i]);
        free(keys);
        return 1;
    }

    printf("Done.\n");

    mphf_destroy(m);
    for (size_t i = 0; i < n; ++i) free(keys[i]);
    free(keys);
    return 0;
}
