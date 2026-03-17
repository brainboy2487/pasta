#!/usr/bin/env python3
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
    (ART / 'golden_mismatch.txt').write_text('\n'.join(mismatches))
    print('Golden check failed. See artifacts/golden_mismatch.txt')
    raise SystemExit(1)
print('Golden check passed.')
