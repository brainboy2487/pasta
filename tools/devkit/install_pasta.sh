#!/usr/bin/env bash
set -euo pipefail
SRC="target/release/pasta"
DEST="/usr/local/bin/pasta"
if [[ ! -f "$SRC" ]]; then
  echo "ERROR: $SRC not found. Build first."
  exit 1
fi
if [[ -f "$DEST" ]]; then
  sudo cp "$DEST" "${DEST}.bak_$(date -u +%Y%m%d_%H%M%S)"
fi
sudo cp "$SRC" "$DEST"
sudo chmod 755 "$DEST"
echo "Installed $DEST"
