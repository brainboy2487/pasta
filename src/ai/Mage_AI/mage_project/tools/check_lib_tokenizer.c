#include <stdio.h>
#include <stdlib.h>
#include "mage/tokenizer.h"
int main(void) {
    const char *s = "Hello, world!";
    char *n = mage_normalize(s);
    if (!n) { printf("normalize returned NULL\n"); return 1; }
    printf("normalized: '%s'\n", n);
    char **tokens = NULL; size_t ncount = 0;
    int rc = mage_tokenize(s, &tokens, &ncount);
    printf("tokenize rc=%d n=%zu\n", rc, ncount);
    for (size_t i=0;i<ncount;i++) printf("token[%zu]=[%s]\n", i, tokens[i]);
    mage_free_tokens(tokens, ncount);
    free(n);
    return 0;
}
