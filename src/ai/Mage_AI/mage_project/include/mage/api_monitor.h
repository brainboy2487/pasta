/* api_monitor.h - lightweight watchdog to monitor and restart modules
 * Provides a simple API for modules to register themselves, send heartbeats,
 * and supply a restart callback invoked when the monitor detects a timeout.
 */
#ifndef MAGE_API_MONITOR_H
#define MAGE_API_MONITOR_H

#ifndef MAGE_API_MONITOR_H
#define MAGE_API_MONITOR_H

#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
/* Opaque handle for a registered module */
typedef struct api_monitor_handle api_monitor_handle_t;

/* Initialize the monitor. Call once at process startup. Returns 0 on success. */
int api_monitor_init(void);

/* Shutdown the monitor cleanly. Blocks until background thread exits. */
void api_monitor_shutdown(void);

/* Register a module with the monitor.
 * - name: human-readable name (copied internally)
 * - timeout_ms: if no heartbeat received within this many ms, restart_fn is invoked
 * - ctx: user-provided context passed to restart_fn
 * - restart_fn: function pointer invoked to restart the module; should return 0 on success
 * Returns a non-NULL handle on success, NULL on error.
 */
api_monitor_handle_t* api_monitor_register_module(const char* name, unsigned int timeout_ms, void* ctx, int (*restart_fn)(void* ctx));

/* Unregister a module, freeing internal resources. Returns 0 on success. */
int api_monitor_unregister_module(api_monitor_handle_t* h);

/* Send a heartbeat for the given registered module. Returns 0 on success. */
int api_monitor_heartbeat(api_monitor_handle_t* h);

#ifdef __cplusplus
#endif

#endif /* MAGE_API_MONITOR_H */

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque handle for a registered module */
typedef struct api_monitor_handle api_monitor_handle_t;

/* Initialize the monitor. Call once at process startup. Returns 0 on success. */
int api_monitor_init(void);

/* Shutdown the monitor cleanly. Blocks until background thread exits. */
void api_monitor_shutdown(void);

/* Register a module with the monitor.
 * - name: human-readable name (copied internally)
 * - timeout_ms: if no heartbeat received within this many ms, restart_fn is invoked
 * - ctx: user-provided context passed to restart_fn
 * - restart_fn: function pointer invoked to restart the module; should return 0 on success
 * Returns a non-NULL handle on success, NULL on error.
 */
api_monitor_handle_t* api_monitor_register_module(const char* name, unsigned int timeout_ms, void* ctx, int (*restart_fn)(void* ctx));

/* Unregister a module, freeing internal resources. Returns 0 on success. */
int api_monitor_unregister_module(api_monitor_handle_t* h);

/* Send a heartbeat for the given registered module. Returns 0 on success. */
int api_monitor_heartbeat(api_monitor_handle_t* h);

#ifdef __cplusplus
}
#endif

#endif /* MAGE_API_MONITOR_H */
