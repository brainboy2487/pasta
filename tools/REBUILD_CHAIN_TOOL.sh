#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.."; pwd)"
ART="$ROOT/artifacts"
LOG="$ART/build_and_run.log"
EATME="$ART/EATME.txt"

mkdir -p "$ART"
rm -f "$LOG" "$EATME"

timestamp() { date -u +"%Y-%m-%dT%H:%M:%SZ"; }

echo "COMMAND: cargo build --release && bash tools/install_pasta.sh && pasta tests/test_graphics.ps" > "$EATME"
echo "START: $(timestamp)" >> "$EATME"
echo "" >> "$EATME"
echo "---- RAW LOG (full) ----" >> "$EATME"

# Run build, install, run; capture exit codes and append to raw log
(
  echo "=== BUILD START: $(timestamp) ==="
  if cargo build --release 2>&1 | tee -a "$LOG"; then
    echo "=== BUILD EXIT: 0 ===" | tee -a "$LOG"
  else
    echo "=== BUILD EXIT: 1 ===" | tee -a "$LOG"
  fi

  echo "=== INSTALL START: $(timestamp) ===" | tee -a "$LOG"
  if bash tools/install_pasta.sh 2>&1 | tee -a "$LOG"; then
    echo "=== INSTALL EXIT: 0 ===" | tee -a "$LOG"
  else
    echo "=== INSTALL EXIT: 1 ===" | tee -a "$LOG"
  fi

  echo "=== RUN START: $(timestamp) ===" | tee -a "$LOG"
  if pasta tests/test_graphics.ps 2>&1 | tee -a "$LOG"; then
    echo "=== RUN EXIT: 0 ===" | tee -a "$LOG"
  else
    echo "=== RUN EXIT: 1 ===" | tee -a "$LOG"
  fi
) 2>&1 | tee -a "$LOG"

# Append raw log to EATME
echo "" >> "$EATME"
cat "$LOG" >> "$EATME"
echo "" >> "$EATME"
echo "---- END RAW LOG ----" >> "$EATME"
echo "" >> "$EATME"

# Summary section for AI consumption
{
  echo "SUMMARY"
  echo "Start: $(timestamp)"
  echo ""
  echo "Exit codes (last occurrences):"
  grep -E "=== (BUILD|INSTALL|RUN) EXIT:[0-9]+" "$LOG" | tail -n 10 || true
  echo ""
  echo "First compiler error block (if any):"
  awk '
    BEGIN{found=0}
    /error:/{ if(!found){ found=1; print; for(i=0;i<40;i++){ if(getline){ print } else { exit } } } }
  ' "$LOG" | sed -n '1,200p' || true
  echo ""
  echo "Last runtime error lines (if any):"
  # show last 200 lines and extract last 'Error:' occurrences
  tail -n 200 "$LOG" | sed -n '/Error:/,$p' | sed -n '1,200p' || true
  echo ""
  echo "Last 200 lines of log:"
  tail -n 200 "$LOG" || true
  echo ""
  echo "Warnings count (approx):"
  grep -c "warning:" "$LOG" || true
  echo ""
  echo "END: $(timestamp)"
} >> "$EATME"

# Print short console summary
echo "Wrote $EATME (full raw log: $LOG)."
