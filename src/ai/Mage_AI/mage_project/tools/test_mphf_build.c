#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include "mage/mphf.h"

int main(int argc, char** argv) {
    if (argc < 3) { fprintf(stderr, "Usage: %s <vocab_file> <key>\n", argv[0]); return 2; }
    const char* vocab = argv[1];
    const char* key = argv[2];
    FILE* f = fopen(vocab, "r"); if (!f) { perror("fopen"); return 1; }
    char* line = NULL; size_t len = 0; ssize_t r;
    size_t cap = 128, n = 0; char** keys = malloc(sizeof(char*)*cap);
    while ((r = getline(&line, &len, f)) != -1) {
        char* s = line;
        while (*s==' '||*s=='\t'||*s=='\n' || *s=='\r') ++s;
        if (*s=='\0') continue;
        if (*s=='"') {
            char* q = strchr(s+1,'"'); if (q) {*q='\0'; s++;}
        } else if (*s == '{') {
            const char* key_marker = "\"key\"";
            char* p = strstr(s, key_marker);
            if (p) {
                char* colon = strchr(p + strlen(key_marker), ':');
                if (colon) {
                    char* q = colon + 1;
                    while (*q && (*q == ' ' || *q == '\t')) ++q;
                    if (*q == '"') {
                        q++;
                        char* endq = strchr(q, '"');
                        if (endq) { *endq = '\0'; s = q; }
                    }
                }
            }
        }
        if (n+1>cap) { cap*=2; keys = realloc(keys, sizeof(char*)*cap); }
        keys[n++] = strdup(s);
    }
    free(line); fclose(f);
    mphf_t* m = mphf_build((const char* const*)keys, n);
    if (!m) { fprintf(stderr, "mphf_build failed\n"); return 1; }
    uint32_t idx = mphf_lookup(m, key);
    printf("Built lookup '%s' -> %u\n", key, idx);
    mphf_destroy(m);
    for (size_t i=0;i<n;++i) free(keys[i]); free(keys);
    return 0;
}
