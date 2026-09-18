/* plugin.c - simple dynamic plugin loader
 * Scans `plugins/<star>.so` and calls `mage_plugin_init()` and `mage_plugin_shutdown()` if present.
 */

#define _POSIX_C_SOURCE 200809L

#include "mage/plugin.h"
#include "mage/log.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <dirent.h>
#include <dlfcn.h>

typedef int (*plugin_init_fn)(void);
typedef void (*plugin_shutdown_fn)(void);

typedef struct plugin_handle { void* dl; plugin_shutdown_fn shutdown; struct plugin_handle* next; } plugin_handle_t;
static plugin_handle_t* plugins = NULL;

int mage_plugins_load(void) {
    DIR* d = opendir("plugins");
    if (!d) { mage_log(MAGE_LOG_INFO, "plugin: no plugins directory"); return 0; }
    struct dirent* e;
    int loaded = 0;
    while ((e = readdir(d)) != NULL) {
        const char* name = e->d_name;
        size_t L = strlen(name);
        if (L > 3 && strcmp(name + L - 3, ".so") == 0) {
            char path[1024]; snprintf(path, sizeof(path), "plugins/%s", name);
            void* dl = dlopen(path, RTLD_NOW);
            if (!dl) { mage_log(MAGE_LOG_WARN, "plugin: dlopen failed %s: %s", path, dlerror()); continue; }
            plugin_init_fn init = (plugin_init_fn)dlsym(dl, "mage_plugin_init");
            plugin_shutdown_fn shut = (plugin_shutdown_fn)dlsym(dl, "mage_plugin_shutdown");
            if (init) {
                int r = init();
                if (r != 0) { mage_log(MAGE_LOG_WARN, "plugin: init returned %d for %s", r, path); dlclose(dl); continue; }
                mage_log(MAGE_LOG_INFO, "plugin: loaded %s", path);
            } else {
                mage_log(MAGE_LOG_INFO, "plugin: no init in %s", path);
            }
            plugin_handle_t* h = malloc(sizeof(*h));
            h->dl = dl; h->shutdown = shut; h->next = plugins; plugins = h; loaded++;
        }
    }
    closedir(d);
    return loaded;
}

void mage_plugins_unload(void) {
    plugin_handle_t* h = plugins;
    while (h) {
        if (h->shutdown) {
            mage_log(MAGE_LOG_INFO, "plugin: calling shutdown");
            h->shutdown();
        }
        if (h->dl) dlclose(h->dl);
        plugin_handle_t* nx = h->next; free(h); h = nx;
    }
    plugins = NULL;
}
