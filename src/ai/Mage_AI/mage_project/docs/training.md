# Training & Scraping — Quick Runbook

This document shows the minimal commands to run scraping, generate a vocab, vectorize, and train the CLI locally.

Prerequisites
- Build the C binaries: `make all` or `make -j2 all`.
- Activate Python venv with dependencies installed (project venv used in CI):

```bash
source .venv/bin/activate
pip install -r python/requirements.txt  # if needed
```

Quick local workflow (preferred: Python pipeline)

1) Scrape a local file (or URL) into JSONL

```bash
# read a local text or JSONL file and write normalized JSONL
python3 python/tools/scraper.py --local path/to/input.txt --out data/corpus/scraped.jsonl
```

2) Run the training pipeline (Python-first)

```bash
# produces data/vocab/generated_vocab.jsonl (and optional artifacts)
python3 python/tools/train_pipeline.py --input data/corpus/scraped.jsonl --vocab-out data/vocab/generated_vocab.jsonl --py-vectorize --snapshot
```

Notes:
- `--run-mphf` will run `bin/gen_mphf` (or `tools/gen_mphf`) if present and produce `data/mphf.bin`.
- `--vectorize` invokes `mage_cli vectorize` by default; use `--py-vectorize` to use the Python fallback.

3) Query the CLI before/after training

```bash
# baseline
./bin/mage_cli foobaz
# after training the same input
./bin/mage_cli foobaz
```

`mage_cli train <file>` behavior
- `mage_cli train <file>` prefers the Python pipeline `python/tools/train_pipeline.py` if present. This avoids using the older in-process C trainer which may crash in some environments.

Scraper in-process training
- `python/tools/scraper.py` will by default call the Python training pipeline for in-memory entries (it writes a temp JSONL and runs the pipeline). If the Python pipeline is missing, it falls back to `./bin/mage_cli train`.

Vectorization
- To create `data/vectors/vectors.bin` and its manifest, either:

```bash
# CLI vectorizer
./bin/mage_cli vectorize --input data/corpus/scraped.jsonl --output data/vectors

# or Python fallback
python3 python/tools/train_pipeline.py --input data/corpus/scraped.jsonl --vectorize --py-vectorize
```

Run tests and full integration locally

```bash
# unit tests (C) and Python unit tests
make test
pytest tests/unit -q

# full integration (runs the scenarios in tests/integration/full_integration.py)
make integration-full
```

CI helper (local CI emulation)

```bash
# runs the local CI helper which builds tools, sets up venv, and runs tests
./ci/run_ci_locally.sh
```

Troubleshooting & FAQs
- If `mage_cli train` segfaults: the repo now prefers the Python pipeline; run `python3 python/tools/train_pipeline.py` directly. If you need the native trainer fixed, run `make debug` and use `gdb` to capture a backtrace for `./bin/mage_cli train <file>`.
- If `mage_cli` returns a fallback message after training: ensure `data/vocab/generated_vocab.jsonl` contains the expected tokens and that `bin/mage_cli` was restarted (CLI loads vocab at startup). Re-run `./bin/mage_cli train <file>` or restart the process.
- FFI/ctypes crashes observed earlier were due to pointer handling; the Python FFI wrapper (`python/mage/ffi.py`) has been updated to treat C-returned JSON pointers as raw pointers and free them safely.

Contact / Next steps
- If you want, I can:
  - Add a `make train-pipeline` target to the `Makefile` that wraps the Python pipeline.
  - Add a short `docs/training-advanced.md` describing MPHF generation and vector formats.

End of runbook.
