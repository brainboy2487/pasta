/* bench_tap_shortlist.c - microbenchmark for tap_select_top performance */
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <stdint.h>
#include "mage/xorshift.h"
#include <sys/time.h>
#include "mage/tap.h"

int main(int argc, char** argv) {
    size_t n = 1000000; /* default 1M candidates */
    size_t S = 64;
    if (argc >= 2) n = (size_t)strtoull(argv[1], NULL, 10);
    if (argc >= 3) S = (size_t)strtoull(argv[2], NULL, 10);

    double *scores = malloc(sizeof(double) * n);
    if (!scores) return 1;
    /* Use deterministic xorshift RNG for reproducible benchmarks */
    xorshift32_state rng;
    xorshift32_init(&rng, 0x12345678u);
    for (size_t i = 0; i < n; ++i) {
        uint32_t v = xorshift32_next(&rng);
        scores[i] = (double)v / (double)UINT32_MAX;
    }

    uint32_t *out = malloc(sizeof(uint32_t) * S);
    size_t out_count = 0;
    struct timeval t0, t1;
    gettimeofday(&t0, NULL);
    if (tap_select_top(scores, n, S, out, &out_count) != 0) { free(scores); free(out); return 2; }
    gettimeofday(&t1, NULL);
    double elapsed = (t1.tv_sec - t0.tv_sec) + (t1.tv_usec - t0.tv_usec) / 1e6;
    printf("tap_select_top: n=%zu S=%zu out=%zu time=%.6f s\n", n, S, out_count, elapsed);

    free(scores); free(out);
    return 0;
}
