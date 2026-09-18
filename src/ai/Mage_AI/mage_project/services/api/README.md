MAGE API scaffold
=================

This directory contains a minimal OpenAPI spec to begin implementing the HTTP API for MAGE.

Endpoints included:
- POST /vectorize
- POST /retrieve
- POST /generate
- POST /tap/parse
- GET  /admin/indices

Next steps:
- Implement a small HTTP server (Flask/FastAPI or C-based) to serve these routes.
- Wire `python/mage/ffi.py` or call `libmage` via subprocess for compute paths.
- Add contract tests mirroring `stack_layered.txt` smoke tests.
