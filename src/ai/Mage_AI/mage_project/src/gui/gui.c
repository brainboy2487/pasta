/* src/gui/gui.c - Optional GLFW-backed GUI implementation with a no-op fallback.
 * This file implements the `include/gui.h` wrapper. It compiles without
 * GLFW and provides stub implementations; when compiled with -DUSE_GLFW it
 * expects GLFW headers and a basic setup to create a window.
 */

#include "gui.h"
#include <stdio.h>
#include "mage/api.h"

#ifdef USE_GLFW
#include <GLFW/glfw3.h>

static GLFWwindow* win = NULL;

int mage_gui_init(void) {
    if (!glfwInit()) return -1;
    win = glfwCreateWindow(640, 480, "Mage GUI (GLFW)", NULL, NULL);
    if (!win) { glfwTerminate(); return -1; }
    glfwMakeContextCurrent(win);
    return 0;
}

int mage_gui_run(void) {
    if (!win) return -1;
    while (!glfwWindowShouldClose(win)) {
        glClearColor(0.1f,0.1f,0.12f,1.0f);
        glClear(GL_COLOR_BUFFER_BIT);
        glfwPollEvents();
        glfwSwapBuffers(win);
    }
    return 0;
}

void mage_gui_shutdown(void) {
    if (win) { glfwDestroyWindow(win); win = NULL; }
    glfwTerminate();
}

#else

int mage_gui_init(void) {
    /* No-op fallback */
    return 0;
}

int mage_gui_run(void) {
    /* Minimal blocking behavior: run a small prompt loop via the API */
    printf("GUI not available. Running fallback prompt.\n");
    mage_api_init();
    char buf[1024];
    while (1) {
        printf("> "); fflush(stdout);
        if (!fgets(buf, sizeof(buf), stdin)) break;
        size_t n = strlen(buf); if (n && buf[n-1] == '\n') buf[n-1] = '\0';
        if (buf[0] == '\0') continue;
        char* parsed = mage_api_parse_prompt(buf);
        if (!parsed) { printf("(parse error)\n"); continue; }
        char* final = mage_api_finalize_reply(parsed);
        if (final) { printf("%s\n", final); free(final); } else { printf("%s\n", parsed); }
        free(parsed);
    }
    mage_api_shutdown();
    return 0;
}

void mage_gui_shutdown(void) { /* no-op */ }

#endif
