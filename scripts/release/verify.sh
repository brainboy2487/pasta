#!/usr/bin/env bash
set -euo pipefail

echo "==> Verifying release artifact checksum (Unix)"

if [[ "${RUNNER_OS:-}" == "macOS" || "$(uname -s)" == "Darwin" ]]; then
  archive="dist/pasta-macos-x64.tar.gz"
  checksum="dist/pasta-macos-x64.sha256"
else
  archive="dist/pasta-linux-x64.tar.gz"
  checksum="dist/pasta-linux-x64.sha256"
fi

[[ -f "$archive" ]] || { echo "missing archive file: $archive" >&2; exit 1; }
[[ -f "$checksum" ]] || { echo "missing checksum file: $checksum" >&2; exit 1; }
[[ -f "dist/release-manifest.txt" ]] || { echo "missing release manifest: dist/release-manifest.txt" >&2; exit 1; }

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum -c "$checksum"
else
  expected="$(awk '{print $1}' "$checksum")"
  actual="$(shasum -a 256 "$archive" | awk '{print $1}')"
  [[ "$expected" == "$actual" ]] || { echo "checksum mismatch: expected $expected got $actual" >&2; exit 1; }
fi

echo "RELEASE_VERIFY_OK"
