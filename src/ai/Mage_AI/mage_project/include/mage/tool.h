/* tool.h - Resolve repository-relative tools robustly
 */
#ifndef MAGE_TOOL_H
#define MAGE_TOOL_H

#ifdef __cplusplus
extern "C" {
#endif

/* Return malloc'd path to an executable for `rel` (e.g., "tools/gen_mphf").
 * Caller must free() the returned string. Returns NULL if not found.
 */
char* mage_find_tool(const char* rel);

#ifdef __cplusplus
}
#endif

#endif
