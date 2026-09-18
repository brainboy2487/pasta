/* Lightweight supervised subprocess helpers
 * - subproc_run_shell: runs command via /bin/sh -c with timeout
 * - subproc_run_capture_stdin: runs argv[] program, writes stdin_data to child stdin,
 *   captures stdout into an allocated buffer, returns exit code.
 */

#define _POSIX_C_SOURCE 200809L
#include "mage/subproc.h"
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <sys/select.h>
#include <fcntl.h>
#include <signal.h>
#include <errno.h>
#include <stdio.h>
#include <time.h>

static int set_nonblocking(int fd) {
    int flags = fcntl(fd, F_GETFL, 0);
    if (flags == -1) return -1;
    return fcntl(fd, F_SETFL, flags | O_NONBLOCK);
}

int subproc_run_shell(const char *cmd, unsigned int timeout_ms) {
    if (!cmd) return -1;
    pid_t pid = fork();
    if (pid < 0) return -1;
    if (pid == 0) {
        /* child */
        execl("/bin/sh", "sh", "-c", cmd, (char*)NULL);
        _exit(127);
    }
    /* parent */
    unsigned int waited = 0;
    const unsigned int step = 50; /* ms */
    int status = 0;
    while (1) {
        pid_t r = waitpid(pid, &status, WNOHANG);
        if (r == pid) return status;
        if (r == -1) return -1;
        if (timeout_ms != 0 && waited >= timeout_ms) {
            kill(pid, SIGKILL);
            waitpid(pid, &status, 0);
            return status;
        }
        struct timespec ts = { .tv_sec = step / 1000, .tv_nsec = (step % 1000) * 1000000 }; (void)nanosleep(&ts, NULL);
        waited += step;
    }
}

int subproc_run_capture_stdin(const char *const argv[], const char *stdin_data, unsigned int timeout_ms, char **out, int *exit_code) {
    if (!argv || !argv[0]) return -1;
    int inpipe[2] = {-1,-1};
    int outpipe[2] = {-1,-1};
    if (pipe(inpipe) != 0) return -1;
    if (pipe(outpipe) != 0) { close(inpipe[0]); close(inpipe[1]); return -1; }

    pid_t pid = fork();
    if (pid < 0) {
        close(inpipe[0]); close(inpipe[1]); close(outpipe[0]); close(outpipe[1]);
        return -1;
    }
    if (pid == 0) {
        /* child: connect pipes */
        dup2(inpipe[0], STDIN_FILENO);
        dup2(outpipe[1], STDOUT_FILENO);
        /* close unused */
        close(inpipe[0]); close(inpipe[1]); close(outpipe[0]); close(outpipe[1]);
        execvp(argv[0], (char * const *)argv);
        _exit(127);
    }

    /* parent */
    close(inpipe[0]); close(outpipe[1]);
    /* write stdin_data (if any) and close write end */
    if (stdin_data && stdin_data[0]) {
        size_t rem = strlen(stdin_data);
        const char *p = stdin_data;
        while (rem > 0) {
            ssize_t w = write(inpipe[1], p, rem);
            if (w < 0) {
                if (errno == EINTR) continue;
                break;
            }
            rem -= (size_t)w; p += w;
        }
    }
    close(inpipe[1]);

    /* set non-blocking to read with timeout */
    set_nonblocking(outpipe[0]);

    size_t cap = 4096; size_t len = 0; char *buf = malloc(cap);
    if (!buf) { close(outpipe[0]); kill(pid, SIGKILL); waitpid(pid, NULL, 0); return -1; }
    unsigned int waited = 0; const unsigned int step = 50;
    int child_exited = 0; int status = 0;
    while (1) {
        /* read available data */
        char tmp[1024]; ssize_t r = read(outpipe[0], tmp, sizeof(tmp));
        if (r > 0) {
            if (len + (size_t)r + 1 > cap) {
                size_t nc = cap * 2 + (size_t)r + 1;
                char *n = realloc(buf, nc);
                if (!n) { free(buf); close(outpipe[0]); kill(pid, SIGKILL); waitpid(pid, NULL, 0); return -1; }
                buf = n; cap = nc;
            }
            memcpy(buf + len, tmp, (size_t)r); len += (size_t)r; buf[len] = '\0';
            continue; /* attempt to read more without waiting */
        }
        if (r == 0) {
            /* EOF */
            break;
        }
        if (r < 0 && errno != EAGAIN && errno != EWOULDBLOCK && errno != EINTR) {
            /* read error */
            break;
        }

        /* check child status */
        pid_t w = waitpid(pid, &status, WNOHANG);
        if (w == pid) { child_exited = 1; }

        if (child_exited) {
            /* try final read once more and then break */
            continue;
        }

        if (timeout_ms != 0 && waited >= timeout_ms) {
            kill(pid, SIGKILL);
            waitpid(pid, &status, 0);
            break;
        }
        struct timespec ts = { .tv_sec = step / 1000, .tv_nsec = (step % 1000) * 1000000 };
        (void)nanosleep(&ts, NULL);
        waited += step;
    }

    close(outpipe[0]);
    if (waitpid(pid, &status, 0) == -1) status = -1;
    if (out) {
        *out = buf;
    } else {
        free(buf);
    }
    if (exit_code) *exit_code = (status == -1) ? -1 : status;
    return 0;
}
