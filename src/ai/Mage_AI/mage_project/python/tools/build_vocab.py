#!/usr/bin/env python3
"""
Simple vocabulary builder for MAGE.

Reads one or more input files (plain text or JSONL with a `text` field) and
emits a vocabulary JSONL file with token counts sorted by frequency.

Usage:
  python3 python/tools/build_vocab.py --in file1.txt [file2.jsonl ...] --out data/vocab/generated_vocab.jsonl [--min-count N] [--merge existing_vocab.jsonl]

This is intentionally small and dependency-free; tokenizer is a simple
Unicode-normalizing, lowercase, word-extraction routine. It creates lines of
the form: {"token":"...","count":NN}
"""
import sys
import argparse
import json
import re
import unicodedata
from collections import Counter


def normalize_text(s: str) -> str:
    s = unicodedata.normalize('NFKC', s)
    s = s.lower()
    return s


WORD_RE = re.compile(r"\w+", re.UNICODE)


def tokenize(text: str):
    text = normalize_text(text)
    return WORD_RE.findall(text)


def extract_texts_from_jsonl(path):
    with open(path, 'r', encoding='utf-8') as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                obj = json.loads(line)
            except Exception:
                continue
            # common fields: text, content, body
            for key in ('text', 'content', 'body'):
                if key in obj and isinstance(obj[key], str):
                    yield obj[key]
                    break


def read_plain(path):
    with open(path, 'r', encoding='utf-8') as f:
        for line in f:
            yield line


def merge_existing(counter: Counter, path: str):
    try:
        with open(path, 'r', encoding='utf-8') as f:
            for line in f:
                try:
                    obj = json.loads(line)
                except Exception:
                    continue
                if 'token' in obj and 'count' in obj:
                    counter[obj['token']] += int(obj['count'])
    except FileNotFoundError:
        return


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--in', '-i', dest='inputs', nargs='+', required=True, help='Input files (plain text or JSONL)')
    p.add_argument('--out', '-o', dest='out', required=True, help='Output vocabulary JSONL path')
    p.add_argument('--min-count', dest='min_count', type=int, default=1, help='Minimum token count to include')
    p.add_argument('--merge', dest='merge', default=None, help='Merge counts from existing vocab JSONL')
    args = p.parse_args()

    counter = Counter()

    if args.merge:
        merge_existing(counter, args.merge)

    for path in args.inputs:
        path = path.strip()
        # heuristics: JSONL files often end with .jsonl
        if path.endswith('.jsonl'):
            for text in extract_texts_from_jsonl(path):
                for tok in tokenize(text):
                    counter[tok] += 1
        else:
            for line in read_plain(path):
                for tok in tokenize(line):
                    counter[tok] += 1

    # filter and write
    items = [(t, c) for t, c in counter.items() if c >= args.min_count]
    items.sort(key=lambda x: (-x[1], x[0]))

    with open(args.out, 'w', encoding='utf-8') as out:
        for t, c in items:
            json.dump({'token': t, 'count': c}, out, ensure_ascii=False)
            out.write('\n')

    print(f'Wrote {len(items)} tokens to {args.out}', file=sys.stderr)


if __name__ == '__main__':
    main()
