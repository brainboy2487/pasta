#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "mage/tokenizer.h"
#include <sys/types.h>

int main(void) {
    const char *path = "tests/golden/tokenizer_golden.txt";
    FILE *f = fopen(path, "r");
    if (!f) { perror(path); return 2; }
    char *orig = NULL; size_t ol = 0;
    char *gold = NULL; size_t gl = 0;
    ssize_t r1, r2, r3;
    int failures = 0;
    while (1) {
        r1 = getline(&orig, &ol, f);
        if (r1 <= 0) break;
        r2 = getline(&gold, &gl, f);
        if (r2 <= 0) break;
        /* consume optional blank line */
        char *blank = NULL; size_t bl = 0; r3 = getline(&blank, &bl, f);
        (void)r3;
        if (blank) free(blank);

        /* strip trailing newlines */
        while (r1 > 0 && (orig[r1-1] == '\n' || orig[r1-1] == '\r')) orig[--r1] = '\0';
        while (r2 > 0 && (gold[r2-1] == '\n' || gold[r2-1] == '\r')) gold[--r2] = '\0';

        char *n = mage_normalize(orig);
        if (!n) { printf("normalize returned NULL for '%s'\n", orig); failures++; continue; }
        if (strcmp(n, gold) != 0) {
            printf("FAILED:\n  input: '%s'\n  got:   '%s'\n  want:  '%s'\n", orig, n, gold);
            failures++;
        } else {
            printf("PASS: '%s' -> '%s'\n", orig, n);
        }
        free(n);
    }
    free(orig); free(gold);
    fclose(f);
    return failures == 0 ? 0 : 2;
}
