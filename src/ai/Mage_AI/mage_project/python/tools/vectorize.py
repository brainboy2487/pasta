#!/usr/bin/env python3
"""Deterministic Python vectorizer to produce data/vectors/vectors.bin

Writes a simple vectors.bin with header (uint32 n, uint32 d) followed by n*d float32.
Also writes a manifest file `data/vectors/manifest.jsonl` mapping index->doc id.

Usage: python3 python/tools/vectorize.py --input data/corpus/scraped.jsonl --output data/vectors --dim 8
"""
import argparse
import os
import struct
import json
import hashlib


def deterministic_vector(text, dim):
    # simple FNV-like hash to generate bytes then map to floats in [-1,1]
    h = 1469598101
    if text:
        for b in text.encode('utf-8', errors='replace'):
            h = (h ^ b) * 16777619 & 0xFFFFFFFF
    out = []
    for i in range(dim):
        v = ((h >> (i % 24)) & 0xFF) - 128
        out.append(float(v) / 128.0)
    return out


def read_entries(input_path):
    entries = []
    if not os.path.exists(input_path):
        return entries
    with open(input_path, 'r', encoding='utf-8') as f:
        for line in f:
            line = line.strip()
            if not line: continue
            try:
                obj = json.loads(line)
                if isinstance(obj, dict) and 'text' in obj:
                    entries.append((obj.get('id') or '', obj.get('text') or ''))
                    continue
            except Exception:
                pass
            # fallback plain text line
            entries.append(('', line))
    return entries


def write_vectors(out_dir, vectors, ids, dim):
    os.makedirs(out_dir, exist_ok=True)
    vec_path = os.path.join(out_dir, 'vectors.bin')
    with open(vec_path, 'wb') as f:
        # header: uint32 n, uint32 d
        f.write(struct.pack('<I', len(vectors)))
        f.write(struct.pack('<I', dim))
        for vec in vectors:
            for v in vec:
                f.write(struct.pack('<f', float(v)))
    # write manifest mapping
    man_path = os.path.join(out_dir, 'manifest.jsonl')
    with open(man_path, 'w', encoding='utf-8') as mf:
        for i, idv in enumerate(ids):
            mf.write(json.dumps({'index': i, 'id': idv}, ensure_ascii=False) + '\n')
    return vec_path, man_path


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--input', '-i', required=True)
    p.add_argument('--output', '-o', default='data/vectors')
    p.add_argument('--dim', type=int, default=8)
    args = p.parse_args()

    entries = read_entries(args.input)
    if not entries:
        # nothing to vectorize; create empty vectors.bin with n=0 d=0 (io expects non-zero, so skip)
        print('No entries to vectorize')
        return 0

    vectors = []
    ids = []
    for i, (eid, text) in enumerate(entries):
        vec = deterministic_vector(text, args.dim)
        vectors.append(vec)
        ids.append(eid or f'doc-{i}')

    vp, mp = write_vectors(args.output, vectors, ids, args.dim)
    print('Wrote vectors to', vp)
    print('Wrote manifest to', mp)
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
