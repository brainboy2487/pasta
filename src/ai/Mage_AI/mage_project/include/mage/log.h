/* log.h - simple leveled logging API for MAGE
 */
#ifndef MAGE_LOG_H
#define MAGE_LOG_H

#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef enum { MAGE_LOG_ERROR=0, MAGE_LOG_WARN=1, MAGE_LOG_INFO=2, MAGE_LOG_DEBUG=3 } mage_log_level_t;

int mage_log_init(mage_log_level_t level, const char* path);
void mage_log_close(void);
void mage_log_set_level(mage_log_level_t level);
mage_log_level_t mage_log_get_level(void);
void mage_log(mage_log_level_t level, const char* fmt, ...);

#ifdef __cplusplus
}
#endif

#endif /* MAGE_LOG_H */
