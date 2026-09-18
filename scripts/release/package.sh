#!/usr/bin/env bash
set -euo pipefail

mkdir -p dist
cp target/release/pasta dist/

if [[ "${RUNNER_OS:-}" == "macOS" || "$(uname -s)" == "Darwin" ]]; then
  archive="pasta-macos-x64.tar.gz"
else
  archive="pasta-linux-x64.tar.gz"
fi

tar -C dist -czf "dist/${archive}" pasta

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "dist/${archive}" > "dist/${archive%.tar.gz}.sha256"
else
  shasum -a 256 "dist/${archive}" > "dist/${archive%.tar.gz}.sha256"
fi

cargo_version="$(awk -F '"' '/^version[[:space:]]*=/ { print $2; exit }' Cargo.toml)"
created_utc="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"
os_name="linux"
if [[ "${RUNNER_OS:-}" == "macOS" || "$(uname -s)" == "Darwin" ]]; then
  os_name="macos"
fi
cat > dist/release-manifest.txt <<EOF
os=${os_name}
artifact=${archive}
checksum_file=${archive%.tar.gz}.sha256
cargo_version=${cargo_version}
created_utc=${created_utc}
EOF

echo "RELEASE_PACKAGE_OK"
