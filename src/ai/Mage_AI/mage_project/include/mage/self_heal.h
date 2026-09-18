/* self_heal.h - Auto‑healing framework public API
 * Provides a lightweight self‑healing background service that monitors
 * file integrity, MPHF/vocab artifacts, and performs reflex repairs.
 */
#ifndef MAGE_SELF_HEAL_H
#define MAGE_SELF_HEAL_H

#ifdef __cplusplus
extern "C" {
#endif

/* Start the self‑healing background service. Returns 0 on success. */
int mage_self_heal_start(void);

/* Stop the self‑healing background service (blocks until stopped). */
void mage_self_heal_stop(void);

#ifdef __cplusplus
}
#endif

#endif /* MAGE_SELF_HEAL_H */
