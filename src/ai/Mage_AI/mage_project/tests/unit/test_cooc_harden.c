/* tests/unit/test_cooc_harden.c
 * Stress test for cooccurrence table resizing and correctness
 */

#include "mage/cooc.h"
#include <stdio.h>
#include <assert.h>

int main(void) {
    printf("test_cooc_harden...\n");
    cooc_table_t* t = cooc_table_create(4);
    assert(t != NULL);

    /* Insert many unique pairs to force resizing */
    const int N = 10000;
    for (int i = 0; i < N; ++i) {
        int a = (uint32_t)(i & 0xFFFF);
        int b = (uint32_t)((i * 1315423911u) & 0xFFFF);
        int rc = cooc_table_insert(t, a, b, 0);
        assert(rc == 0);
    }

    /* Verify some sampled entries exist and count is 1 */
    for (int i = 0; i < 100; ++i) {
        int idx = i * 97 % N;
        uint32_t a = (uint32_t)(idx & 0xFFFF);
        uint32_t b = (uint32_t)((idx * 1315423911u) & 0xFFFF);
        uint32_t c = cooc_table_get(t, a, b, 0);
        assert(c == 1);
    }

    cooc_table_destroy(t);
    printf("...PASSED\n");
    return 0;
}
