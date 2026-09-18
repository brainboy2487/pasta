# Mage C API — Quick Reference

This document summarizes the stable C API exposed by `include/mage/api.h` and gives a minimal usage example.

Overview
- Call `mage_api_init()` once at process startup and `mage_api_shutdown()` at exit.
- Use the `mage_api_*` prefixed functions for safe access to subsystems: tokenization, training, vectorization, retrieval, mmapped index IO, and manifests.
- Functions that return allocated memory return freshly-allocated pointers; free them with the documented free or close helpers.

Common usage patterns

1) Initialize and generate a reply:

```c
#include <stdio.h>
#include "mage/api.h"

int main(void) {
    if (mage_api_init() != 0) {
        fprintf(stderr, "failed to init\n");
        return 1;
    }
    char* out = mage_api_generate("hello world");
    if (out) {
        printf("generated: %s\n", out);
        free(out);
    }
    mage_api_shutdown();
    return 0;
}
```

2) Train from a file (preferred Python pipeline available in repo):
- From shell: `./bin/mage_cli train data/corpus/scraped.jsonl` (the CLI prefers the Python pipeline when available).
- Programmatic: `mage_api_train_from_file("data/corpus/scraped.jsonl");`

3) Vectorize text and retrieve nearest neighbors (C-level):

```c
float buf[128];
if (mage_api_vectorize_text("example", buf, 128) == 0) {
    char* res = mage_api_retrieve(buf, 128, 5);
    if (res) {
        printf("retrieve json: %s\n", res);
        free(res);
    }
}
```

Notes & Troubleshooting
- See `docs/training.md` for end-to-end training and scraping instructions.
- The repo provides Python FFI wrappers in `python/mage/ffi.py` which call into this C API via `ctypes`.
- If you modify API headers, keep backward-compatible symbols in `include/mage/api.h` and document semantic changes here.

Want more?
- If you want, I can add a `examples/` entry to the `Makefile` to build the example automatically or expand this document with typedefs and full return-code tables.
