/* xorshift.h - simple deterministic RNG API */
#ifndef MAGE_XORSHIFT_H
#define MAGE_XORSHIFT_H

#include <stdint.h>

typedef struct {
    uint32_t state;
} xorshift32_state;

void xorshift32_init(xorshift32_state* s, uint32_t seed);
uint32_t xorshift32_next(xorshift32_state* s);

#endif /* MAGE_XORSHIFT_H */
