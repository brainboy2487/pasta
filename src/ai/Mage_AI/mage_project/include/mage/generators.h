#ifndef MAGE_GENERATORS_H
#define MAGE_GENERATORS_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Generator interfaces (stubs) used by API layer */

/* Generate a short string from vocab; caller frees return. Returns NULL on error. */
char* warm_generate_from_vocab(const char* prompt, const char* vocab_path, int max_tokens);
char* cold_generate_from_vocab(const char* prompt, const char* vocab_path, int max_tokens);

#ifdef __cplusplus
}
#endif

#endif /* MAGE_GENERATORS_H */
