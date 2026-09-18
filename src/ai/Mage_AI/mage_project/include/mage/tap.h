/* tap.h - TAP (Top-k Answer Parser) public interface
 * Consolidated public header exposing TAP shortlist and parse functionality.
 */

#ifndef MAGE_TAP_H
#define MAGE_TAP_H

#include <stddef.h>
#include <stdint.h>
#include "types.h"

#ifdef __cplusplus
extern "C" {
#endif

/* ------------------------------------------------------------------------ */
/* Shortlist selection (heap-based utility)                                    */
/* Select the top S indices from an array of scores of length n.
 * - scores: array of length n
 * - n: number of candidates
 * - S: desired shortlist size (max)
 * - out: pre-allocated array of uint32_t of size at least S to receive indices
 * - out_count: pointer to size_t that will receive the number of indices written
 * Returns 0 on success, negative value on error.
 */
int tap_select_top(const double* scores, size_t n, size_t S, uint32_t* out, size_t* out_count);

/* ------------------------------------------------------------------------ */
/* TAP parse-step API                                                          */
/* Performs one bounded TAP parse step using the provided context and input
 * feature. Produces the next state and optional metrics.
 * Returns TAP_SUCCESS (0) on success or a TAP_ERROR_* code on failure.
 */
int tap_parse_step(tap_context_t* ctx,
                   const char* input_feature,
                   uint32_t* next_state,
                   tap_metrics_t* metrics);

/* Context management */
tap_context_t* tap_context_create(const tap_config_t* config);
void tap_context_destroy(tap_context_t* ctx);

#ifdef __cplusplus
}
#endif

#endif /* MAGE_TAP_H */
