#include <stdio.h>
#include <stdint.h>
#include <inttypes.h>
#include <stdlib.h>
#include "mage/rp.h"

int main(int argc, char **argv) {
    const char *out = "tests/golden/rp_golden.txt";
    size_t dim = 1024;
    uint64_t seed = 0x12345678ULL;
    uint64_t start = 1, end = 2000;
    if (argc > 1) out = argv[1];
    if (argc > 2) dim = (size_t)atoi(argv[2]);
    if (argc > 3) seed = (uint64_t)strtoull(argv[3], NULL, 0);

    FILE *f = fopen(out, "w");
    if (!f) { perror(out); return 2; }
    for (uint64_t k = start; k <= end; ++k) {
        size_t idx; int sign;
        mage_rp_project(k, dim, seed, &idx, &sign);
        fprintf(f, "%" PRIu64 " %zu %d\n", k, idx, sign);
    }
    fclose(f);
    printf("Wrote %s entries %llu-%llu dim=%zu seed=0x%llx\n", out, (unsigned long long)start, (unsigned long long)end, dim, (unsigned long long)seed);
    return 0;
}
