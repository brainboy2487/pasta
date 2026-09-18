#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include "mage/rp.h"

int main(void) {
    const size_t dim = 128;
    const uint64_t seed = 0xfeedfaceULL;
    const size_t K = 4;
    size_t idx[K]; int sign[K];

    for (uint64_t k = 1; k <= 1000; ++k) {
        mage_rp_project_k(k, dim, seed, K, idx, sign, 1);
        /* check range and determinism */
        for (size_t t = 0; t < K; ++t) {
            if (idx[t] >= dim) { printf("idx out of range\n"); return 2; }
        }
        /* call again and compare */
        size_t idx2[K]; int sign2[K];
        mage_rp_project_k(k, dim, seed, K, idx2, sign2, 1);
        for (size_t t = 0; t < K; ++t) {
            if (idx[t] != idx2[t] || sign[t] != sign2[t]) { printf("nondet\n"); return 2; }
        }
    }
    printf("RP K-projection tests passed\n");
    return 0;
}
