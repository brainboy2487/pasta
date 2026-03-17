#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.."; pwd)"
ART="$ROOT/artifacts"
OUT="$ART/CHEWME.txt"
mkdir -p "$ART"
rm -f "$OUT"

timestamp() { date -u +"%Y-%m-%dT%H:%M:%SZ"; }

echo "CHEWME DIAGNOSTIC DUMP" > "$OUT"
echo "Generated: $(timestamp)" >> "$OUT"
echo "" >> "$OUT"

echo "SYSTEM" >> "$OUT"
echo "-------" >> "$OUT"
uname -a >> "$OUT" 2>&1 || true
rustc --version 2>&1 | sed -n '1,1p' >> "$OUT" || true
cargo --version 2>&1 | sed -n '1,1p' >> "$OUT" || true
echo "" >> "$OUT"

echo "COMMAND LINE (what you ran)" >> "$OUT"
echo "---------------------------" >> "$OUT"
echo 'cargo build --release && bash tools/install_pasta.sh && pasta tests/test_graphics.ps' >> "$OUT"
echo "" >> "$OUT"

echo "WORKSPACE FILES (list)" >> "$OUT"
echo "----------------------" >> "$OUT"
ls -la "$ROOT" | sed -n '1,200p' >> "$OUT" 2>&1 || true
echo "" >> "$OUT"

echo "TEST SCRIPTS (all .ps under tests/)" >> "$OUT"
echo "-----------------------------------" >> "$OUT"
for f in $(ls tests/*.ps 2>/dev/null || true); do
  echo "---- FILE: $f ----" >> "$OUT"
  nl -ba "$f" | sed -n '1,400p' >> "$OUT"
  echo "" >> "$OUT"
done

echo "FIRST-LOOK: lines around first 'window' occurrence in tests/test_graphics.ps" >> "$OUT"
echo "-----------------------------------------------------------------------" >> "$OUT"
if grep -q "window" tests/test_graphics.ps 2>/dev/null; then
  LNUM=$(grep -n "window" tests/test_graphics.ps | head -n1 | cut -d: -f1)
  START=$(( LNUM > 20 ? LNUM - 20 : 1 ))
  END=$(( LNUM + 20 ))
  sed -n "${START},${END}p" tests/test_graphics.ps | nl -ba -v${START} >> "$OUT"
else
  echo "no 'window' found in tests/test_graphics.ps" >> "$OUT"
fi
echo "" >> "$OUT"

echo "SEARCH: all occurrences of 'window' and 'gfx' in repo" >> "$OUT"
echo "----------------------------------------------------" >> "$OUT"
grep -R --line-number --no-color -E "window|gfx|gfx_windows|gfx_canvases|window_set_pixel|window_save|window_fill" src tests || true
echo "" >> "$OUT"

echo "INTERPRETER: executor.rs top (first 220 lines)" >> "$OUT"
echo "-----------------------------------------------" >> "$OUT"
if [ -f src/interpreter/executor.rs ]; then
  sed -n '1,220p' src/interpreter/executor.rs | nl -ba >> "$OUT"
else
  echo "src/interpreter/executor.rs not found" >> "$OUT"
fi
echo "" >> "$OUT"

echo "INTERPRETER: find call_builtin location and dump surrounding region" >> "$OUT"
echo "-----------------------------------------------------------------" >> "$OUT"
if [ -f src/interpreter/executor.rs ]; then
  CB_LINE=$(grep -n "pub fn call_builtin" -n src/interpreter/executor.rs | head -n1 | cut -d: -f1 || true)
  if [ -n "$CB_LINE" ]; then
    START=$(( CB_LINE > 40 ? CB_LINE - 40 : 1 ))
    END=$(( CB_LINE + 240 ))
    echo "call_builtin at line: $CB_LINE; dumping $START..$END" >> "$OUT"
    sed -n "${START},${END}p" src/interpreter/executor.rs | nl -ba -v${START} >> "$OUT"
  else
    echo "call_builtin not found by grep; dumping 400..760 for manual inspection" >> "$OUT"
    sed -n '400,760p' src/interpreter/executor.rs | nl -ba >> "$OUT"
  fi
else
  echo "executor.rs missing" >> "$OUT"
fi
echo "" >> "$OUT"

echo "INTERPRETER: region around 'match name {' dispatch" >> "$OUT"
echo "----------------------------------------------------" >> "$OUT"
if [ -f src/interpreter/executor.rs ]; then
  # find match name occurrences
  grep -n "match .*name" src/interpreter/executor.rs || true
  # dump a few candidate regions
  for L in $(grep -n "match .*name" src/interpreter/executor.rs | cut -d: -f1 || true); do
    START=$(( L > 20 ? L - 20 : 1 ))
    END=$(( L + 120 ))
    echo "---- region around line $L ----" >> "$OUT"
    sed -n "${START},${END}p" src/interpreter/executor.rs | nl -ba -v${START} >> "$OUT"
    echo "" >> "$OUT"
  done
fi

echo "PARSER / LEXER: key files" >> "$OUT"
echo "-------------------------" >> "$OUT"
for f in src/parser/*.rs src/lexer/*.rs src/interpreter/*.rs src/interpreter/*.rs; do
  [ -f "$f" ] || continue
  echo "---- FILE: $f (first 200 lines) ----" >> "$OUT"
  sed -n '1,200p' "$f" | nl -ba >> "$OUT"
  echo "" >> "$OUT"
done

echo "ENVIRONMENT & API: int_api.rs, environment.rs, ex_eval.rs (first 200 lines each)" >> "$OUT"
echo "-------------------------------------------------------------------------------" >> "$OUT"
for f in src/interpreter/int_api.rs src/interpreter/environment.rs src/interpreter/ex_eval.rs; do
  [ -f "$f" ] || continue
  echo "---- FILE: $f ----" >> "$OUT"
  sed -n '1,200p' "$f" | nl -ba >> "$OUT"
  echo "" >> "$OUT"
done

echo "BACKUPS: any *.bak_* files near executor.rs" >> "$OUT"
echo "-------------------------------------------" >> "$OUT"
ls -la src/interpreter | sed -n '1,200p' >> "$OUT" 2>&1 || true
ls -1 src/interpreter/executor.rs.bak_* 2>/dev/null | sed -n '1,200p' >> "$OUT" || true
echo "" >> "$OUT"

echo "PATCH MARKERS: search for our inserted marker strings" >> "$OUT"
echo "----------------------------------------------------" >> "$OUT"
grep -R --line-number --no-color -E "Headless graphics builtins|window_set_pixel|window_save|window_fill|win://" src || true
echo "" >> "$OUT"

echo "ARTIFACTS LOGS" >> "$OUT"
echo "-------------" >> "$OUT"
if [ -f artifacts/build_and_run.log ]; then
  echo "---- artifacts/build_and_run.log (first 400 lines) ----" >> "$OUT"
  sed -n '1,400p' artifacts/build_and_run.log >> "$OUT"
fi
if [ -f artifacts/EATME.txt ]; then
  echo "---- artifacts/EATME.txt (first 400 lines) ----" >> "$OUT"
  sed -n '1,400p' artifacts/EATME.txt >> "$OUT"
fi
if [ -f artifacts/test_graphics_debug.log ]; then
  echo "---- artifacts/test_graphics_debug.log (first 400 lines) ----" >> "$OUT"
  sed -n '1,400p' artifacts/test_graphics_debug.log >> "$OUT"
fi
echo "" >> "$OUT"

echo "RECENT GIT STATUS (if repo is a git repo)" >> "$OUT"
echo "-----------------------------------------" >> "$OUT"
if [ -d .git ]; then
  git status --porcelain -b | sed -n '1,200p' >> "$OUT" 2>&1 || true
  echo "" >> "$OUT"
  echo "Last 200 lines of git diff (if any):" >> "$OUT"
  git --no-pager diff -- src/interpreter/executor.rs | sed -n '1,400p' >> "$OUT" 2>&1 || true
else
  echo "No .git directory found; skipping git info" >> "$OUT"
fi
echo "" >> "$OUT"

echo "MINIMAL GFX SMOKE SCRIPT (created for quick repro)" >> "$OUT"
echo "-------------------------------------------------" >> "$OUT"
cat > tests/_gfx_smoke.ps <<'PS'
h = window("smoke", 32, 24)
window_fill(h, 0, 0, 32, 24, 128, 64, 32)
window_set_pixel(h, 1, 1, 255, 0, 0)
window_save(h, "artifacts/smoke.ppm")
PRINT("DONE")
PS
nl -ba tests/_gfx_smoke.ps | sed -n '1,200p' >> "$OUT"
echo "" >> "$OUT"

echo "HOW TO REPRODUCE (commands)" >> "$OUT"
echo "---------------------------" >> "$OUT"
cat >> "$OUT" <<'CMD'
# build debug and run minimal smoke
cargo build
./target/debug/pasta tests/_gfx_smoke.ps 2>&1 | tee artifacts/gfx_smoke_debug.log

# or run your full chain and capture logs
bash tools/REBUILD_CHAIN_TOOL.sh
CMD

echo "" >> "$OUT"
echo "END OF CHEWME" >> "$OUT"
echo "Wrote $OUT"
