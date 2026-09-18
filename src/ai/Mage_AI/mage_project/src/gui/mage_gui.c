/* mage_gui.c - Refactored GUI wrapper that reuses CLI/API functions
 *
 * Goals:
 *  - Use mage_api_* functions where possible instead of spawning the CLI binary.
 *  - Centralize command handling so GUI and CLI share logic.
 *  - Safer string handling and clearer control flow.
 *
 * Drop-in replacement for the original mage_gui.c
 */

#define _POSIX_C_SOURCE 200809L
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <stddef.h>
#include <unistd.h>
#include <fcntl.h>
#include <limits.h>
#include <libgen.h>
#include <stdarg.h>

#include <GLFW/glfw3.h>

#include "mage/api.h"
#include "mage/subproc.h"
#include "mage/log.h"

/* stb_easy_font single-file lib (same as original) */
#define STB_EASY_FONT_IMPLEMENTATION
#include "../gui/stb_easy_font.h"
#include "gui.h"

/* UI constants */
#define TAB_CHAT   0
#define TAB_TRAIN  1
#define TAB_SCRAPER 2
#define TAB_DEBUG  3

/* Limits */
#define MAX_MESSAGES 1024
#define INPUT_BUF_SZ 1024
#define CLI_CMD_SZ 1024
#define SCRAPER_URL_SZ 2048
#define SCRAPER_OUT_SZ 512
#define LOG_PATH "logs/mage.log"

/* Global UI state */
static int gui_current_tab = TAB_CHAT;
static int input_active_field = 0;
static char *messages[MAX_MESSAGES];
static int msg_count = 0;

static char input_buf[INPUT_BUF_SZ];
static int input_len = 0;

typedef struct { int top_k, max_nexts, sample_limit, verbose; } train_settings_t;
static train_settings_t TRAIN_SETTINGS = {5,5,0,0};

typedef struct { int online; char host[64]; int port; char username[64]; char password[64]; } api_settings_t;
static api_settings_t API_SETTINGS = {0, "127.0.0.1", 8080, "", ""};
static int api_active_field = 0;

static GLFWwindow *window = NULL;
static int use_cli_backend = 0;

/* CLI command buffer (for debug tab) */
static char cli_cmd_buf[CLI_CMD_SZ];
static int cli_cmd_len = 0;

/* Scraper fields */
static char scraper_url[SCRAPER_URL_SZ];
static int scraper_url_len = 0;
static char scraper_out[SCRAPER_OUT_SZ];
static int scraper_out_len = 0;
static int scraper_active_field = 0; /* 1=url, 2=out */

/* Forward declarations */
static void append_message(const char *who, const char *text);
static void draw_text(float x, float y, const char *text);
static char* run_cli_prompt_via_subproc(const char *prompt);
static char* run_generate_via_api(const char *prompt);
static int run_train_via_api(const char *path);
static void save_api_settings(void);
static void load_api_settings(void);
static void save_train_settings(void);
static void load_train_settings(void);

/* ---------- Input handling ---------- */

static void gui_char_callback(GLFWwindow *w, unsigned int codepoint) {
    (void)w;
    if (codepoint > 127) return; /* ASCII-only for now */
    char c = (char)codepoint;
    if (cli_cmd_len + 1 < (int)sizeof(cli_cmd_buf) && glfwGetKey(window, GLFW_KEY_LEFT_SHIFT) != GLFW_PRESS) {
        /* prefer CLI field only when active */
    }
    if (scraper_active_field == 1) {
        if (scraper_url_len + 1 < (int)sizeof(scraper_url)) { scraper_url[scraper_url_len++] = c; scraper_url[scraper_url_len] = '\0'; }
    } else if (scraper_active_field == 2) {
        if (scraper_out_len + 1 < (int)sizeof(scraper_out)) { scraper_out[scraper_out_len++] = c; scraper_out[scraper_out_len] = '\0'; }
    } else if (cli_cmd_len > 0 || glfwGetKey(window, GLFW_KEY_TAB) == GLFW_PRESS) {
        if (cli_cmd_len + 1 < (int)sizeof(cli_cmd_buf)) { cli_cmd_buf[cli_cmd_len++] = c; cli_cmd_buf[cli_cmd_len] = '\0'; }
    } else {
        if (input_len + 1 < (int)sizeof(input_buf)) { input_buf[input_len++] = c; input_buf[input_len] = '\0'; }
    }
}

/* ---------- Utilities ---------- */

static void append_message(const char *who, const char *text) {
    if (!who || !text) return;
    if (msg_count >= MAX_MESSAGES) {
        free(messages[0]);
        memmove(messages, messages + 1, (MAX_MESSAGES - 1) * sizeof(char*));
        msg_count--;
    }
    size_t need = strlen(who) + 2 + strlen(text) + 1;
    char *m = malloc(need);
    if (!m) return;
    snprintf(m, need, "%s: %s", who, text);
    messages[msg_count++] = m;

    FILE *f = fopen(LOG_PATH, "a");
    if (f) {
        time_t t = time(NULL);
        struct tm tmv;
        localtime_r(&t, &tmv);
        char timestr[64];
        strftime(timestr, sizeof(timestr), "%Y-%m-%d %H:%M:%S", &tmv);
        fprintf(f, "%s %s: %s\n", timestr, who, text);
        fclose(f);
    }
}

static void draw_text(float x, float y, const char *text) {
    if (!text) return;
    unsigned char color[4] = {255,255,255,255};
    char buffer[4096];
    int vcount = stb_easy_font_print((int)x, (int)y, (char*)text, color, buffer, sizeof(buffer));
    if (vcount > 0) {
        glColor3f(1.0f, 1.0f, 1.0f);
        glEnableClientState(GL_VERTEX_ARRAY);
        glVertexPointer(2, GL_FLOAT, 16, buffer);
        glDrawArrays(GL_QUADS, 0, vcount * 4);
        glDisableClientState(GL_VERTEX_ARRAY);
    }
}

/* Run the legacy CLI binary via supervised subproc (used only when CLI backend selected) */
static char* run_cli_prompt_via_subproc(const char *prompt) {
    if (!prompt) return NULL;
    const char *argvv[] = {"./bin/mage_cli", NULL};
    char *out = NULL;
    int exitcode = -1;
    if (subproc_run_capture_stdin(argvv, prompt, 30000, &out, &exitcode) != 0) {
        if (out) { free(out); out = NULL; }
        return NULL;
    }
    if (!out) return NULL;
    /* trim trailing newlines */
    size_t used = strlen(out);
    while (used > 0 && (out[used-1] == '\n' || out[used-1] == '\r')) { out[used-1] = '\0'; used--; }
    return out;
}

/* Prefer API-based generation when possible */
static char* run_generate_via_api(const char *prompt) {
    if (!prompt) return NULL;
    /* mage_api_generate returns a JSON string like {"generated": "..."}. Use it directly. */
    char *js = mage_api_generate(prompt);
    if (!js) return NULL;
    return js;
}

/* Prefer API-based training when possible */
static int run_train_via_api(const char *path) {
    if (!path) return MAGE_ERR_INVALID_ARG;
    return mage_api_train_from_file(path);
}

/* Save/load small JSON-like settings (simple, robust parsing) */
static void save_api_settings(void) {
    FILE *f = fopen("services/api/config.json", "w");
    if (!f) return;
    fprintf(f, "{\n  \"online\": %s,\n  \"host\": \"%s\",\n  \"port\": %d,\n  \"username\": \"%s\",\n  \"password\": \"%s\"\n}\n",
            API_SETTINGS.online ? "true" : "false",
            API_SETTINGS.host,
            API_SETTINGS.port,
            API_SETTINGS.username,
            API_SETTINGS.password);
    fclose(f);
}

static void load_api_settings(void) {
    FILE *f = fopen("services/api/config.json", "r");
    if (!f) return;
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);
    if (sz <= 0) { fclose(f); return; }
    char *buf = malloc((size_t)sz + 1);
    if (!buf) { fclose(f); return; }
    size_t n = fread(buf, 1, (size_t)sz, f);
    buf[n] = '\0';
    char *p;
    if ((p = strstr(buf, "\"online\""))) API_SETTINGS.online = (strstr(p, "true") != NULL);
    if ((p = strstr(buf, "\"host\""))) {
        char *q = strchr(p, ':');
        if (q) {
            char *s = strchr(q, '"');
            if (s) { s++; char *e = strchr(s, '"'); if (e) { size_t ln = (size_t)(e - s); if (ln >= sizeof(API_SETTINGS.host)) ln = sizeof(API_SETTINGS.host) - 1; memcpy(API_SETTINGS.host, s, ln); API_SETTINGS.host[ln] = '\0'; } }
        }
    }
    if ((p = strstr(buf, "\"port\""))) {
        int v;
        if (sscanf(p, "\"port\"%*[^0-9]%d", &v) == 1) API_SETTINGS.port = v;
    }
    if ((p = strstr(buf, "\"username\""))) {
        char *q = strchr(p, ':');
        if (q) {
            char *s = strchr(q, '"');
            if (s) { s++; char *e = strchr(s, '"'); if (e) { size_t ln = (size_t)(e - s); if (ln >= sizeof(API_SETTINGS.username)) ln = sizeof(API_SETTINGS.username) - 1; memcpy(API_SETTINGS.username, s, ln); API_SETTINGS.username[ln] = '\0'; } }
        }
    }
    if ((p = strstr(buf, "\"password\""))) {
        char *q = strchr(p, ':');
        if (q) {
            char *s = strchr(q, '"');
            if (s) { s++; char *e = strchr(s, '"'); if (e) { size_t ln = (size_t)(e - s); if (ln >= sizeof(API_SETTINGS.password)) ln = sizeof(API_SETTINGS.password) - 1; memcpy(API_SETTINGS.password, s, ln); API_SETTINGS.password[ln] = '\0'; } }
        }
    }
    free(buf);
    fclose(f);
}

static void save_train_settings(void) {
    FILE *f = fopen("data/train_settings.json", "w");
    if (!f) return;
    fprintf(f, "{\n  \"top_k\": %d,\n  \"max_nexts\": %d,\n  \"sample_limit\": %d,\n  \"verbose\": %d\n}\n",
            TRAIN_SETTINGS.top_k, TRAIN_SETTINGS.max_nexts, TRAIN_SETTINGS.sample_limit, TRAIN_SETTINGS.verbose);
    fclose(f);
}

static void load_train_settings(void) {
    FILE *f = fopen("data/train_settings.json", "r");
    if (!f) return;
    char buf[256];
    while (fgets(buf, sizeof(buf), f)) {
        char *p;
        if ((p = strstr(buf, "\"top_k\""))) sscanf(p, "\"top_k\" : %d", &TRAIN_SETTINGS.top_k);
        if ((p = strstr(buf, "\"max_nexts\""))) sscanf(p, "\"max_nexts\" : %d", &TRAIN_SETTINGS.max_nexts);
        if ((p = strstr(buf, "\"sample_limit\""))) sscanf(p, "\"sample_limit\" : %d", &TRAIN_SETTINGS.sample_limit);
        if ((p = strstr(buf, "\"verbose\""))) sscanf(p, "\"verbose\" : %d", &TRAIN_SETTINGS.verbose);
    }
    fclose(f);
}

/* ---------- GUI lifecycle ---------- */

int mage_gui_init(void) {
    if (!glfwInit()) return -1;
    glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 2);
    glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 0);
    window = glfwCreateWindow(900, 640, "Mage GUI", NULL, NULL);
    if (!window) { glfwTerminate(); window = NULL; return -1; }
    glfwMakeContextCurrent(window);
    glfwSetCharCallback(window, gui_char_callback);

    load_train_settings();
    load_api_settings();
    append_message("system", "Mage GUI started");
    return 0;
}

/* Helper: handle a chat send action (used by Send button and Enter key) */
static void handle_chat_send(void) {
    if (input_len == 0) return;
    input_buf[input_len] = '\0';
    append_message("you", input_buf);

    if (use_cli_backend) {
        char *r = run_cli_prompt_via_subproc(input_buf);
        if (r) { append_message("cli", r); free(r); }
        else append_message("system", "CLI failed to respond");
    } else {
        /* Prefer API generate (returns JSON). If it fails, fall back to parse/finalize pair. */
        char *js = run_generate_via_api(input_buf);
        if (js) {
            /* show raw JSON so user can inspect generated payload */
            append_message("mage", js);
            free(js);
        } else {
            /* fallback: parse + finalize */
            char *parsed = mage_api_parse_prompt(input_buf);
            if (parsed) {
                char *final = mage_api_finalize_reply(parsed);
                if (final) { append_message("mage", final); free(final); }
                else append_message("mage", parsed);
                free(parsed);
            } else {
                append_message("mage", "(no response)");
            }
        }
    }
    input_len = 0;
    input_buf[0] = '\0';
}

/* Helper: handle training action */
static void handle_train_send(void) {
    if (input_len == 0) { append_message("system", "Provide a path to a training file in the input box"); return; }
    input_buf[input_len] = '\0';
    append_message("system", "Starting training...");
    int added = run_train_via_api(input_buf);
    if (added < 0) {
        /* try data/<name> fallback */
        char alt[PATH_MAX];
        snprintf(alt, sizeof(alt), "data/%s", input_buf);
        added = run_train_via_api(alt);
    }
    if (added < 0) append_message("system", "Training failed (see logs)");
    else {
        char tmp[128];
        snprintf(tmp, sizeof(tmp), "Training complete: %d entries added", added);
        append_message("system", tmp);
    }
    input_len = 0;
    input_buf[0] = '\0';
}

/* Helper: run scraper (calls python tool via supervised subproc) */
static void handle_scraper_run(int train_after) {
    if (scraper_url_len == 0 || scraper_out_len == 0) { append_message("system", "Provide URL and output filename first"); return; }
    char urltmp[SCRAPER_URL_SZ];
    char outtmp[SCRAPER_OUT_SZ];
    strncpy(urltmp, scraper_url, sizeof(urltmp)-1); urltmp[sizeof(urltmp)-1] = '\0';
    strncpy(outtmp, scraper_out, sizeof(outtmp)-1); outtmp[sizeof(outtmp)-1] = '\0';
    char cmd[4096];
    snprintf(cmd, sizeof(cmd), "python3 python/tools/scraper.py --url '%s' --out '%s'", urltmp, outtmp);
    append_message("system", "Running scraper...");
    int rc = system(cmd);
    if (rc == 0) {
        append_message("system", "Scrape complete");
        if (train_after) {
            append_message("system", "Starting training on scraped output...");
            int added = run_train_via_api(outtmp);
            if (added < 0) append_message("system", "Training failed (see logs)");
            else {
                char tmp[128];
                snprintf(tmp, sizeof(tmp), "Training complete: %d entries added", added);
                append_message("system", tmp);
            }
        }
    } else {
        append_message("system", "Scrape failed (see console)");
    }
}

/* Helper: run CLI command from debug tab (prefer API equivalents) */
static void handle_debug_cli_run(void) {
    if (cli_cmd_len == 0) { append_message("system", "No CLI command to run"); return; }
    cli_cmd_buf[cli_cmd_len] = '\0';

    /* Recognize a few commands and call API directly where possible */
    if (strncmp(cli_cmd_buf, "help -all", 9) == 0) {
        if (mage_api_print_help_all() != 0) append_message("system", "No help.json found");
        else append_message("system", "Help printed to stdout");
    } else if (strncmp(cli_cmd_buf, "contract flush", 14) == 0) {
        mage_api_contract_flush();
        append_message("system", "Contract cache flushed");
    } else if (strncmp(cli_cmd_buf, "vectors close", 13) == 0) {
        /* nothing to do here; mmapped handles are local to CLI; just inform user */
        append_message("system", "Vectors close requested (no-op in GUI)");
    } else if (strncmp(cli_cmd_buf, "validate-vocab", 14) == 0) {
        /* run python validator if present */
        char cmd[4096];
        snprintf(cmd, sizeof(cmd), "python3 tools/validate_vocab.py %s", cli_cmd_buf + 15);
        int rc = system(cmd);
        if (rc == 0) append_message("system", "Vocab validated");
        else append_message("system", "Vocab validation failed");
    } else {
        /* fallback: run via subproc */
        char *out = run_cli_prompt_via_subproc(cli_cmd_buf);
        if (out) { append_message("cli", out); free(out); }
        else append_message("system", "CLI execution failed");
    }

    cli_cmd_len = 0;
    cli_cmd_buf[0] = '\0';
}

/* Main GUI loop */
int mage_gui_run(void) {
    if (!window) return -1;
    int current_tab = gui_current_tab;
    int debug_visible = 0;

    while (!glfwWindowShouldClose(window)) {
        int w, h;
        glfwGetFramebufferSize(window, &w, &h);
        glViewport(0, 0, w, h);
        glMatrixMode(GL_PROJECTION); glLoadIdentity(); glOrtho(0, w, h, 0, -1, 1);
        glMatrixMode(GL_MODELVIEW); glLoadIdentity();
        glClearColor(0.08f, 0.08f, 0.11f, 1.0f);
        glClear(GL_COLOR_BUFFER_BIT);

        /* Draw recent messages (from bottom up) */
        int y = 20;
        for (int i = msg_count - 1; i >= 0 && y < h - 120; --i) {
            draw_text(10, y, messages[i]);
            y += 18;
        }

        /* Input box background */
        glColor3f(0.2f, 0.2f, 0.25f);
        glBegin(GL_QUADS);
        glVertex2f(10, h - 80); glVertex2f(w - 10, h - 80);
        glVertex2f(w - 10, h - 40); glVertex2f(10, h - 40);
        glEnd();

        /* Input text */
        draw_text(14, h - 66, input_buf);

        /* Tabs */
        draw_text(14, 10, "[Chat]");
        draw_text(100, 10, "[Train]");
        draw_text(200, 10, "[Scraper]");
        draw_text(320, 10, "[Debug]");

        /* CLI toggle */
        glColor3f(use_cli_backend ? 0.0f : 0.2f, use_cli_backend ? 0.6f : 0.2f, 0.2f);
        glBegin(GL_QUADS);
        glVertex2f(w - 120, 8); glVertex2f(w - 80, 8);
        glVertex2f(w - 80, 36); glVertex2f(w - 120, 36);
        glEnd();
        draw_text(w - 112, 14, use_cli_backend ? "CLI: ON" : "CLI: OFF");

        /* Send button */
        glColor3f(0.15f, 0.45f, 0.15f);
        glBegin(GL_QUADS);
        glVertex2f(w - 80, h - 76); glVertex2f(w - 12, h - 76);
        glVertex2f(w - 12, h - 44); glVertex2f(w - 80, h - 44);
        glEnd();
        draw_text(w - 68, h - 66, "Send");

        /* Scraper Run button (placeholder position) */
        glColor3f(0.25f, 0.25f, 0.45f);
        glBegin(GL_QUADS);
        glVertex2f(300, h - 240); glVertex2f(420, h - 240);
        glVertex2f(420, h - 260); glVertex2f(300, h - 260);
        glEnd();
        draw_text(310, h - 254, "Run Scraper");

        /* If scraper tab active, show header */
        if (current_tab == TAB_SCRAPER) {
            draw_text(60, 60, "Scraper: enter a URL and output JSONL filename below");
        }

        glfwPollEvents();

        /* Mouse handling */
        if (glfwGetMouseButton(window, GLFW_MOUSE_BUTTON_LEFT) == GLFW_PRESS) {
            double mx, my;
            glfwGetCursorPos(window, &mx, &my);

            /* Tab clicks */
            if (my >= 0 && my <= 40) {
                if (mx >= 10 && mx <= 70) current_tab = TAB_CHAT;
                else if (mx >= 90 && mx <= 160) current_tab = TAB_TRAIN;
                else if (mx >= 190 && mx <= 270) current_tab = TAB_SCRAPER;
                else if (mx >= 310 && mx <= 380) current_tab = TAB_DEBUG;
                gui_current_tab = current_tab;
            }

            /* Input focus */
            if (mx >= 12 && mx <= (w - 84) && my >= (h - 80) && my <= (h - 40)) {
                input_active_field = 1;
                cli_cmd_len = 0;
                scraper_active_field = 0;
            } else {
                input_active_field = 0;
            }

            /* CLI toggle */
            if (mx >= w - 120 && mx <= w - 80 && my >= 8 && my <= 36) {
                use_cli_backend = !use_cli_backend;
                append_message("system", use_cli_backend ? "CLI backend enabled" : "CLI backend disabled");
            }

            /* Send button click */
            if (mx >= w - 80 && mx <= w - 12 && my >= h - 76 && my <= h - 44) {
                if (current_tab == TAB_CHAT) handle_chat_send();
                else if (current_tab == TAB_TRAIN) handle_train_send();
            }

            /* Scraper tab clicks */
            if (current_tab == TAB_SCRAPER) {
                /* URL box */
                if (mx >= 60 && mx <= w - 60 && my >= 120 && my <= 140) {
                    scraper_active_field = 1;
                    input_active_field = 0;
                }
                /* Out box */
                if (mx >= 60 && mx <= 360 && my >= 150 && my <= 170) {
                    scraper_active_field = 2;
                    input_active_field = 0;
                }
                /* Scrape button */
                if (mx >= 60 && mx <= 200 && my >= 180 && my <= 210) {
                    handle_scraper_run(0);
                }
                /* Scrape & Train */
                if (mx >= 220 && mx <= 420 && my >= 180 && my <= 210) {
                    handle_scraper_run(1);
                }
            }

            /* Debug tab interactions */
            if (current_tab == TAB_DEBUG) {
                /* CLI input area */
                if (mx >= 60 && mx <= w - 140 && my >= 220 && my <= 240) {
                    cli_cmd_len = 0;
                    input_active_field = 0;
                    scraper_active_field = 0;
                }
                /* Run CLI button */
                if (mx >= w - 120 && mx <= w - 12 && my >= 212 && my <= 244) {
                    handle_debug_cli_run();
                }
                /* Helper templates */
                if (mx >= 60 && mx <= 160 && my >= 200 && my <= 216) {
                    strncpy(cli_cmd_buf, "help -all", sizeof(cli_cmd_buf)-1); cli_cmd_len = strlen(cli_cmd_buf);
                    append_message("system", "CLI template set: help -all");
                }
                if (mx >= 170 && mx <= 300 && my >= 200 && my <= 216) {
                    strncpy(cli_cmd_buf, "contract flush", sizeof(cli_cmd_buf)-1); cli_cmd_len = strlen(cli_cmd_buf);
                    append_message("system", "CLI template set: contract flush");
                }
                if (mx >= 310 && mx <= 420 && my >= 200 && my <= 216) {
                    strncpy(cli_cmd_buf, "vectors close", sizeof(cli_cmd_buf)-1); cli_cmd_len = strlen(cli_cmd_buf);
                    append_message("system", "CLI template set: vectors close");
                }
            }

            /* wait for release to avoid repeated clicks */
            while (glfwGetMouseButton(window, GLFW_MOUSE_BUTTON_LEFT) == GLFW_PRESS) glfwPollEvents();
        }

        /* Keyboard handling: Enter */
        if (glfwGetKey(window, GLFW_KEY_ENTER) == GLFW_PRESS) {
            if (cli_cmd_len > 0) {
                handle_debug_cli_run();
            } else if (current_tab == TAB_CHAT) {
                handle_chat_send();
            } else if (current_tab == TAB_TRAIN) {
                handle_train_send();
            } else if (current_tab == TAB_SCRAPER) {
                if (scraper_active_field == 1 || scraper_active_field == 2) handle_scraper_run(0);
            }
            while (glfwGetKey(window, GLFW_KEY_ENTER) == GLFW_PRESS) glfwPollEvents();
        }

        /* Backspace handling */
        if (glfwGetKey(window, GLFW_KEY_BACKSPACE) == GLFW_PRESS) {
            if (scraper_active_field == 1) {
                if (scraper_url_len > 0) { scraper_url_len--; scraper_url[scraper_url_len] = '\0'; }
            } else if (scraper_active_field == 2) {
                if (scraper_out_len > 0) { scraper_out_len--; scraper_out[scraper_out_len] = '\0'; }
            } else if (cli_cmd_len > 0) {
                cli_cmd_len--; cli_cmd_buf[cli_cmd_len] = '\0';
            } else if (input_len > 0) {
                input_len--; input_buf[input_len] = '\0';
            }
            while (glfwGetKey(window, GLFW_KEY_BACKSPACE) == GLFW_PRESS) glfwPollEvents();
        }

        /* Toggle debug overlay */
        if (glfwGetKey(window, GLFW_KEY_F1) == GLFW_PRESS) {
            debug_visible = !debug_visible;
            while (glfwGetKey(window, GLFW_KEY_F1) == GLFW_PRESS) glfwPollEvents();
        }

        /* Debug overlay rendering */
        if (debug_visible || current_tab == TAB_DEBUG) {
            glColor3f(0.05f, 0.05f, 0.08f);
            glBegin(GL_QUADS);
            glVertex2f(50, 50); glVertex2f(w - 50, 50);
            glVertex2f(w - 50, h - 50); glVertex2f(50, h - 50);
            glEnd();

            char bufset[256];
            snprintf(bufset, sizeof(bufset), " [ %c ] Online", API_SETTINGS.online ? 'X' : ' ');
            draw_text(60, 320, bufset);
            snprintf(bufset, sizeof(bufset), " Host: %s", API_SETTINGS.host); draw_text(60, 340, bufset);
            snprintf(bufset, sizeof(bufset), " Port: %d", API_SETTINGS.port); draw_text(60, 360, bufset);
            snprintf(bufset, sizeof(bufset), " User: %s", API_SETTINGS.username); draw_text(60, 380, bufset);
            draw_text(60, 400, " Password: ******");
            draw_text(300, 450, "[ Save API Settings ]");

            if (current_tab == TAB_TRAIN) {
                char tbuf[128];
                snprintf(tbuf, sizeof(tbuf), "top_k: %d", TRAIN_SETTINGS.top_k); draw_text(60, 140, tbuf);
                snprintf(tbuf, sizeof(tbuf), "max_nexts: %d", TRAIN_SETTINGS.max_nexts); draw_text(60, 160, tbuf);
            }

            if (current_tab == TAB_SCRAPER) {
                draw_text(60, 100, "URL:");
                glColor3f(0.12f, 0.12f, 0.12f);
                glBegin(GL_QUADS);
                glVertex2f(60, 120); glVertex2f(w - 60, 120);
                glVertex2f(w - 60, 140); glVertex2f(60, 140);
                glEnd();
                draw_text(62, 126, scraper_url[0] ? scraper_url : "(enter URL)");

                draw_text(60, 130, "Output file:");
                glBegin(GL_QUADS);
                glVertex2f(60, 150); glVertex2f(360, 150);
                glVertex2f(360, 170); glVertex2f(60, 170);
                glEnd();
                draw_text(62, 156, scraper_out[0] ? scraper_out : "(enter out.jsonl)");

                /* buttons */
                glColor3f(0.25f, 0.25f, 0.45f);
                glBegin(GL_QUADS);
                glVertex2f(60, 180); glVertex2f(200, 180);
                glVertex2f(200, 210); glVertex2f(60, 210);
                glEnd();
                draw_text(86, 192, "Scrape");

                glColor3f(0.15f, 0.45f, 0.15f);
                glBegin(GL_QUADS);
                glVertex2f(220, 180); glVertex2f(420, 180);
                glVertex2f(420, 210); glVertex2f(220, 210);
                glEnd();
                draw_text(286, 192, "Scrape & Train");
            }

            /* CLI helpers and input */
            draw_text(60, 208, "[ CLI Helpers ]");
            glColor3f(0.2f, 0.2f, 0.4f);
            glBegin(GL_QUADS);
            glVertex2f(60, 200); glVertex2f(160, 200);
            glVertex2f(160, 216); glVertex2f(60, 216);
            glEnd();
            draw_text(72, 204, "help -all");

            glBegin(GL_QUADS);
            glVertex2f(170, 200); glVertex2f(300, 200);
            glVertex2f(300, 216); glVertex2f(170, 216);
            glEnd();
            draw_text(182, 204, "contract flush");

            glBegin(GL_QUADS);
            glVertex2f(310, 200); glVertex2f(420, 200);
            glVertex2f(420, 216); glVertex2f(310, 216);
            glEnd();
            draw_text(322, 204, "vectors close");

            /* CLI input box */
            glColor3f(0.12f, 0.12f, 0.12f);
            glBegin(GL_QUADS);
            glVertex2f(60, 240); glVertex2f(w - 140, 240);
            glVertex2f(w - 140, 220); glVertex2f(60, 220);
            glEnd();
            draw_text(62, 226, cli_cmd_buf[0] ? cli_cmd_buf : "(enter CLI command here)");

            /* Run CLI button */
            glColor3f(0.15f, 0.45f, 0.15f);
            glBegin(GL_QUADS);
            glVertex2f(w - 120, 212); glVertex2f(w - 12, 212);
            glVertex2f(w - 12, 244); glVertex2f(w - 120, 244);
            glEnd();
            draw_text(w - 112, 220, "Run CLI");
        }

        glfwSwapBuffers(window);
    }

    return 0;
}

void mage_gui_shutdown(void) {
    if (window) { glfwDestroyWindow(window); window = NULL; }
    glfwTerminate();
    /* free messages */
    for (int i = 0; i < msg_count; ++i) { free(messages[i]); messages[i] = NULL; }
    msg_count = 0;
}

/* ---------- main ---------- */

int main(int argc, char **argv) {
    /* Ensure repo root as cwd when launched from bin/ */
    if (argc > 0 && argv[0]) {
        char realpath_buf[PATH_MAX];
        if (realpath(argv[0], realpath_buf)) {
            char *d = strdup(realpath_buf);
            char *exec_dir = dirname(d);
            char *d2 = strdup(exec_dir);
            char *repo_root = dirname(d2);
            if (repo_root) chdir(repo_root);
            free(d); free(d2);
        }
    }

    /* Verbose flag */
    for (int i = 1; i < argc; ++i) {
        if (strcmp(argv[i], "--verbose") == 0) {
            mage_log_init(MAGE_LOG_DEBUG, "data/full_verbose.log");
            mage_log_set_level(MAGE_LOG_DEBUG);
        }
    }

    /* Initialize API subsystems */
    (void)mage_api_init();

    if (mage_gui_init() != 0) {
        fprintf(stderr, "Failed to initialize Mage GUI (GLFW)\n");
        return 1;
    }

    int rc = mage_gui_run();

    mage_gui_shutdown();
    mage_api_shutdown();
    return rc == 0 ? 0 : 1;
}
