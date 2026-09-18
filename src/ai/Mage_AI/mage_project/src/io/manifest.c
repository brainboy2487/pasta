/* manifest.c - simple JSONL manifest loader
 * Expected per-line JSON objects with at least an `index` and `id` field.
 * Other optional fields: `title`, `path`.
 */

#define _POSIX_C_SOURCE 200809L
#include "mage/manifest.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>

/* Minimal JSON key extraction helpers (no full JSON parser required for tests).
 * Looks for keys in the line like "index": 123, "id": "doc-1"
 */
static char* extract_json_str(const char* s, const char* key) {
    const char* p = strstr(s, key);
    if (!p) return NULL;
    p = strchr(p, ':'); if (!p) return NULL; ++p;
    while (*p && (*p == ' ' || *p == '\t')) ++p;
    if (*p != '"') return NULL;
    ++p;
    const char* q = p;
    while (*q && *q != '"') {
        if (*q == '\\' && q[1]) q += 2; else ++q;
    }
    size_t len = (size_t)(q - p);
    char* out = malloc(len + 1);
    if (!out) return NULL;
    strncpy(out, p, len);
    out[len] = '\0';
    return out;
}

static int extract_json_uint(const char* s, const char* key, uint32_t* out) {
    const char* p = strstr(s, key);
    if (!p) return 0;
    p = strchr(p, ':'); if (!p) return 0; ++p;
    while (*p && (*p == ' ' || *p == '\t')) ++p;
    char* end = NULL;
    unsigned long v = strtoul(p, &end, 10);
    if (end == p) return 0;
    *out = (uint32_t)v;
    return 1;
}

manifest_t* manifest_load(const char* path) {
    if (!path) return NULL;
    FILE* f = fopen(path, "r");
    if (!f) return NULL;
    manifest_t* m = calloc(1, sizeof(manifest_t));
    if (!m) { fclose(f); return NULL; }
    size_t cap = 16;
    m->entries = calloc(cap, sizeof(manifest_entry_t));
    if (!m->entries) { fclose(f); free(m); return NULL; }

    char* line = NULL; size_t sz = 0; ssize_t r;
    while ((r = getline(&line, &sz, f)) != -1) {
        if (m->n + 1 > cap) {
            cap *= 2;
            manifest_entry_t* t = realloc(m->entries, cap * sizeof(manifest_entry_t));
            if (!t) break;
            m->entries = t;
        }
        /* parse fields */
        uint32_t idx = 0;
        extract_json_uint(line, "\"index\"", &idx);
        char* id = extract_json_str(line, "\"id\"");
        char* title = extract_json_str(line, "\"title\"");
        char* pth = extract_json_str(line, "\"path\"");
        manifest_entry_t* e = &m->entries[m->n++];
        e->index = idx;
        e->id = id;
        e->title = title;
        e->path = pth;
    }
    free(line);
    fclose(f);
    return m;
}

const manifest_entry_t* manifest_get(const manifest_t* m, uint32_t idx) {
    if (!m) return NULL;
    for (uint32_t i = 0; i < m->n; ++i) {
        if (m->entries[i].index == idx) return &m->entries[i];
    }
    return NULL;
}

void manifest_free(manifest_t* m) {
    if (!m) return;
    for (uint32_t i = 0; i < m->n; ++i) {
        free(m->entries[i].id);
        free(m->entries[i].title);
        free(m->entries[i].path);
    }
    free(m->entries);
    free(m);
}
