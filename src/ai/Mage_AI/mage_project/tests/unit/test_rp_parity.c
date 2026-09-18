#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <inttypes.h>
#include "mage/rp.h"

int main(void) {
    const char *golden = "tests/golden/rp_golden.txt";
    FILE *f = fopen(golden, "r");
    if (!f) { perror(golden); return 2; }

    size_t line = 0;
    uint64_t key; size_t idx; int sign;
    uint64_t seed = 0x12345678ULL;
    size_t dim = 1024;
    int failures = 0;

    while (fscanf(f, "%" SCNu64 " %zu %d\n", &key, &idx, &sign) == 3) {
        ++line;
        size_t out_idx = 0; int out_sign = 0;
        mage_rp_project(key, dim, seed, &out_idx, &out_sign);
        if (out_idx != idx || out_sign != sign) {
            printf("Mismatch at line %zu key=%" PRIu64 " got=(%zu,%d) want=(%zu,%d)\n",
                   line, key, out_idx, out_sign, idx, sign);
            failures++;
            if (failures > 20) break;
        }
    }
    fclose(f);
    if (failures) {
        printf("RP parity test failed (%d mismatches)\n", failures);
        return 2;
    }
    printf("RP parity passed (lines=%zu)\n", line);
    return 0;
}
