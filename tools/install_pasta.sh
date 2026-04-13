#!/usr/bin/env bash
set -euo pipefail

SRC="target/release/pasta"
# Install to /usr/local/bin so it takes priority over any older /usr/bin/pasta.
DEST="/usr/local/bin/pasta"

echo "[install] Building release binary ..."
cargo build --release -q

echo "[install] Copying $SRC -> $DEST"
sudo cp "$SRC" "$DEST"

echo "[install] Setting executable permissions"
sudo chmod 755 "$DEST"

echo "[install] Done. System pasta updated."
