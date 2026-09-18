#include <stdio.h>
#include "mage/tokenizer.h"
#include <stdlib.h>
int main(void) {
    const char *s = "Hello, world!";
    char *n = mage_normalize(s);
    if (!n) { printf("NULL\n"); return 1; }
    printf("norm: [%s]\n", n);
    free(n);
    return 0;
}
