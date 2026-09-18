#!/usr/bin/env python3
"""Check presentation adapters include only the public API header.

Scans a set of presentation files (CLI, GUI, services) and flags any
`#include "mage/*.h"` that is not `mage/api.h`.
"""
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PAT = re.compile(r'#include\s+"mage/([^"]+)"')

paths = [
    ROOT / 'src' / 'mage_cli.c',
    ROOT / 'src' / 'gui' / 'mage_gui.c',
    ROOT / 'services' / 'api' / 'server.py',
]

bad = []
for p in paths:
    if not p.exists():
        continue
    text = p.read_text()
    for m in PAT.finditer(text):
        hdr = m.group(1)
        if hdr != 'api.h':
            bad.append((str(p.relative_to(ROOT)), hdr))

if bad:
    print('Presentation include policy violations found:')
    for f, h in bad:
        print(f' - {f} includes mage/{h}')
    raise SystemExit(2)
else:
    print('Presentation include check: OK')
