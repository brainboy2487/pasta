#ifndef MAGE_SUBPROC_H
#define MAGE_SUBPROC_H

#ifdef __cplusplus
extern "C" {
#endif

/* Run a shell command (via /bin/sh -c) and wait up to timeout_ms milliseconds.
 * Returns the child's exit status (as returned by waitpid) or -1 on error.
 */
int subproc_run_shell(const char *cmd, unsigned int timeout_ms);

/* Run a program with argv (NULL-terminated) and provide `stdin_data` as its stdin.
 * Captures stdout into a newly-allocated string placed in *out (caller must free).
 * If out is NULL, stdout is discarded. Returns 0 on success and places child's
 * exit code into *exit_code if provided. Returns -1 on internal error.
 */
int subproc_run_capture_stdin(const char *const argv[], const char *stdin_data, unsigned int timeout_ms, char **out, int *exit_code);

#ifdef __cplusplus
}
#endif

#endif /* MAGE_SUBPROC_H */
