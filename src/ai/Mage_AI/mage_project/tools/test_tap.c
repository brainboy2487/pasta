#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "mage/tap.h"

int main(void) {
    tap_config_t cfg = { .shortlist_size = 8, .confidence_threshold = 0.1, .enable_early_commit = 1 };
    tap_context_t* ctx = tap_context_create(&cfg);
    if (!ctx) { fprintf(stderr, "tap_context_create failed\n"); return 1; }

    /* Build minimal tensor metadata with 8 slices */
    ctx->tensor_meta->num_slices = 8;
    ctx->tensor_meta->state_dim = 16;
    ctx->tensor_meta->weights = malloc(sizeof(double)*8);
    ctx->tensor_meta->recency = malloc(sizeof(uint32_t)*8);
    for (size_t i = 0; i < 8; ++i) {
        ctx->tensor_meta->weights[i] = (i==3) ? 10.0 : 1.0; /* make slice 3 dominant */
        ctx->tensor_meta->recency[i] = (uint32_t)(i % 5);
    }

    tap_metrics_t metrics = {0};
    uint32_t next_state = 0;

    int rc = tap_parse_step(ctx, "hello", &next_state, &metrics);
    printf("tap_parse_step rc=%d next_state=%u ops=%u shortlist=%zu early=%d\n",
           rc, next_state, metrics.ops_used, metrics.shortlist_size, metrics.early_commit);

    tap_context_destroy(ctx);
    return 0;
}
