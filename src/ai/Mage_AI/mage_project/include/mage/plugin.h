/* plugin.h - simple plugin system API
 */
#ifndef MAGE_PLUGIN_H
#define MAGE_PLUGIN_H

#ifdef __cplusplus
extern "C" {
#endif

/* Load plugins from `plugins/` directory and call their init handlers. */
int mage_plugins_load(void);
void mage_plugins_unload(void);

#ifdef __cplusplus
}
#endif

#endif /* MAGE_PLUGIN_H */
