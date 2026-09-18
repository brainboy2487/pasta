/* Minimal mmapped index scaffold header */
#ifndef MAGE_MMAP_INDEX_H
#define MAGE_MMAP_INDEX_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct mmapped_index mmapped_index_t;

/* Open a memory-mapped vector index.
 * File format (optional): uint32_t n, uint32_t d, followed by n*d float32 values.
 * If `vector_dim` is non-zero it will be used for zero-fallback allocation;
 * otherwise the header d is authoritative when present in the file.
 * Returns an allocated mmapped_index_t* or NULL on error.
 */
mmapped_index_t *mmapped_index_open(const char *path, size_t vector_dim, size_t vector_size);
void mmapped_index_close(mmapped_index_t *idx);

/* Fetch vector at index `i` into `out` (must have room for `mmapped_index_dim(idx)` floats).
 * Returns 0 on success, non-zero on failure.
 */
int mmapped_index_get(const mmapped_index_t *idx, size_t i, float *out);

/* Return the vector dimension `d` as read from the file header, or 0 if unknown.
 * Callers can use this to size buffers dynamically.
 */
uint32_t mmapped_index_dim(const mmapped_index_t *idx);

/* Lightweight C wrappers exposed for Python FFI convenience */
void* mage_mmapped_index_open(const char* path);
void mage_mmapped_index_close(void* handle);
int mage_mmapped_index_get_vector(void* handle, uint32_t i, float* out_buf);

#ifdef __cplusplus
}
#endif

#endif /* MAGE_MMAP_INDEX_H */
