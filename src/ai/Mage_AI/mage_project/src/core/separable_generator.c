/* separable_generator.c - simple separable block generator helper
 */

#include <stddef.h>
#include "mage/generator.h"

void generate_separable_from_vectors(const double* f, const double* g, size_t block_size, double* out) {
    if (!f || !g || !out) return;
    for (size_t i = 0; i < block_size; ++i) {
        for (size_t j = 0; j < block_size; ++j) {
            out[i * block_size + j] = f[i] * g[j];
        }
    }
}
