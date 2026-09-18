#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <assert.h>
#include "mage/rp.h"

int main(void) {
    const size_t dim = 1024;
    const uint64_t seed = 0x12345678ULL;

    /* determinism checks */
    for (uint64_t k = 1; k <= 1000; ++k) {
        size_t i1 = mage_rp_index(k, dim, seed);
        int s1 = mage_hash_sign64(k, seed);
        size_t i2 = mage_rp_index(k, dim, seed);
        int s2 = mage_hash_sign64(k, seed);
        if (i1 >= dim || i2 >= dim) { printf("index out of range\n"); return 2; }
        if (s1 != s2) { printf("sign nondeterministic\n"); return 2; }
        if (i1 != i2) { printf("index nondeterministic\n"); return 2; }
    }

    /* basic distribution sanity (not strict test) */
    size_t counts_plus = 0, counts_minus = 0;
    for (uint64_t k = 1; k <= 10000; ++k) {
        int s = mage_hash_sign64(k, seed);
        if (s > 0) ++counts_plus; else ++counts_minus;
    }
    if (counts_plus == 0 || counts_minus == 0) { printf("bad sign distribution\n"); return 2; }

    printf("RP tests passed. +=%zu -=%zu\n", counts_plus, counts_minus);
    return 0;
}
