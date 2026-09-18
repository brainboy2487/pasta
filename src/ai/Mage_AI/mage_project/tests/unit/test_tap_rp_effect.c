/* tests/unit/test_tap_rp_effect.c
 * Simple test demonstrating TAP shortlist behavior with different rp_K values.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include "mage/tap.h"
#include "mage/types.h"

int main(void) {
    /* Build a small synthetic tensor metadata with predictable weights */
    const size_t K = 128;

    tap_config_t cfg0 = {0};
    cfg0.shortlist_size = 16;
    cfg0.confidence_threshold = 0.15;
    cfg0.enable_early_commit = 0;
    cfg0.rp_K = 0; /* no RP */
    cfg0.rp_seed = 0x12345678ULL;

    tap_context_t* ctx0 = tap_context_create(&cfg0);
    if (!ctx0) { fprintf(stderr, "failed create ctx0\n"); return 2; }

    /* populate tensor_meta */
    ctx0->tensor_meta->num_slices = K;
    ctx0->tensor_meta->state_dim = 64;
    ctx0->tensor_meta->weights = malloc(sizeof(double) * K);
    ctx0->tensor_meta->recency = malloc(sizeof(uint32_t) * K);
    ctx0->tensor_meta->tier_costs = ctx0->tensor_meta->tier_costs ? ctx0->tensor_meta->tier_costs : malloc(sizeof(double)*3);
    for (size_t i = 0; i < K; ++i) {
        ctx0->tensor_meta->weights[i] = 1.0 + (double)(i % 5);
        ctx0->tensor_meta->recency[i] = (uint32_t)(i % 3);
    }
    /* ensure tier_costs present */
    ctx0->tensor_meta->tier_costs[0] = 0.66; ctx0->tensor_meta->tier_costs[1] = 7.1; ctx0->tensor_meta->tier_costs[2] = 38.7;

    tap_metrics_t metrics0 = {0};
    uint32_t next0 = 0;
    int rc0 = tap_parse_step(ctx0, "example_feature", &next0, &metrics0);
    if (rc0 != TAP_SUCCESS) { fprintf(stderr, "tap_parse_step failed rc=%d\n", rc0); }

    /* Now create a context using RP projections (rp_K = 8) */
    tap_config_t cfg1 = cfg0;
    cfg1.rp_K = 8;
    tap_context_t* ctx1 = tap_context_create(&cfg1);
    if (!ctx1) { fprintf(stderr, "failed create ctx1\n"); return 2; }
    /* copy the same tensor metadata into ctx1 for parity */
    ctx1->tensor_meta->num_slices = K;
    ctx1->tensor_meta->state_dim = 64;
    ctx1->tensor_meta->weights = malloc(sizeof(double) * K);
    ctx1->tensor_meta->recency = malloc(sizeof(uint32_t) * K);
    ctx1->tensor_meta->tier_costs = ctx1->tensor_meta->tier_costs ? ctx1->tensor_meta->tier_costs : malloc(sizeof(double)*3);
    for (size_t i = 0; i < K; ++i) {
        ctx1->tensor_meta->weights[i] = 1.0 + (double)(i % 5);
        ctx1->tensor_meta->recency[i] = (uint32_t)(i % 3);
    }
    ctx1->tensor_meta->tier_costs[0] = 0.66; ctx1->tensor_meta->tier_costs[1] = 7.1; ctx1->tensor_meta->tier_costs[2] = 38.7;

    tap_metrics_t metrics1 = {0};
    uint32_t next1 = 0;
    int rc1 = tap_parse_step(ctx1, "example_feature", &next1, &metrics1);
    if (rc1 != TAP_SUCCESS) { fprintf(stderr, "tap_parse_step failed rc=%d\n", rc1); }

    printf("tap next state without RP (rp_K=0): %u (shortlist=%zu ops=%u)\n", next0, metrics0.shortlist_size, metrics0.ops_used);
    printf("tap next state with RP  (rp_K=8): %u (shortlist=%zu ops=%u)\n", next1, metrics1.shortlist_size, metrics1.ops_used);

    /* Basic check: the chosen next state may differ when using RP projections.
     * We don't enforce a particular relationship here; the test demonstrates
     * that RP changes the shortlist/choice deterministically.
     */

    /* cleanup */
    if (ctx0) {
        if (ctx0->tensor_meta) {
            if (ctx0->tensor_meta->weights) free(ctx0->tensor_meta->weights);
            if (ctx0->tensor_meta->recency) free(ctx0->tensor_meta->recency);
            if (ctx0->tensor_meta->tier_costs) free(ctx0->tensor_meta->tier_costs);
            free(ctx0->tensor_meta);
        }
        free(ctx0);
    }
    if (ctx1) {
        if (ctx1->tensor_meta) {
            if (ctx1->tensor_meta->weights) free(ctx1->tensor_meta->weights);
            if (ctx1->tensor_meta->recency) free(ctx1->tensor_meta->recency);
            if (ctx1->tensor_meta->tier_costs) free(ctx1->tensor_meta->tier_costs);
            free(ctx1->tensor_meta);
        }
        free(ctx1);
    }

    return 0;
}
