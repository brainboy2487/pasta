/* include/gui.h - Thin GUI wrapper API for Mage presentation layer */
#ifndef MAGE_GUI_H
#define MAGE_GUI_H

#ifdef __cplusplus
extern "C" {
#endif

/* Initialize GUI subsystem. Returns 0 on success, non-zero on error. */
int mage_gui_init(void);

/* Run the GUI main loop (blocks until exit). */
int mage_gui_run(void);

/* Shutdown/cleanup GUI resources. */
void mage_gui_shutdown(void);

#ifdef __cplusplus
}
#endif

#endif /* MAGE_GUI_H */
