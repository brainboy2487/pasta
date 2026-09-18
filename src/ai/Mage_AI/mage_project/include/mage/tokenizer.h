/* tokenizer.h - improved tokenization utilities for Mage
 * Generated: 2026-01-23
 */
#ifndef MAGE_TOKENIZER_H
#define MAGE_TOKENIZER_H

#include <stddef.h>

/* Normalize a string: lower-case and keep alphanumeric and spaces.
 * Returns a newly allocated string which the caller must free.
 */
char* mage_normalize(const char* s);

/* Tokenize a UTF-8 / ASCII text into an array of freshly-allocated
 * null-terminated lower-case tokens. The caller receives `*tokens_out`
 * (an array of char*) and `*count_out`. Return 0 on success, -1 on error.
 */
int mage_tokenize(const char* text, char*** tokens_out, size_t* count_out);

/* Free tokens produced by `mage_tokenize`. */
void mage_free_tokens(char** tokens, size_t count);

#endif /* MAGE_TOKENIZER_H */
