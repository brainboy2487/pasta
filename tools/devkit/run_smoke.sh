#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.."; pwd)"
ART="$ROOT/artifacts"
mkdir -p "$ART"

echo "[devkit] Building release..."
if ! cargo build --release 2>&1 | tee "$ART/build.log"; then
  cp "$ART/build.log" "$ART/EATME_build.txt"
  exit 1
fi

echo "[devkit] Running smoke scripts..."
for s in "$ROOT"/tests/*.ps; do
  echo "[devkit] Running $s"
  if ! "$ROOT/target/release/pasta" "$s" 2>&1 | tee "$ART/$(basename "$s").log"; then
    cp "$ART/$(basename "$s").log" "$ART/EATME_$(basename "$s").txt"
    exit 2
  fi
done

echo "[devkit] All smoke tests passed."
