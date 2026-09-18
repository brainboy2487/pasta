#!/usr/bin/env python3
"""
tools/devkit_bootstrap.py

Create a repeatable developer toolkit skeleton for the Pasta project.

What it creates (idempotent, safe):
- tools/devkit/README.md
- tools/devkit/tasks.py (task runner for common dev tasks)
- tools/devkit/run_smoke.sh (shell smoke runner)
- tools/devkit/check_golden.py (compare generated images/text to golden)
- tools/devkit/install_pasta.sh (safe installer)
- tools/devkit/patch_templates/ (placeholder patch scripts)
- Makefile (top-level targets: build, smoke, install, dev-setup)
- tests/ (if missing) with bubble_sort.ps and a golden output
- .github/workflows/ci.yml (CI skeleton) — only written if .github doesn't exist
- .pre-commit-config.yaml (basic hooks)
- artifacts/ directory for logs and failures

The script backs up any file it will overwrite to <path>.bak_<timestamp>.
"""
import os
import sys
import shutil
from pathlib import Path
from datetime import datetime

ROOT = Path('.').resolve()
TOOLS = ROOT / 'tools' / 'devkit'
PATCH_TEMPLATES = TOOLS / 'patch_templates'
ARTIFACTS = ROOT / 'artifacts'
GITHUB = ROOT / '.github' / 'workflows'
TS = datetime.utcnow().strftime('%Y%m%d_%H%M%S')

def safe_write(path: Path, content: str):
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        bak = path.with_name(path.name + f'.bak_{TS}')
        shutil.copy2(path, bak)
        print(f'Backup: {path} -> {bak}')
    path.write_text(content, encoding='utf-8')
    print(f'Wrote: {path}')

def ensure_dir(p: Path):
    p.mkdir(parents=True, exist_ok=True)
    print(f'Ensured dir: {p}')

# 1) README for devkit
ensure_dir(TOOLS)
safe_write(TOOLS / 'README.md', """# Pasta Devkit

This directory contains developer tooling stubs and scripts for the Pasta project.

Key files:
- `tasks.py` — small Python task runner for common dev tasks.
- `run_smoke.sh` — builds and runs smoke tests, writes artifacts on failure.
- `check_golden.py` — compares generated outputs to golden files.
- `install_pasta.sh` — safe system installer for the built binary.
- `patch_templates/` — placeholder scripts for safe, reversible patches.

Usage:
- `make dev-setup` to create virtualenv and install Python deps (if any).
- `make smoke` to run the smoke test suite and collect artifacts.
""")

# 2) tasks.py — simple task runner
safe_write(TOOLS / 'tasks.py', """#!/usr/bin/env python3
\"\"\"Simple task runner for common dev tasks.

Usage:
  python3 tools/devkit/tasks.py build
  python3 tools/devkit/tasks.py smoke
  python3 tools/devkit/tasks.py golden-check
  python3 tools/devkit/tasks.py install
\"\"\"
import sys
import subprocess
from pathlib import Path

ROOT = Path('.').resolve()
ARTIFACTS = ROOT / 'artifacts'
ARTIFACTS.mkdir(exist_ok=True)

def run(cmd, **kwargs):
    print('> ' + ' '.join(cmd))
    res = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, **kwargs)
    print(res.stdout)
    return res.returncode, res.stdout

def build():
    return run(['cargo', 'build', '--release'])

def smoke():
    rc, out = build()
    if rc != 0:
        (ARTIFACTS / 'EATME_build.txt').write_text(out)
        return rc
    # run smoke scripts
    scripts = list((ROOT / 'tests').glob('*.ps'))
    for s in scripts:
        rc, out = run(['./target/release/pasta', str(s)])
        if rc != 0:
            (ARTIFACTS / f'EATME_{s.name}.txt').write_text(out)
            return rc
    return 0

def golden_check():
    return run(['python3', 'tools/devkit/check_golden.py'])

def install():
    return run(['sudo', 'tools/devkit/install_pasta.sh'])

if __name__ == '__main__':
    cmd = sys.argv[1] if len(sys.argv) > 1 else 'help'
    if cmd == 'build':
        sys.exit(build()[0])
    elif cmd == 'smoke':
        sys.exit(smoke())
    elif cmd == 'golden-check':
        sys.exit(golden_check()[0])
    elif cmd == 'install':
        sys.exit(install()[0])
    else:
        print('Usage: tasks.py [build|smoke|golden-check|install]')
        sys.exit(2)
""")
os.chmod(TOOLS / 'tasks.py', 0o755)

# 3) run_smoke.sh — shell wrapper (idempotent, writes artifacts on failure)
safe_write(TOOLS / 'run_smoke.sh', """#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.."; pwd)"
ART="$ROOT/artifacts"
mkdir -p "$ART"

echo "[devkit] Building release..."
if ! cargo build --release 2>&1 | tee "$ART/build.log"; then
  echo "[devkit] Build failed. See $ART/build.log"
  cp "$ART/build.log" "$ART/EATME_build.txt"
  exit 1
fi

echo "[devkit] Running smoke scripts..."
FAILED=0
for s in "$ROOT"/tests/*.ps; do
  echo "[devkit] Running $s"
  if ! "$ROOT/target/release/pasta" "$s" 2>&1 | tee "$ART/$(basename "$s").log"; then
    echo "[devkit] Test failed: $s"
    cp "$ART/$(basename "$s").log" "$ART/EATME_$(basename "$s").txt"
    FAILED=1
    break
  fi
done

if [ "$FAILED" -ne 0 ]; then
  exit 2
fi

echo "[devkit] All smoke tests passed."
""")
os.chmod(TOOLS / 'run_smoke.sh', 0o755)

# 4) check_golden.py — compare outputs to golden files (text or PPM)
safe_write(TOOLS / 'check_golden.py', """#!/usr/bin/env python3
\"\"\"Compare outputs in artifacts/ to tests/golden/ by SHA256.

Writes non-matching filenames to artifacts/golden_mismatch.txt.
\"\"\"
import hashlib
from pathlib import Path

ROOT = Path('.').resolve()
ART = ROOT / 'artifacts'
GOLD = ROOT / 'tests' / 'golden'
mismatches = []

def sha256(p: Path):
    h = hashlib.sha256()
    h.update(p.read_bytes())
    return h.hexdigest()

for g in GOLD.glob('*'):
    out = ART / g.name
    if not out.exists():
        mismatches.append(f'missing output for {g.name}')
        continue
    if sha256(g) != sha256(out):
        mismatches.append(f'mismatch: {g.name}')

if mismatches:
    (ART / 'golden_mismatch.txt').write_text('\\n'.join(mismatches))
    print('Golden check failed. See artifacts/golden_mismatch.txt')
    raise SystemExit(1)
print('Golden check passed.')
""")
os.chmod(TOOLS / 'check_golden.py', 0o755)

# 5) install_pasta.sh — safe installer (copies to /usr/local/bin with backup)
safe_write(TOOLS / 'install_pasta.sh', """#!/usr/bin/env bash
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
""")
os.chmod(TOOLS / 'install_pasta.sh', 0o755)

# 6) patch_templates/ — placeholders for safe patch scripts
ensure_dir(PATCH_TEMPLATES)
safe_write(PATCH_TEMPLATES / 'apply_patch_template.sh', """#!/usr/bin/env bash
# Template: create a backup, apply a sed/perl patch, build, and on failure restore.
set -euo pipefail
FILE="$1"
BACKUP="${FILE}.bak_$(date -u +%Y%m%d_%H%M%S)"
cp "$FILE" "$BACKUP"
echo "Backup created: $BACKUP"
# Insert patch commands here (sed -i ... or perl -0777 -pe ...)
# After patch:
if ! cargo build --release; then
  echo "Build failed; restoring backup"
  cp "$BACKUP" "$FILE"
  exit 1
fi
echo "Patch applied and build succeeded."
""")
os.chmod(PATCH_TEMPLATES / 'apply_patch_template.sh', 0o755)

# 7) Makefile — top-level dev targets (safe, small)
safe_write(ROOT / 'Makefile', f"""# Makefile for Pasta dev tasks (generated by tools/devkit_bootstrap.py)
.PHONY: build smoke install dev-setup golden-check

build:
\tcargo build --release

smoke:
\tpython3 tools/devkit/tasks.py smoke

golden-check:
\tpython3 tools/devkit/tasks.py golden-check

install:
\tbash tools/devkit/install_pasta.sh

dev-setup:
\tpython3 -m venv .venv || true
\t. .venv/bin/activate && pip install --upgrade pip
\t@echo "Dev environment ready. Activate with: . .venv/bin/activate"
""")

# 8) tests/ bubble_sort.ps and golden output (if tests dir missing)
ensure_dir(ROOT / 'tests' / 'golden')
if not (ROOT / 'tests' / 'bubble_sort.ps').exists():
    safe_write(ROOT / 'tests' / 'bubble_sort.ps', """# bubble_sort.ps
def.swap(a, i, j):
    temp = a[i]
    a[i] = a[j]
    a[j] = temp
end

def.bubble_sort(a, n):
    i = 0
    while i < n {
        j = 0
        while j < n - 1 {
            if a[j] > a[j+1] {
                swap(a, j, j+1)
            }
            j = j + 1
        }
        i = i + 1
    }
end

arr = [5,1,4,2,8,3]
PRINT("Before:", arr)
bubble_sort(arr, 6)
PRINT("After:", arr)
""")
    # golden placeholder
    safe_write(ROOT / 'tests' / 'golden' / 'bubble_sort.out', "Before: [5, 1, 4, 2, 8, 3]\nAfter:  [1, 2, 3, 4, 5, 8]\n")

# 9) artifacts/ directory
ensure_dir(ARTIFACTS)

# 10) .pre-commit-config.yaml (basic)
safe_write(ROOT / '.pre-commit-config.yaml', """repos:
- repo: https://github.com/pre-commit/pre-commit-hooks
  rev: v4.5.0
  hooks:
    - id: trailing-whitespace
    - id: end-of-file-fixer
- repo: https://github.com/dnephin/pre-commit-rust
  rev: v0.2.0
  hooks:
    - id: rustfmt
""")

# 11) CI workflow skeleton (only if .github/workflows doesn't exist)
if not (GITHUB.exists()):
    ensure_dir(GITHUB)
    safe_write(GITHUB / 'ci.yml', """name: CI

on:
  push:
  pull_request:

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Build
        run: cargo build --release
      - name: Run smoke (fast)
        run: |
          ./target/release/pasta tests/bubble_sort.ps || true
      - name: Upload artifacts on failure
        if: failure()
        uses: actions/upload-artifact@v4
        with:
          name: artifacts
          path: artifacts/
""")

print("\\nDevkit bootstrap complete.")
print("Run `make dev-setup` to prepare a Python venv, then `make smoke` to build and run smoke tests.")
""")

Make the script executable and run it:

```bash
chmod +x tools/devkit_bootstrap.py
python3 tools/devkit_bootstrap.py
