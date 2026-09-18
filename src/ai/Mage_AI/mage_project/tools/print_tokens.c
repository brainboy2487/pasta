#include <stdio.h>
#include <stdlib.h>
#include "mage/tokenizer.h"
int main(void) {
    const char *s = "Hello, world!";
    char **tokens = NULL; size_t n = 0;
    int rc = mage_tokenize(s, &tokens, &n);
    printf("rc=%d n=%zu\n", rc, n);
    for (size_t i=0;i<n;i++) printf("token[%zu]=[%s]\n", i, tokens[i]);
    mage_free_tokens(tokens, n);
    return 0;
}
