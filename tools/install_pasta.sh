#!/usr/bin/env bash
set -euo pipefail

SRC="target/release/pasta"
DEST="/usr/bin/pasta"

echo "[install] Checking for $SRC ..."
if [[ ! -f "$SRC" ]]; then
    echo "[install] ERROR: $SRC not found. Run 'cargo build --release' first."
    exit 1
fi

echo "[install] Copying $SRC -> $DEST"
sudo cp "$SRC" "$DEST"

echo "[install] Setting executable permissions"
sudo chmod 755 "$DEST"

echo "[install] Done. System pasta updated."
