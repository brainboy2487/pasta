#!/usr/bin/env python3
"""Build a simple vocabulary JSONL from a corpus file.

This is a Python implementation of the C `mage_api_build_vocab_from_file` functionality
so we can run training without invoking the C CLI (useful while debugging crashes).

It accepts a text file or JSONL and writes `data/vocab/generated_vocab.jsonl`-style
entries: {"id": <hash>, "key": <token>, "responses": [<token>], "type": "other"}

Usage:
  python3 python/tools/train_vocab.py --input data/corpus/scraped.jsonl --out data/vocab/generated_vocab.jsonl
"""
import argparse
import json
import os
import hashlib
import re
from collections import Counter


def simple_tokenize(text):
    # lower, split on non-word, filter short tokens
    text = text.lower()
    toks = re.findall(r"[a-z0-9]+", text)
    return [t for t in toks if len(t) > 0]


def extract_text_from_jsonl_line(line):
    try:
        obj = json.loads(line)
        if isinstance(obj, dict):
            return obj.get('text') or obj.get('content') or obj.get('body') or None
    except Exception:
        return None
    return None


def build_vocab_from_file(input_path, out_path, min_count=1):
    counts = Counter()
    # read lines; if JSONL, extract text per object; otherwise whole file
    saw_json = False
    try:
        with open(input_path, 'r', encoding='utf-8') as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                t = extract_text_from_jsonl_line(line)
                if t is not None:
                    saw_json = True
                    toks = simple_tokenize(t)
                    counts.update(toks)
                else:
                    # treat as plain text line
                    toks = simple_tokenize(line)
                    counts.update(toks)
        # if file wasn't JSONL but is plain text, we already counted lines; ok
    except FileNotFoundError:
        return -1

    # filter and sort
    items = [(tok, c) for tok, c in counts.items() if c >= min_count]
    if not items:
        return 0
    items.sort(key=lambda x: (-x[1], x[0]))

    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    written = 0
    with open(out_path, 'w', encoding='utf-8') as out:
        for tok, c in items:
            # id: short stable hash of token
            hid = hashlib.sha1(tok.encode('utf-8')).hexdigest()[:16]
            # Include both `key` and `token` fields for compatibility with
            # the C generators (`warm_generate_from_vocab` / `cold_generate_from_vocab`) which
            # look for a "token" field in the JSONL. Keep `key` for other consumers.
            entry = {"id": hid, "key": tok, "token": tok, "responses": [tok], "type": "other"}
            out.write(json.dumps(entry, ensure_ascii=False) + '\n')
            written += 1
    return written


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--input', '-i', required=True)
    p.add_argument('--out', '-o', default='data/vocab/generated_vocab.jsonl')
    p.add_argument('--min-count', type=int, default=1)
    args = p.parse_args()

    n = build_vocab_from_file(args.input, args.out, args.min_count)
    if n < 0:
        print('Input file not found or unreadable')
        raise SystemExit(2)
    print(f'Wrote {n} vocab entries to {args.out}')


if __name__ == '__main__':
    main()
