#!/usr/bin/env python3
"""Integration test: responses improve after training.

Creates a tiny corpus with a unique token, queries the CLI before and after
training, and asserts the post-training reply is different and not the
generic fallback.
"""
import os
import subprocess
import sys
import shutil

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', '..'))
BINARY = os.path.join(ROOT, 'bin', 'mage_cli')
TMPDIR = os.path.join(ROOT, 'tests', 'integration', 'tmp_response')
os.makedirs(TMPDIR, exist_ok=True)

def run(cmd, capture=True):
    print('RUN:', ' '.join(cmd))
    r = subprocess.run(cmd, check=False, capture_output=capture, text=True, env=None)
    print('RC=', r.returncode)
    if r.stdout: print('STDOUT:\n', r.stdout)
    if r.stderr: print('STDERR:\n', r.stderr)
    return r

def ensure_binary():
    if not os.path.exists(BINARY):
        print('mage_cli binary missing; building...')
        run(['make', 'bin/mage_cli'])
    if not os.path.exists(BINARY):
        print('Failed to build mage_cli')
        raise SystemExit(3)

def main():
    ensure_binary()
    # prepare tiny corpus with unique token
    inp = os.path.join(TMPDIR, 'train_unique.txt')
    with open(inp, 'w', encoding='utf-8') as f:
        f.write('foobaz qux\nfoobaz qux\n')

    # remove any pre-existing generated vocab to ensure clean state
    gv = os.path.join('data', 'vocab', 'generated_vocab.jsonl')
    try:
        if os.path.exists(gv): os.remove(gv)
    except Exception:
        pass

    # query before training
    before = run([BINARY, 'foobaz'])
    out_before = (before.stdout or '').strip()

    # run python trainer pipeline (avoids invoking C trainer)
    tp = run(['python3', 'python/tools/train_pipeline.py', '--input', inp, '--vocab-out', gv])
    if tp.returncode != 0:
        print('Training pipeline failed')
        raise SystemExit(4)

    # query after training
    after = run([BINARY, 'foobaz'])
    out_after = (after.stdout or '').strip()

    fallback = "I'm not sure how to respond to that." 
    if out_after == '' or out_after == fallback:
        print('Test failed: post-training response is fallback or empty')
        print('before:', out_before)
        print('after :', out_after)
        raise SystemExit(5)

    if out_after == out_before:
        print('Test failed: response did not change after training')
        print('before:', out_before)
        print('after :', out_after)
        raise SystemExit(6)

    # sanity: ensure the generated vocab contains our token
    found = False
    try:
        with open(gv, 'r', encoding='utf-8') as f:
            for line in f:
                if '"key": "foobaz"' in line:
                    found = True; break
    except Exception:
        pass
    if not found:
        print('Warning: generated vocab did not contain key foobaz')

    print('Test passed: response improved after training')

if __name__ == '__main__':
    main()
