/* mage_gui_stub.c - Fallback GUI stub when GLFW is not available
 * Provides a minimal interactive loop that forwards lines to the mage API
 * so `bin/mage_gui` exists even on systems without OpenGL/GLFW dev headers.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "mage/api.h"

int main(int argc, char** argv) {
    (void)argc; (void)argv;
    if (mage_api_init() != 0) {
        fprintf(stderr, "mage_api_init failed\n");
        return 1;
    }
    printf("Mage GUI stub (no GLFW). Running simple interactive loop. Type EOF (Ctrl-D) to exit.\n");
    char line[2048];
    while (1) {
        printf("> "); fflush(stdout);
        if (!fgets(line, sizeof(line), stdin)) break;
        size_t n = strlen(line);
        if (n > 0 && line[n-1] == '\n') line[n-1] = '\0';
        if (line[0] == '\0') continue;
        char* parsed = mage_api_parse_prompt(line);
        if (!parsed) { printf("(parse error)\n"); continue; }
        char* final = mage_api_finalize_reply(parsed);
        if (final) {
            printf("%s\n", final);
            free(final);
        } else {
            printf("%s\n", parsed);
        }
        free(parsed);
    }
    mage_api_shutdown();
    printf("Goodbye.\n");
    return 0;
}
