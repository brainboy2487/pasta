#ifndef MAGE_IO_H
#define MAGE_IO_H

#include <stdint.h>
#include "mage/mmappedindex.h"

int write_vectors_bin(const char* path, const float* vecs, uint32_t n, uint32_t d);
int read_vectors_bin(const char* path, float** vecs, uint32_t* n, uint32_t* d);

/* Convenience wrappers to expose the mmapped index via the IO API */
mmapped_index_t* io_mmapped_open(const char* path);
int io_mmapped_get(mmapped_index_t* idx, uint32_t i, float* out_buf);
void io_mmapped_close(mmapped_index_t* idx);
uint32_t io_mmapped_dim(mmapped_index_t* idx);

#endif
