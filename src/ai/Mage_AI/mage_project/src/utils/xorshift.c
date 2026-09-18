/* xorshift.c - Deterministic RNG
 * Generated: 2026-01-23 15:32:35
 */

#include <stdint.h>
#include "mage/xorshift.h"

void xorshift32_init(xorshift32_state* s, uint32_t seed) {
    s->state = seed ? seed : 0xdeadbeefu;
}

uint32_t xorshift32_next(xorshift32_state* rng) {
    uint32_t x = rng->state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    rng->state = x;
    return x;
}
