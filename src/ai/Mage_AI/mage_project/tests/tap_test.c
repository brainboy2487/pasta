/* tap_test.c - small unit test for tap_select_top
 */

#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include "mage/tap.h"

int main(void) {
    double scores[] = {0.1, 0.9, 0.5, 0.7, 0.2};
    size_t n = sizeof(scores)/sizeof(scores[0]);
    size_t S = 3;
    uint32_t out[3];
    size_t out_count = 0;
    if (tap_select_top(scores, n, S, out, &out_count) != 0) {
        fprintf(stderr, "tap_select_top failed\n");
        return 2;
    }
    if (out_count == 0) {
        fprintf(stderr, "no results\n");
        return 2;
    }
    printf("Top %zu indices:\n", out_count);
    for (size_t i = 0; i < out_count; ++i) printf(" %u", out[i]);
    printf("\n");
    return 0;
}
