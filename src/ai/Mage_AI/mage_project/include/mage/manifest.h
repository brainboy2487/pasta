/* manifest.h - JSONL manifest reader for vectors/documents */
#ifndef MAGE_MANIFEST_H
#define MAGE_MANIFEST_H

#include <stdint.h>

typedef struct {
    uint32_t index; /* vector index */
    char* id;       /* document id */
    char* title;    /* optional title */
    char* path;     /* optional source path */
} manifest_entry_t;

typedef struct {
    manifest_entry_t* entries;
    uint32_t n;
} manifest_t;

/* Load a JSONL manifest from `path`. Returns malloc'd manifest or NULL on error. */
manifest_t* manifest_load(const char* path);
/* Get entry at vector index (or NULL if out of range). */
const manifest_entry_t* manifest_get(const manifest_t* m, uint32_t idx);
/* Free manifest and contained strings. */
void manifest_free(manifest_t* m);

#endif
