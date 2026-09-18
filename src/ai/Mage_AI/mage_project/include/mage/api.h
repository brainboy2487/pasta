/* mage/api.h - Middleman API layer header
 *
 * Public, stable interface for interacting with core subsystems.
 *
 * Ownership and threading conventions:
 * - Functions that return pointer-to-data (char*, mmapped_index_t*, manifest_t*, etc.)
 *   return freshly-allocated objects unless otherwise documented. The caller is
 *   responsible for freeing those objects using the corresponding free/close
 *   API (e.g. mage_api_manifest_free, mage_api_mmapped_close, free()).
 * - The API is largely thread-safe for independent objects. Global initialization
 *   should be done once via `mage_api_init()` from the host process before
 *   invoking other APIs. Individual subsystems may have additional thread
 *   restrictions documented on the specific type.
 * - Error reporting: functions that return int return >=0 on success and
 *   negative values for errors (see MAGE_ERR_* below). Functions that return
 *   pointers return NULL on error.
 */

#ifndef MAGE_API_H
#define MAGE_API_H

#include <stddef.h>
#include <stdint.h>
#include "types.h"
#include "mage/mmappedindex.h"
#include "mage/manifest.h"

#ifdef __cplusplus
extern "C" {
#endif

/* Initialize / shutdown hooks. Call `mage_api_init()` once at process startup
 * and `mage_api_shutdown()` prior to exit to allow orderly teardown of
 * background services (self-heal, plugins, log flush).
 */
int mage_api_init(void);
void mage_api_shutdown(void);

/* Print help for all subsystems to stdout. Returns 0 on success. */
int mage_api_print_help_all(void);

/* Standardized error codes returned by API functions when negative. */
#define MAGE_OK 0
#define MAGE_ERR_GENERIC -1
#define MAGE_ERR_INVALID_ARG -2
#define MAGE_ERR_IO -3
#define MAGE_ERR_NOT_FOUND -4

/* Training ---------------------------------------------------------------*/
/* Train from an input file (plain text or JSONL). Returns number of entries
 * processed (>=0) on success or a negative MAGE_ERR_* code on error.
 */
int mage_api_train_from_file(const char* path);

/* Parsing / Prompting ---------------------------------------------------*/
/* Parse a user prompt into an internal representation. Returns a newly
 * allocated string containing the parsed result. Caller must free the
 * returned pointer. Returns NULL on error.
 */
char* mage_api_parse_prompt(const char* prompt);

/* High-level generate: takes a prompt, runs parsing + generation + finalizer
 * and returns a newly-allocated JSON string {"generated": "..."}.
 * Caller must free the returned string. Returns NULL on error.
 */
char* mage_api_generate(const char* prompt);

/* Context utilities -----------------------------------------------------*/
/* Load contextual data (JSONL) into memory for retrieval/generation.
 * Returns 0 on success or negative error code on failure.
 */
int mage_api_context_load_from_jsonl(const char* path);

/* MPHF probe: returns 1 if the MPHF file at `path` is loadable, 0 otherwise.
 */
int mage_api_mphf_probe(const char* path);

/* Finalize reply via coherence checker. Returns newly-allocated string
 * (caller frees) or NULL on error.
 */
char* mage_api_finalize_reply(const char* reply);

/* Contract management helpers -------------------------------------------*/
/* Set contract cache capacity (entries). Returns 0 on success or negative
 * error code.
 */
int mage_api_contract_set_cache(size_t entries);
void mage_api_contract_flush(void);
int mage_api_contract_precompute(const mage_slice_t* slices, uint32_t slice_idx, size_t row_start, size_t col_start, size_t block_size);

/* MMapped index access via API layer ------------------------------------*/
/* Open a mmapped index; returns an opaque handle or NULL on error. Caller
 * must call `mage_api_mmapped_close()` to release resources.
 */
mmapped_index_t* mage_api_mmapped_open(const char* path);
int mage_api_mmapped_get(mmapped_index_t* idx, uint32_t i, float* out_buf);
void mage_api_mmapped_close(mmapped_index_t* idx);
uint32_t mage_api_mmapped_dim(mmapped_index_t* idx);

/* Manifest helpers ------------------------------------------------------*/
manifest_t* mage_api_manifest_load(const char* path);
const manifest_entry_t* mage_api_manifest_get(const manifest_t* m, uint32_t idx);
void mage_api_manifest_free(manifest_t* m);

/* Vectorization & Retrieval helpers ------------------------------------*/
/* Vectorize text into `out_buf` of length `dim`. Returns 0 on success or
 * negative error code on failure.
 */
int mage_api_vectorize_text(const char* text, float* out_buf, uint32_t dim);

/* retrieve: returns newly-allocated JSON string with results; caller must free.
 * Returns NULL on error.
 */
char* mage_api_retrieve(const float* query, uint32_t dim, uint32_t k);

/* Tokenizer API exposed to presentation layers --------------------------*/
/* Tokenize text into an array of C-strings. On success returns 0 and sets
 * `*tokens_out` to a newly-allocated array of `*count_out` pointers. The
 * caller must release the tokens via `mage_api_free_tokens(tokens, count)`.
 */
int mage_api_tokenize(const char* text, char*** tokens_out, size_t* count_out);
void mage_api_free_tokens(char** tokens, size_t count);

/* Build a vocabulary JSONL from an input file (plain text or JSONL) and write
 * it to `out_path`. Returns number of entries written (>=0) or negative
 * error code on failure. The `min_count` parameter filters low-frequency
 * tokens.
 */
int mage_api_build_vocab_from_file(const char* input_path, const char* out_path, int min_count);

#ifdef __cplusplus
}
#endif

#endif /* MAGE_API_H */
