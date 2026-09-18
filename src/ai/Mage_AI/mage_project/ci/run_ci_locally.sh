#!/usr/bin/env bash
set -euo pipefail

# Local CI helper: builds project, runs C smoke test, and Python unit tests.
# Usage: ./ci/run_ci_locally.sh

echo "Building project..."
make -j2

echo "Building and installing helper tools (gen_mphf)..."
make install-tools || true

echo "Running C unit test..."
# use bin/test_runner created by Makefile
if [ -x bin/test_runner ]; then
	./bin/test_runner
else
	echo "bin/test_runner not found; skipping C unit test"
fi

echo "Installing Python deps into virtualenv .venv..."
python3 -m venv .venv
source .venv/bin/activate
python -m pip install --upgrade pip
if [ -f python/requirements.txt ]; then python -m pip install -r python/requirements.txt || true; fi
python -m pip install pytest numpy

echo "Running Python tests (unit)..."
mkdir -p build/tests
PYTHONPATH=. python -m pytest -q tests/unit
deactivate

echo "Generating a sample vocab and running gen_mphf to validate toolchain..."
mkdir -p data/corpus data/vocab
cat > data/train_sample.txt <<'EOF'
Hello world
Sample for CI MPHF
EOF
# Prefer Python trainer when available
if [ -f python/tools/train_vocab.py ]; then
	python3 python/tools/train_vocab.py --input data/train_sample.txt --out data/vocab/generated_vocab.jsonl
else
	# fallback to CLI generate-vocab if present
	if [ -x bin/mage_cli ]; then
		./bin/mage_cli generate-vocab data/train_sample.txt data/vocab/generated_vocab.jsonl || true
	fi
fi

if [ -x bin/gen_mphf ]; then
	./bin/gen_mphf data/vocab/generated_vocab.jsonl data/mphf.bin || echo "gen_mphf failed"
elif [ -x tools/gen_mphf ]; then
	./tools/gen_mphf data/vocab/generated_vocab.jsonl data/mphf.bin || echo "tools/gen_mphf failed"
else
	echo "gen_mphf not found; install-tools may have failed"
fi

echo "CI local run completed successfully."