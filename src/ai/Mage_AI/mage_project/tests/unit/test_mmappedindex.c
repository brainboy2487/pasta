#include <stdio.h>
#include <stdlib.h>
#include "mage/mmappedindex.h"

int main(void) {
    /* Create an index with dimension 8 and test get/close lifecycle */
    mmapped_index_t *idx = mmapped_index_open("data/vectors.bin", 8, sizeof(float) * 8);
    if (!idx) {
        fprintf(stderr, "mmapped_index_open failed\n");
        return 2;
    }
    float vec[8];
    int r = mmapped_index_get(idx, 0, vec);
    if (r != 0) {
        fprintf(stderr, "mmapped_index_get failed: %d\n", r);
        mmapped_index_close(idx);
        return 3;
    }
    /* verify zeros (scaffold) */
    for (size_t i = 0; i < 8; ++i) {
        if (vec[i] != 0.0f) {
            fprintf(stderr, "expected zero vector element at %zu\n", i);
            mmapped_index_close(idx);
            return 4;
        }
    }
    mmapped_index_close(idx);
    return 0;
}
