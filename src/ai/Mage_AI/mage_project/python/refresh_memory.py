#!/usr/bin/env python3
"""
Simple helper to append conversation lines to data/context_history.jsonl
Usage:
  python3 python/refresh_memory.py --role user --text "Hello"  # appends one entry
  python3 python/refresh_memory.py --file somefile.jsonl         # merge lines
"""
import argparse
import json
import sys
from pathlib import Path

DATA = Path(__file__).resolve().parents[1] / 'data'
DATA.mkdir(parents=True, exist_ok=True)
CTX = DATA / 'context_history.jsonl'

parser = argparse.ArgumentParser()
parser.add_argument('--role', help='role name (user/system/assistant)', default='user')
parser.add_argument('--text', help='text to append')
parser.add_argument('--file', help='merge an existing jsonl file into context history')
args = parser.parse_args()

if args.file:
    p = Path(args.file)
    if not p.exists():
        print('file not found', args.file, file=sys.stderr)
        sys.exit(1)
    with p.open('r', encoding='utf-8') as src, CTX.open('a', encoding='utf-8') as dst:
        for line in src:
            dst.write(line if line.endswith('\n') else (line + '\n'))
    print('merged', args.file)
    sys.exit(0)

if not args.text:
    print('nothing to append; use --text', file=sys.stderr)
    sys.exit(1)

entry = {'role': args.role, 'text': args.text}
with CTX.open('a', encoding='utf-8') as f:
    f.write(json.dumps(entry, ensure_ascii=False) + '\n')
print('appended to', CTX)
