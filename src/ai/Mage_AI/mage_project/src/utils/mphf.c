/*
 * Open-addressing MPHF fallback implementation
 *
 * This implementation provides a deterministic, practical MPHF by inserting
 * keys into a power-of-two-sized table with linear probing. It keeps copies
 * of keys and their assigned indices so lookups for original keys return the
 * stored index. This is intended as a pragmatic replacement while a full
 * CHD builder is integrated.
 */

#include "mage/mphf.h"
#include "mage/hash.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

struct mphf_t {
    size_t num_keys;
    size_t table_size; /* power-of-two table size */
    char** table_keys;  /* table_size entries, NULL if empty */
    uint32_t* table_vals; /* stored index for each occupied slot */
    uint64_t seed;
};

/* next power of two >= x */
static size_t next_pow2(size_t x) {
    size_t v = 1;
    while (v < x) v <<= 1;
    return v;
}

mphf_t* mphf_build(const char* const* keys, size_t num_keys) {
    if (!keys || num_keys == 0) return NULL;

    /* Table at least 4x the key count to reduce probing length */
    size_t table_size = next_pow2(num_keys * 4 + 1);
    mphf_t* m = calloc(1, sizeof(mphf_t));
    if (!m) return NULL;
    m->num_keys = num_keys;
    m->table_size = table_size;
    m->seed = 1469598103934665603ULL; /* fixed seed for determinism */

    m->table_keys = calloc(table_size, sizeof(char*));
    m->table_vals = calloc(table_size, sizeof(uint32_t));
    if (!m->table_keys || !m->table_vals) { mphf_destroy(m); return NULL; }

    for (size_t i = 0; i < num_keys; ++i) {
        const char* k = keys[i];
        uint64_t h = hash_murmur64(k, strlen(k), m->seed);
        size_t idx = (size_t)(h & (m->table_size - 1));
        size_t start = idx;
        while (m->table_keys[idx] != NULL) {
            idx = (idx + 1) & (m->table_size - 1);
            if (idx == start) { mphf_destroy(m); return NULL; }
        }
        /* store a copy of the key to allow independent lifetime */
        size_t l = strlen(k);
        m->table_keys[idx] = malloc(l + 1);
        if (!m->table_keys[idx]) { mphf_destroy(m); return NULL; }
        memcpy(m->table_keys[idx], k, l + 1);
        m->table_vals[idx] = (uint32_t)i;
    }

    return m;
}

uint32_t mphf_lookup(const mphf_t* mphf, const char* key) {
    if (!mphf || !key) return (uint32_t)-1;
    uint64_t h = hash_murmur64(key, strlen(key), mphf->seed);
    size_t idx = (size_t)(h & (mphf->table_size - 1));
    size_t start = idx;
    while (mphf->table_keys[idx] != NULL) {
        if (strcmp(mphf->table_keys[idx], key) == 0) return mphf->table_vals[idx];
        idx = (idx + 1) & (mphf->table_size - 1);
        if (idx == start) break; /* full loop */
    }
    return (uint32_t)-1;
}

void mphf_destroy(mphf_t* mphf) {
    if (!mphf) return;
    if (mphf->table_keys) {
        for (size_t i = 0; i < mphf->table_size; ++i) free(mphf->table_keys[i]);
        free(mphf->table_keys);
    }
    free(mphf->table_vals);
    free(mphf);
}

int mphf_serialize(const mphf_t* mphf, const char* path) {
    if (!mphf || !path) return -1;
    FILE* f = fopen(path, "wb");
    if (!f) return -1;
    if (fwrite(&mphf->num_keys, sizeof(size_t), 1, f) != 1) { fclose(f); return -1; }
    if (fwrite(&mphf->table_size, sizeof(size_t), 1, f) != 1) { fclose(f); return -1; }
    if (fwrite(&mphf->seed, sizeof(uint64_t), 1, f) != 1) { fclose(f); return -1; }

    for (size_t i = 0; i < mphf->table_size; ++i) {
        int32_t val = (mphf->table_keys[i] == NULL) ? -1 : (int32_t)mphf->table_vals[i];
        if (fwrite(&val, sizeof(int32_t), 1, f) != 1) { fclose(f); return -1; }
        if (mphf->table_keys[i]) {
            size_t len = strlen(mphf->table_keys[i]);
            if (fwrite(&len, sizeof(size_t), 1, f) != 1) { fclose(f); return -1; }
            if (fwrite(mphf->table_keys[i], 1, len, f) != len) { fclose(f); return -1; }
        } else {
            size_t zero = 0;
            if (fwrite(&zero, sizeof(size_t), 1, f) != 1) { fclose(f); return -1; }
        }
    }

    fclose(f);
    return 0;
}

mphf_t* mphf_load(const char* path) {
    if (!path) return NULL;
    FILE* f = fopen(path, "rb");
    if (!f) return NULL;
    size_t num_keys = 0, table_size = 0;
    uint64_t seed = 0;
    if (fread(&num_keys, sizeof(size_t), 1, f) != 1) { fclose(f); return NULL; }
    if (fread(&table_size, sizeof(size_t), 1, f) != 1) { fclose(f); return NULL; }
    if (fread(&seed, sizeof(uint64_t), 1, f) != 1) { fclose(f); return NULL; }

    mphf_t* m = calloc(1, sizeof(mphf_t));
    if (!m) { fclose(f); return NULL; }
    m->num_keys = num_keys;
    m->table_size = table_size;
    m->seed = seed;
    m->table_keys = calloc(m->table_size, sizeof(char*));
    m->table_vals = calloc(m->table_size, sizeof(uint32_t));
    if (!m->table_keys || !m->table_vals) { mphf_destroy(m); fclose(f); return NULL; }

    for (size_t i = 0; i < m->table_size; ++i) {
        int32_t val = -1;
        if (fread(&val, sizeof(int32_t), 1, f) != 1) { mphf_destroy(m); fclose(f); return NULL; }
        size_t len = 0;
        if (fread(&len, sizeof(size_t), 1, f) != 1) { mphf_destroy(m); fclose(f); return NULL; }
        if (len > 0) {
            char* buf = malloc(len + 1);
            if (!buf) { mphf_destroy(m); fclose(f); return NULL; }
            if (fread(buf, 1, len, f) != len) { free(buf); mphf_destroy(m); fclose(f); return NULL; }
            buf[len] = '\0';
            m->table_keys[i] = buf;
            m->table_vals[i] = (uint32_t)val;
        } else {
            m->table_keys[i] = NULL;
            m->table_vals[i] = 0;
        }
    }

    fclose(f);
    return m;
}
