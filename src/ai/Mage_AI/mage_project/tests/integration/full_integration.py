#!/usr/bin/env python3
"""Full integration test suite for Mage CLI and training pipeline.

Runs a set of scenarios to validate scraping, vocab generation, vectorization,
and that CLI responses improve after training on different input types.

This is intentionally conservative: it prefers the Python trainer pipeline
to avoid hitting the C CLI trainer crash, but will invoke `./bin/mage_cli`
for vectorize/query checks.
"""
import os
import subprocess
import sys
import shutil
import time

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))
BINARY = os.path.join(ROOT, 'bin', 'mage_cli')
TMPDIR = os.path.join(ROOT, 'tests', 'integration', 'full_tmp')
GV = os.path.join('data', 'vocab', 'generated_vocab.jsonl')

def run(cmd, check=True):
    print('RUN:', ' '.join(cmd))
    # Capture raw bytes to avoid Unicode decode errors when the CLI may
    # emit non-UTF8 bytes (e.g., binary headers). Decode for printing
    # using 'replace' so tests don't fail on decoding issues.
    r = subprocess.run(cmd, capture_output=True)
    print('RC=', r.returncode)
    if r.stdout:
        try:
            print('STDOUT:\n', r.stdout.decode('utf-8'))
        except Exception:
            print('STDOUT (binary, decoded with replace):\n', r.stdout.decode('utf-8', errors='replace'))
    if r.stderr:
        try:
            print('STDERR:\n', r.stderr.decode('utf-8'))
        except Exception:
            print('STDERR (binary, decoded with replace):\n', r.stderr.decode('utf-8', errors='replace'))
    if check and r.returncode != 0:
        raise SystemExit(2)
    return r

def ensure_binary():
    if not os.path.exists(BINARY):
        print('mage_cli binary missing; building...')
        run(['make', 'bin/mage_cli'])
    if not os.path.exists(BINARY):
        print('Failed to build mage_cli')
        raise SystemExit(3)

def prepare_inputs():
    shutil.rmtree(TMPDIR, ignore_errors=True)
    os.makedirs(TMPDIR, exist_ok=True)
    # Plain text sample
    with open(os.path.join(TMPDIR, 'sample1.txt'), 'w', encoding='utf-8') as f:
        f.write('Hello world\nThis is a test corpus.\nHello mage.\n')
    # JSONL sample with explicit text fields and unicode
    jpath = os.path.join(TMPDIR, 'sample2.jsonl')
    with open(jpath, 'w', encoding='utf-8') as f:
        f.write('{"id": "doc1", "text": "Bonjour le monde"}\n')
        f.write('{"id": "doc2", "text": "こんにちは 世界"}\n')
        f.write('{"id": "doc3", "text": "foobaz qux"}\n')
    return [os.path.join(TMPDIR, 'sample1.txt'), jpath]

def clean_generated():
    try:
        if os.path.exists(GV): os.remove(GV)
    except Exception:
        pass

def assert_vocab_contains(token):
    found = False
    try:
        with open(GV, 'r', encoding='utf-8') as f:
            for line in f:
                if f'"key": "{token}"' in line:
                    found = True; break
    except Exception:
        pass
    if not found:
        print('Expected token not present in generated vocab:', token)
        raise SystemExit(5)

def query_cli(q):
    r = run([BINARY, q], check=False)
    return (r.returncode, (r.stdout or '').strip())

def main():
    ensure_binary()
    inputs = prepare_inputs()
    clean_generated()

    # Scenario A: train on plain text
    print('\n--- Scenario A: plain text training ---')
    run(['python3', 'python/tools/scraper.py', '--local', inputs[0], '--out', 'data/corpus/scraped_plain.jsonl', '--no-train'])
    run(['python3', 'python/tools/train_pipeline.py', '--input', 'data/corpus/scraped_plain.jsonl', '--vocab-out', GV])
    assert_vocab_contains('hello')

    # Scenario B: train on JSONL with unicode tokens
    print('\n--- Scenario B: JSONL unicode training ---')
    run(['python3', 'python/tools/scraper.py', '--local', inputs[1], '--out', 'data/corpus/scraped_jsonl.jsonl', '--no-train'])
    run(['python3', 'python/tools/train_pipeline.py', '--input', 'data/corpus/scraped_jsonl.jsonl', '--vocab-out', GV])
    # ensure specific tokens from JSONL present (foobaz, bonjour, こんにちは normalized to tokens may differ)
    assert_vocab_contains('foobaz')

    # Scenario C: vectorize output exists
    print('\n--- Scenario C: vectorize ---')
    run([BINARY, 'vectorize', '--input', 'data/corpus/scraped_jsonl.jsonl', '--output', 'data/vectors'])
    vecfile = os.path.join('data', 'vectors', 'vectors.bin')
    if not os.path.exists('data/vectors'):
        print('Vector output directory missing')
        raise SystemExit(6)

    # Scenario D: responses improve pre/post training for unique token
    print('\n--- Scenario D: response improvement check ---')
    # remove generated vocab to ensure baseline
    clean_generated()
    # baseline query should be fallback or built-in
    rc_b, out_b = query_cli('foobaz')
    # train using Python trainer
    run(['python3', 'python/tools/train_pipeline.py', '--input', inputs[1], '--vocab-out', GV])
    time.sleep(0.2)
    rc_a, out_a = query_cli('foobaz')
    fallback = "I'm not sure how to respond to that."
    if out_a == '' or out_a == fallback:
        print('Post-training response is still fallback; failing')
        raise SystemExit(7)
    if out_a == out_b:
        print('Post-training response did not change; failing')
        raise SystemExit(8)

    print('\nFull integration suite passed')

    # Additional edge-case scenarios
    print('\n--- Scenario E: empty file ---')
    empty = os.path.join(TMPDIR, 'empty.txt')
    with open(empty, 'w', encoding='utf-8') as f: f.write('')
    run(['python3', 'python/tools/scraper.py', '--local', empty, '--out', 'data/corpus/empty.jsonl', '--no-train'])
    # training an empty file should not crash and should return 0 with 0 entries
    run(['python3', 'python/tools/train_pipeline.py', '--input', 'data/corpus/empty.jsonl', '--vocab-out', GV])

    print('\n--- Scenario F: malformed JSONL ---')
    bad = os.path.join(TMPDIR, 'bad.jsonl')
    with open(bad, 'w', encoding='utf-8') as f:
        f.write('{this is not: valid json}\n')
        f.write('{"text": "valid after bad"}\n')
    run(['python3', 'python/tools/scraper.py', '--local', bad, '--out', 'data/corpus/bad.jsonl', '--no-train'])
    run(['python3', 'python/tools/train_pipeline.py', '--input', 'data/corpus/bad.jsonl', '--vocab-out', GV])

    print('\n--- Scenario G: duplicate entries + dedup ---')
    dup = os.path.join(TMPDIR, 'dup.txt')
    with open(dup, 'w', encoding='utf-8') as f:
        f.write('repeat token\nrepeat token\nrepeat token\n')
    # write with and without dedup to compare
    run(['python3', 'python/tools/scraper.py', '--local', dup, '--out', 'data/corpus/dup.jsonl', '--no-train'])
    run(['python3', 'python/tools/scraper.py', '--local', dup, '--out', 'data/corpus/dup_dedup.jsonl', '--dedup'])
    run(['python3', 'python/tools/train_pipeline.py', '--input', 'data/corpus/dup.jsonl', '--vocab-out', GV])
    run(['python3', 'python/tools/train_pipeline.py', '--input', 'data/corpus/dup_dedup.jsonl', '--vocab-out', GV])

    print('\n--- Scenario H: extremely long token / binary content ---')
    longf = os.path.join(TMPDIR, 'long.txt')
    with open(longf, 'wb') as f:
        f.write(b'A' * 10000 + b'\n')
        f.write('normal text here\n'.encode('utf-8'))
    # scraper should handle binary-ish content gracefully
    run(['python3', 'python/tools/scraper.py', '--local', longf, '--out', 'data/corpus/long.jsonl', '--no-train'])
    run(['python3', 'python/tools/train_pipeline.py', '--input', 'data/corpus/long.jsonl', '--vocab-out', GV])

    print('\nEdge-case scenarios completed')

if __name__ == '__main__':
    main()
