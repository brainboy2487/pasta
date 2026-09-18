/* generator.h - helpers for generator tiers (separable block generator)
 */
#ifndef MAGE_GENERATOR_H
#define MAGE_GENERATOR_H

#include <stddef.h>

/* Fill `out` with f[i] * g[j] for i,j in [0, block_size).
 * `out` must be able to hold block_size*block_size doubles in row-major order.
 */
void generate_separable_from_vectors(const double* f, const double* g, size_t block_size, double* out);

#endif /* MAGE_GENERATOR_H */
