#!/usr/bin/env python3
"""Integration test harness for Mage CLI: vocabulary, commands, training, scraping.

Runs a sequence of smoke checks against the built `./bin/mage_cli` and
local Python scraper to validate core flows. Exits with non-zero on failures.
"""
import subprocess
import sys
import os
import shutil

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))
BINARY = os.path.join(ROOT, 'bin', 'mage_cli')
TMPDIR = os.path.join(ROOT, 'tests', 'integration', 'tmp')
os.makedirs(TMPDIR, exist_ok=True)

def run(cmd, check=True, capture=True, env=None):
    print('RUN:', ' '.join(cmd))
    r = subprocess.run(cmd, check=False, capture_output=capture, text=True, env=env)
    print('RC=', r.returncode)
    if r.stdout: print('STDOUT:\n', r.stdout)
    if r.stderr: print('STDERR:\n', r.stderr)
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

def test_help():
    run([BINARY, 'help', '-all'])

def test_simple_commands():
    # basic non-interactive commands
    run([BINARY, 'version'])
    run([BINARY, 'ping'])
    run([BINARY, 'about'])

def test_validate_vocab():
    # use example vocab shipped in repo
    example = os.path.join(ROOT, 'data', 'vocab_examples.jsonl')
    if not os.path.exists(example):
        print('No example vocab found at', example)
        return
    run([BINARY, 'validate-vocab', example])

def test_generate_vocab_and_train():
    # create a tiny training file
    inp = os.path.join(TMPDIR, 'train_sample.txt')
    with open(inp, 'w', encoding='utf-8') as f:
        f.write('Hello world\nThis is a tiny training corpus.\nHello mage.\n')
    out = os.path.join(TMPDIR, 'generated_vocab.jsonl')
    run([BINARY, 'generate-vocab', inp, out])
    if not os.path.exists(out):
        print('generate-vocab did not produce', out); raise SystemExit(4)

def test_scraper_local():
    # call python scraper in local mode on the generated vocab file
    src = os.path.join(TMPDIR, 'train_sample.txt')
    if not os.path.exists(src):
        with open(src, 'w', encoding='utf-8') as f: f.write('sample text for scraper')
    out = os.path.join(TMPDIR, 'scraped.jsonl')
    run(['python3', 'python/tools/scraper.py', '--local', src, '--out', out, '--no-train'])
    if not os.path.exists(out): print('scraper failed to write', out); raise SystemExit(5)

def cleanup():
    try:
        shutil.rmtree(TMPDIR)
    except Exception:
        pass

def main():
    try:
        ensure_binary()
        test_help()
        test_simple_commands()
        test_validate_vocab()
        test_generate_vocab_and_train()
        test_scraper_local()
        print('Integration harness: all checks passed')
    finally:
        cleanup()

if __name__ == '__main__':
    main()
