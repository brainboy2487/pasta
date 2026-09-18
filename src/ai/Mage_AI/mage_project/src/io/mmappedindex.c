/* Memory-mapped index implementation.
 * File format (optional): if present, first two uint32_t values are
 * `n` (number of vectors) and `d` (dimension). Followed by n*d float32
 * values in row-major order. If file is absent or malformed, the index
 * falls back to returning zero vectors.
 */

#define _POSIX_C_SOURCE 200809L

#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <stdint.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <errno.h>

#include "mage/mmappedindex.h"

struct mmapped_index {
    char *path;
    int fd;
    void *map;
    size_t map_size;
    uint32_t n;
    uint32_t d;
    size_t expected_vector_bytes;
    float *data; /* pointer into map where float data begins */
    float *zero_vec;
    size_t zero_len;
};

mmapped_index_t *mmapped_index_open(const char *path, size_t vector_dim, size_t vector_size) {
    mmapped_index_t *idx = calloc(1, sizeof(*idx));
    if (!idx) return NULL;
    idx->path = NULL; idx->fd = -1; idx->map = NULL; idx->map_size = 0; idx->n = 0; idx->d = 0; idx->data = NULL;
    idx->zero_vec = NULL;
    idx->zero_len = 0;
    if (vector_dim > 0) {
        idx->zero_vec = calloc(vector_dim, sizeof(float));
        if (!idx->zero_vec) { free(idx); return NULL; }
        idx->zero_len = vector_dim;
        /* treat provided vector_dim as fallback dimension when file missing */
        idx->d = (uint32_t)vector_dim;
    }
    if (path) {
        idx->path = strdup(path);
        int fd = open(path, O_RDONLY);
        if (fd < 0) {
            /* missing file: keep zero-vector fallback */
            return idx;
        }
        struct stat st;
        if (fstat(fd, &st) != 0) { close(fd); return idx; }
        if (st.st_size < 8) { close(fd); return idx; }
        /* Try to read n,d header */
        uint32_t hdr[2];
        ssize_t rr = pread(fd, hdr, sizeof(hdr), 0);
        if (rr != sizeof(hdr)) { close(fd); return idx; }
        uint32_t n = hdr[0]; uint32_t d = hdr[1];
        /* validate that provided vector_dim matches header d (if header non-zero) */
        if (d == 0) { close(fd); return idx; }
        size_t vec_bytes = (size_t)d * sizeof(float);
        size_t expected = (size_t)8 + (size_t)n * vec_bytes;
        if ((size_t)st.st_size < expected) { close(fd); return idx; }
        void *map = mmap(NULL, expected, PROT_READ, MAP_PRIVATE, fd, 0);
        if (map == MAP_FAILED) { close(fd); return idx; }
        idx->fd = fd; idx->map = map; idx->map_size = expected;
        idx->n = n; idx->d = d; idx->expected_vector_bytes = vec_bytes;
        idx->data = (float *)((uint8_t *)map + 8);
    }
    return idx;
}

void mmapped_index_close(mmapped_index_t *idx) {
    if (!idx) return;
    if (idx->map && idx->map != MAP_FAILED) munmap(idx->map, idx->map_size);
    if (idx->fd >= 0) close(idx->fd);
    free(idx->path);
    if (idx->zero_vec) free(idx->zero_vec);
    free(idx);
}

int mmapped_index_get(const mmapped_index_t *idx, size_t i, float *out) {
    if (!idx || !out) return -1;
    if (idx->data) {
        if (i >= idx->n) return -1;
        const float *src = idx->data + ((size_t)i * (size_t)idx->d);
        memcpy(out, src, (size_t)idx->d * sizeof(float));
        return 0;
    }
    /* fallback: return zero vector sized by zero_vec allocation */
    if (idx->zero_vec) {
        size_t copy_floats = idx->zero_len ? idx->zero_len : 0;
        if (copy_floats == 0) return -1;
        memcpy(out, idx->zero_vec, copy_floats * sizeof(float));
        return 0;
    }
    return -1;
}

/* Lightweight wrapper for Python FFI convenience (optional) */
void* mage_mmapped_index_open(const char* path) {
    return (void*)mmapped_index_open(path, 0, 0);
}
void mage_mmapped_index_close(void* handle) { mmapped_index_close((mmapped_index_t*)handle); }
int mage_mmapped_index_get_vector(void* handle, uint32_t i, float* out_buf) { return mmapped_index_get((mmapped_index_t*)handle, i, out_buf); }

uint32_t mmapped_index_dim(const mmapped_index_t *idx) {
    if (!idx) return 0;
    if (idx->d != 0) return idx->d;
    if (idx->zero_len) return (uint32_t)idx->zero_len;
    return 0;
}
