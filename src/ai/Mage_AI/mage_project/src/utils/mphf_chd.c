#include "mage/mphf_chd.h"
#include "mage/hash.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

/* Simple CHD-style minimal perfect hash builder (deterministic, straightforward)
 * This is a pragmatic, readable implementation intended to replace the
 * open-addressing fallback for larger vocabularies. It uses bucketization
 * (by a first hash) and finds per-bucket offsets so that secondary hashes
 * land in disjoint table slots.
 */

static size_t next_pow2(size_t x) {
    size_t v = 1;
    while (v < x) v <<= 1;
    return v;
}

struct mphf_chd_t {
    size_t num_keys;
    size_t num_buckets;
    size_t table_size;
    uint32_t *g; /* per-bucket displacement */
    char **table_keys; /* table_size entries, NULL if empty */
    uint32_t *table_vals; /* stored index for each occupied slot */
    uint64_t seed;
};

mphf_chd_t* mphf_chd_build(const char* const* keys, size_t num_keys) {
    if (!keys || num_keys == 0) return NULL;

    size_t num_buckets = num_keys; /* simple choice: one bucket per key on average */
    size_t table_size = next_pow2(num_keys * 2 + 1);

    mphf_chd_t* m = calloc(1, sizeof(mphf_chd_t));
    if (!m) return NULL;
    m->num_keys = num_keys;
    m->num_buckets = num_buckets;
    m->table_size = table_size;
    m->seed = 1469598103934665603ULL;

    m->g = calloc(num_buckets, sizeof(uint32_t));
    m->table_keys = calloc(table_size, sizeof(char*));
    m->table_vals = calloc(table_size, sizeof(uint32_t));
    if (!m->g || !m->table_keys || !m->table_vals) { mphf_chd_destroy(m); return NULL; }

    /* Build buckets: first count sizes */
    size_t *bucket_counts = calloc(num_buckets, sizeof(size_t));
    if (!bucket_counts) { mphf_chd_destroy(m); return NULL; }
    for (size_t i = 0; i < num_keys; ++i) {
        uint64_t h1 = hash_murmur64(keys[i], strlen(keys[i]), m->seed);
        size_t b = (size_t)(h1 % num_buckets);
        bucket_counts[b]++;
    }

    /* Allocate bucket lists */
    size_t **bucket_lists = calloc(num_buckets, sizeof(size_t*));
    if (!bucket_lists) { free(bucket_counts); mphf_chd_destroy(m); return NULL; }
    for (size_t b = 0; b < num_buckets; ++b) {
        if (bucket_counts[b] > 0) bucket_lists[b] = malloc(bucket_counts[b] * sizeof(size_t));
    }

    /* fill bucket lists */
    memset(bucket_counts, 0, num_buckets * sizeof(size_t));
    for (size_t i = 0; i < num_keys; ++i) {
        uint64_t h1 = hash_murmur64(keys[i], strlen(keys[i]), m->seed);
        size_t b = (size_t)(h1 % num_buckets);
        size_t idx = bucket_counts[b]++;
        bucket_lists[b][idx] = i;
    }

    /* Order buckets by size descending to place large buckets first */
    size_t *bucket_order = malloc(num_buckets * sizeof(size_t));
    if (!bucket_order) { for (size_t b=0;b<num_buckets;++b) free(bucket_lists[b]); free(bucket_lists); free(bucket_counts); mphf_chd_destroy(m); return NULL; }
    for (size_t b=0;b<num_buckets;++b) bucket_order[b] = b;
    /* simple insertion sort (num_buckets == num_keys typically) */
    for (size_t i = 1; i < num_buckets; ++i) {
        size_t v = bucket_order[i];
        size_t j = i;
        while (j > 0 && bucket_counts[bucket_order[j-1]] < bucket_counts[v]) {
            bucket_order[j] = bucket_order[j-1]; j--; }
        bucket_order[j] = v;
    }

    /* helper to check occupancy */
    for (size_t bi = 0; bi < num_buckets; ++bi) {
        size_t b = bucket_order[bi];
        size_t bsize = (bucket_lists[b] ? bucket_counts[b] : 0);
        if (bsize == 0) { m->g[b] = (uint32_t)0; continue; }

        uint32_t offset = 0;
        int placed = 0;
        for (offset = 0; offset < m->table_size; ++offset) {
            int ok = 1;
            for (size_t k = 0; k < bsize; ++k) {
                size_t key_idx = bucket_lists[b][k];
                const char* key = keys[key_idx];
                uint64_t h2 = hash_murmur64(key, strlen(key), m->seed ^ 0x9e3779b97f4a7c15ULL);
                size_t pos = (size_t)((h2 + offset) & (m->table_size - 1));
                if (m->table_keys[pos] != NULL) { ok = 0; break; }
            }
            if (ok) {
                /* commit positions */
                m->g[b] = offset;
                for (size_t k = 0; k < bsize; ++k) {
                    size_t key_idx = bucket_lists[b][k];
                    const char* key = keys[key_idx];
                    uint64_t h2 = hash_murmur64(key, strlen(key), m->seed ^ 0x9e3779b97f4a7c15ULL);
                    size_t pos = (size_t)((h2 + offset) & (m->table_size - 1));
                    m->table_keys[pos] = malloc(strlen(key) + 1);
                    strcpy(m->table_keys[pos], key);
                    m->table_vals[pos] = (uint32_t)key_idx;
                }
                placed = 1;
                break;
            }
        }
        if (!placed) {
            /* failure: fall back to simple linear probing insertion for remaining keys */
            for (size_t k = 0; k < bsize; ++k) {
                size_t key_idx = bucket_lists[b][k];
                const char* key = keys[key_idx];
                uint64_t h2 = hash_murmur64(key, strlen(key), m->seed ^ 0x9e3779b97f4a7c15ULL);
                size_t pos = (size_t)(h2 & (m->table_size - 1));
                size_t start = pos;
                while (m->table_keys[pos] != NULL) pos = (pos + 1) & (m->table_size - 1);
                m->table_keys[pos] = malloc(strlen(key) + 1);
                strcpy(m->table_keys[pos], key);
                m->table_vals[pos] = (uint32_t)key_idx;
                m->g[b] = 0;
            }
        }
    }

    /* cleanup */
    for (size_t b=0;b<num_buckets;++b) free(bucket_lists[b]);
    free(bucket_lists);
    free(bucket_counts);
    free(bucket_order);

    return m;
}

uint32_t mphf_chd_lookup(const mphf_chd_t* mphf, const char* key) {
    if (!mphf || !key) return (uint32_t)-1;
    uint64_t h1 = hash_murmur64(key, strlen(key), mphf->seed);
    size_t b = (size_t)(h1 % mphf->num_buckets);
    uint32_t offset = mphf->g[b];
    uint64_t h2 = hash_murmur64(key, strlen(key), mphf->seed ^ 0x9e3779b97f4a7c15ULL);
    size_t pos = (size_t)((h2 + offset) & (mphf->table_size - 1));
    if (mphf->table_keys[pos] && strcmp(mphf->table_keys[pos], key) == 0) return mphf->table_vals[pos];
    return (uint32_t)-1;
}

void mphf_chd_destroy(mphf_chd_t* mphf) {
    if (!mphf) return;
    if (mphf->g) free(mphf->g);
    if (mphf->table_keys) {
        for (size_t i = 0; i < mphf->table_size; ++i) free(mphf->table_keys[i]);
        free(mphf->table_keys);
    }
    free(mphf->table_vals);
    free(mphf);
}

int mphf_chd_serialize(const mphf_chd_t* mphf, const char* path) {
    if (!mphf || !path) return -1;
    FILE* f = fopen(path, "wb");
    if (!f) return -1;
    if (fwrite(&mphf->num_keys, sizeof(size_t), 1, f) != 1) { fclose(f); return -1; }
    if (fwrite(&mphf->num_buckets, sizeof(size_t), 1, f) != 1) { fclose(f); return -1; }
    if (fwrite(&mphf->table_size, sizeof(size_t), 1, f) != 1) { fclose(f); return -1; }
    if (fwrite(&mphf->seed, sizeof(uint64_t), 1, f) != 1) { fclose(f); return -1; }

    for (size_t i = 0; i < mphf->num_buckets; ++i) {
        uint32_t v = mphf->g[i];
        if (fwrite(&v, sizeof(uint32_t), 1, f) != 1) { fclose(f); return -1; }
    }

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

mphf_chd_t* mphf_chd_load(const char* path) {
    if (!path) return NULL;
    FILE* f = fopen(path, "rb");
    if (!f) return NULL;
    size_t num_keys = 0, num_buckets = 0, table_size = 0;
    uint64_t seed = 0;
    if (fread(&num_keys, sizeof(size_t), 1, f) != 1) { fclose(f); return NULL; }
    if (fread(&num_buckets, sizeof(size_t), 1, f) != 1) { fclose(f); return NULL; }
    if (fread(&table_size, sizeof(size_t), 1, f) != 1) { fclose(f); return NULL; }
    if (fread(&seed, sizeof(uint64_t), 1, f) != 1) { fclose(f); return NULL; }

    mphf_chd_t* m = calloc(1, sizeof(mphf_chd_t));
    if (!m) { fclose(f); return NULL; }
    m->num_keys = num_keys;
    m->num_buckets = num_buckets;
    m->table_size = table_size;
    m->seed = seed;
    m->g = calloc(m->num_buckets, sizeof(uint32_t));
    m->table_keys = calloc(m->table_size, sizeof(char*));
    m->table_vals = calloc(m->table_size, sizeof(uint32_t));
    if (!m->g || !m->table_keys || !m->table_vals) { mphf_chd_destroy(m); fclose(f); return NULL; }

    for (size_t i = 0; i < m->num_buckets; ++i) {
        uint32_t v = 0;
        if (fread(&v, sizeof(uint32_t), 1, f) != 1) { mphf_chd_destroy(m); fclose(f); return NULL; }
        m->g[i] = v;
    }

    for (size_t i = 0; i < m->table_size; ++i) {
        int32_t val = -1;
        if (fread(&val, sizeof(int32_t), 1, f) != 1) { mphf_chd_destroy(m); fclose(f); return NULL; }
        size_t len = 0;
        if (fread(&len, sizeof(size_t), 1, f) != 1) { mphf_chd_destroy(m); fclose(f); return NULL; }
        if (len > 0) {
            char* buf = malloc(len + 1);
            if (!buf) { mphf_chd_destroy(m); fclose(f); return NULL; }
            if (fread(buf, 1, len, f) != len) { free(buf); mphf_chd_destroy(m); fclose(f); return NULL; }
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
