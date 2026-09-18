/* io.c - I/O Operations
 * Implementations for vector file read/write and mmapped index wrappers
 */

#define _POSIX_C_SOURCE 200809L

#include "mage/io.h"
#include "mage/mmappedindex.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/stat.h>

/* Write a vectors.bin file with header: uint32_t n, uint32_t d, followed
 * by n*d float32 values in row-major order.
 */
int write_vectors_bin(const char* filepath, const float* vectors,
                     uint32_t num_vectors, uint32_t dim) {
    if (!filepath || !vectors || num_vectors == 0 || dim == 0) return -1;
    FILE* f = fopen(filepath, "wb");
    if (!f) return -1;
    if (fwrite(&num_vectors, sizeof(uint32_t), 1, f) != 1) { fclose(f); return -1; }
    if (fwrite(&dim, sizeof(uint32_t), 1, f) != 1) { fclose(f); return -1; }
    size_t total = (size_t)num_vectors * (size_t)dim;
    if (fwrite(vectors, sizeof(float), total, f) != total) { fclose(f); return -1; }
    fclose(f);
    return 0;
}

/* Read a vectors.bin file into a newly-allocated float array. Caller must free *vecs.
 * On success returns 0 and sets *vecs, *n, *d. On error returns -1.
 */
int read_vectors_bin(const char* path, float** vecs, uint32_t* n, uint32_t* d) {
    if (!path || !vecs || !n || !d) return -1;
    FILE* f = fopen(path, "rb");
    if (!f) return -1;
    uint32_t nn = 0, dd = 0;
    if (fread(&nn, sizeof(uint32_t), 1, f) != 1) { fclose(f); return -1; }
    if (fread(&dd, sizeof(uint32_t), 1, f) != 1) { fclose(f); return -1; }
    if (nn == 0 || dd == 0) { fclose(f); return -1; }
    size_t total = (size_t)nn * (size_t)dd;
    float* data = malloc(total * sizeof(float));
    if (!data) { fclose(f); return -1; }
    size_t got = fread(data, sizeof(float), total, f);
    fclose(f);
    if (got != total) { free(data); return -1; }
    *vecs = data; *n = nn; *d = dd; return 0;
}

/* IO wrappers around the mmapped index implementation */
mmapped_index_t* io_mmapped_open(const char* path) {
    /* We pass 0/0 for vector dim/size since mmapped_index will read header */
    return mmapped_index_open(path, 0, 0);
}

int io_mmapped_get(mmapped_index_t* idx, uint32_t i, float* out_buf) {
    if (!idx || !out_buf) return -1;
    return mmapped_index_get(idx, (size_t)i, out_buf);
}

void io_mmapped_close(mmapped_index_t* idx) {
    if (!idx) return;
    mmapped_index_close(idx);
}

uint32_t io_mmapped_dim(mmapped_index_t* idx) {
    if (!idx) return 0;
    return mmapped_index_dim(idx);
}
