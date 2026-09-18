#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "mage/tokenizer.h"

int main(void) {
    const char *path = "tests/golden/tokenizer_tokens.txt";
    FILE *f = fopen(path, "r");
    if (!f) { perror(path); return 2; }
    char *orig = NULL; size_t ol = 0;
    char *tokline = NULL; size_t tl = 0;
    int failures = 0;
    while (1) {
        ssize_t r1 = getline(&orig, &ol, f);
        if (r1 <= 0) break;
        ssize_t r2 = getline(&tokline, &tl, f);
        if (r2 <= 0) break;
        /* consume blank */
        char *blank = NULL; size_t bl = 0; getline(&blank, &bl, f); if (blank) free(blank);

        /* strip newlines */
        while (r1>0 && (orig[r1-1]=='\n' || orig[r1-1]=='\r')) orig[--r1]='\0';
        while (r2>0 && (tokline[r2-1]=='\n' || tokline[r2-1]=='\r')) tokline[--r2]='\0';

        /* expected tokens split by space (may be empty) */
        char *exp_copy = strdup(tokline);
        size_t exp_n = 0; char **exp = NULL;
        char *p = strtok(exp_copy, " ");
        while (p) { exp = realloc(exp, (exp_n+1)*sizeof(char*)); exp[exp_n++] = strdup(p); p = strtok(NULL, " "); }

        /* run mage_tokenize on original (it normalizes internally) */
        char **got = NULL; size_t got_n = 0;
        int rc = mage_tokenize(orig, &got, &got_n);
        if (rc != 0) { printf("tokenize failed rc=%d for '%s'\n", rc, orig); failures++; }
        else {
            int ok = 1;
            if (got_n != exp_n) ok = 0;
            else {
                for (size_t i=0;i<exp_n;i++) if (strcmp(got[i], exp[i])!=0) { ok = 0; break; }
            }
            if (!ok) {
                printf("MISMATCH for '%s'\n  got(%zu):", orig, got_n);
                for (size_t i=0;i<got_n;i++) printf(" [%s]", got[i]);
                printf("\n  want(%zu):", exp_n);
                for (size_t i=0;i<exp_n;i++) printf(" [%s]", exp[i]);
                printf("\n");
                failures++;
            }
        }
        mage_free_tokens(got, got_n);
        for (size_t i=0;i<exp_n;i++) free(exp[i]); free(exp); free(exp_copy);
    }
    free(orig); free(tokline); fclose(f);
    return failures==0?0:2;
}
