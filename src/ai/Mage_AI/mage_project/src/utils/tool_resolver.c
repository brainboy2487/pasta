/* tool_resolver.c - simple resolver for repo-relative tools
 * Tries a small set of candidate paths (cwd, up to two parent dirs, PATH)
 */
#define _POSIX_C_SOURCE 200809L

#include "mage/tool.h"
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>
#include <stdio.h>

static int is_executable(const char* path) {
    return access(path, X_OK) == 0;
}

static char* join_paths(const char* a, const char* b) {
    size_t la = strlen(a);
    size_t lb = strlen(b);
    int need_slash = (la > 0 && a[la-1] != '/');
    char* out = malloc(la + lb + (need_slash?2:1));
    if (!out) return NULL;
    strcpy(out, a);
    if (need_slash) strcat(out, "/");
    strcat(out, b);
    return out;
}

char* mage_find_tool(const char* rel) {
    if (!rel) return NULL;
    /* 1) If rel is directly executable as given (relative or absolute), accept it */
    if (is_executable(rel)) return strdup(rel);

    /* 2) Try a few parent-dir offsets (., .., ../.. ) */
    const char* ups[] = {".", "..", ".."};
    for (size_t i = 0; i < 3; ++i) {
        char* cand = join_paths(ups[i], rel);
        if (!cand) continue;
        if (is_executable(cand)) return cand;
        free(cand);
        /* after first iteration, adjust rel prefix for next join (../rel already tried) */
        if (i == 0) {
            /* prepare rel prefixed with .. for next step */
            char* r2 = malloc(3 + strlen(rel) + 1);
            if (!r2) continue;
            strcpy(r2, "../"); strcat(r2, rel);
            rel = r2; /* note: leaks a small malloc if not resolved; acceptable for short-lived resolver */
        }
    }

    /* 3) Search PATH for basename(rel) */
    const char* base = strrchr(rel, '/');
    if (base) base++; else base = rel;
    const char* pathenv = getenv("PATH");
    if (pathenv) {
        char* p = strdup(pathenv);
        char* tok = strtok(p, ":");
        while (tok) {
            char* cand = join_paths(tok, base);
            if (cand) {
                if (is_executable(cand)) {
                    free(p);
                    return cand;
                }
                free(cand);
            }
            tok = strtok(NULL, ":");
        }
        free(p);
    }

    /* 4) fallback: return a strdup of rel (caller may try to execute it) */
    return strdup(rel);
}
