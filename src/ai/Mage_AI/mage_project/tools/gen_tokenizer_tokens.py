#!/usr/bin/env python3
import unicodedata
import os

in_path = 'tests/golden/tokenizer_golden.txt'
out_path = 'tests/golden/tokenizer_tokens.txt'
os.makedirs('tests/golden', exist_ok=True)

def py_tokenize(norm):
    toks = []
    cur = []
    for ch in norm:
        o = ord(ch)
        if o <= 127:
            if ch.isalnum() or ch in ("'", "-"):
                cur.append(ch)
            else:
                if cur:
                    toks.append(''.join(cur))
                    cur = []
        else:
            # non-ASCII treated as token char
            cur.append(ch)
    if cur:
        toks.append(''.join(cur))
    return toks

with open(in_path, 'r', encoding='utf-8') as f_in, open(out_path, 'w', encoding='utf-8') as f_out:
    lines = [l.rstrip('\n') for l in f_in.readlines()]
    i = 0
    while i + 1 < len(lines):
        orig = lines[i]
        norm = lines[i+1]
        # Remove zero-width joiners/zero-width spaces to match C tokenizer behavior
        norm = norm.replace('\u200d', '')
        norm = norm.replace('\u200b', '')
        toks = py_tokenize(norm)
        f_out.write(orig + '\n')
        f_out.write(' '.join(toks) + '\n')
        f_out.write('\n')
        i += 3

print('Wrote', out_path)
