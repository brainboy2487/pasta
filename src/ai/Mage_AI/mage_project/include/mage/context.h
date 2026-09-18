/* context.h - simple conversation context history loader
 * Generated: 2026-01-23
 */
#ifndef MAGE_CONTEXT_H
#define MAGE_CONTEXT_H

#include <stddef.h>

int mage_context_load_from_jsonl(const char* path);
size_t mage_context_count(void);
const char* mage_context_get_role(size_t idx);
const char* mage_context_get_text(size_t idx);
void mage_context_clear(void);

#endif /* MAGE_CONTEXT_H */
