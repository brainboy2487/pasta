#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "mage/contract.h"
#include "mage/types.h"
#include <math.h>

int main(void) {
    const size_t n = 8;
    const size_t block = 4;
    /* allocate 3 slices */
    mage_slice_t *slices = calloc(3, sizeof(mage_slice_t));
    if (!slices) return 2;

    /* HOT slice: dense matrix of ones */
    slices[0].tier = TIER_HOT;
    slices[0].n = n;
    slices[0].weight = 1.0;
    double *dense = malloc(sizeof(double)*n*n);
    for (size_t i=0;i<n*n;++i) dense[i]=1.0;
    slices[0].data.hot.dense_matrix = dense;

    /* WARM slice: rank-1 U,V^T filled with ones */
    slices[1].tier = TIER_WARM;
    slices[1].n = n;
    slices[1].weight = 1.0;
    size_t rank = 1;
    double *U = malloc(sizeof(double)*n*rank);
    double *Vt = malloc(sizeof(double)*rank*n);
    for (size_t i=0;i<n*rank;++i) U[i]=1.0;
    for (size_t i=0;i<rank*n;++i) Vt[i]=1.0;
    slices[1].data.warm.U = U;
    slices[1].data.warm.V_transposed = Vt;
    slices[1].data.warm.rank = rank;

    /* COLD slice: use fallback separable functions (no coefficients) */
    slices[2].tier = TIER_COLD;
    slices[2].n = n;
    slices[2].weight = 1.0;
    slices[2].data.cold.f_params.type = COLD_FUNC_POLYNOMIAL;
    slices[2].data.cold.f_params.coefficients = NULL;
    slices[2].data.cold.f_params.num_coeffs = 0;
    slices[2].data.cold.g_params = slices[2].data.cold.f_params;
    slices[2].data.cold.func_type = COLD_FUNC_POLYNOMIAL;

    uint32_t shortlist[3] = {0,1,2};
    double *out = calloc(block*block, sizeof(double));
    if (!out) return 3;

    int rc = contract_lazy_block(slices, shortlist, 3, 0, 0, block, out);
    if (rc != 0) {
        fprintf(stderr, "contract_lazy_block failed: %d\n", rc);
        return 4;
    }

    /* basic sanity checks */
    double sum = 0.0;
    for (size_t i=0;i<block*block;++i) {
        if (!isfinite(out[i])) { fprintf(stderr, "non-finite out[%zu]\n", i); return 5; }
        sum += out[i];
    }
    if (sum <= 0.0) { fprintf(stderr, "unexpected non-positive sum: %f\n", sum); return 6; }

    /* now precompute one block and run again to exercise cache path */
    if (contract_precompute_block(slices, 0, 0, 0, block) != 0) { fprintf(stderr, "precompute failed\n"); return 7; }
    memset(out,0,block*block*sizeof(double));
    rc = contract_lazy_block(slices, shortlist, 3, 0, 0, block, out);
    if (rc != 0) { fprintf(stderr, "contract_lazy_block failed after precompute: %d\n", rc); return 8; }

    sum = 0.0;
    for (size_t i=0;i<block*block;++i) sum += out[i];
    if (sum <= 0.0) { fprintf(stderr, "unexpected non-positive sum after precompute: %f\n", sum); return 9; }

    printf("contract test passed. sum=%f\n", sum);

    /* cleanup */
    free(dense); free(U); free(Vt); free(out); free(slices);
    return 0;
}
