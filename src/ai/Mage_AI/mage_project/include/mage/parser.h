/* parser.h - Simple prompt parser and fuzzy lookup
 * Generated: 2026-01-23
 */
#ifndef MAGE_PARSER_H
#define MAGE_PARSER_H

#include <stddef.h>

/* Parse a user prompt and return a newly-allocated response string.
 * The caller is responsible for freeing the returned pointer.
 */
char* mage_parse_prompt(const char* prompt);

/* Print all help entries from data/help.json (if present).
 * This prints to stdout and returns 0 on success, -1 on error.
 */
int mage_print_help_all(void);

/* Train from a plain text file. The trainer builds a next-word frequency map
 * and augments the dynamic vocabulary with entries mapping tokens to their
 * most-likely following tokens. Returns the number of vocabulary entries
 * added on success, or -1 on error.
 */
int mage_train_from_file(const char* path);

/* Finalize a reply string by running a coherence check/postprocessing step.
 * Returns a newly-allocated string (caller must free). On error, returns
 * a copy of the original reply.
 */
char* mage_finalize_reply(const char* reply);

#endif /* MAGE_PARSER_H */
