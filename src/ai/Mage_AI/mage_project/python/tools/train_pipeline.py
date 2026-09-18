#!/usr/bin/env python3
"""Simple training pipeline helper for Mage.

Usage: python3 python/tools/train_pipeline.py --input data/corpus/scraped.jsonl

Steps performed:
 - call ./bin/mage_cli generate-vocab <input> <vocab_out>
 - optionally run tools/gen_mphf -> bin/gen_mphf to generate data/mphf.bin
 - optionally run ./bin/mage_cli vectorize to create vectors
"""
import argparse
import subprocess
import os
import shutil
import sys


def run(cmd, check=True):
    print('RUN:', ' '.join(cmd))
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.stdout: print(r.stdout)
    if r.stderr: print(r.stderr, file=sys.stderr)
    if check and r.returncode != 0:
        raise SystemExit(r.returncode)
    return r.returncode


def find_tool(name):
    # prefer bin/ copy, fallback to tools/
    cand = os.path.join('bin', name)
    if os.path.exists(cand) and os.access(cand, os.X_OK):
        return cand
    cand2 = os.path.join('tools', name)
    if os.path.exists(cand2) and os.access(cand2, os.X_OK):
        return cand2
    return None


def main():
    p = argparse.ArgumentParser()
    p.add_argument('--input', '-i', required=True, help='Input JSONL or text corpus')
    p.add_argument('--vocab-out', default='data/vocab/generated_vocab.jsonl')
    p.add_argument('--run-mphf', action='store_true', help='Run gen_mphf after vocab generation')
    p.add_argument('--vectorize', action='store_true', help='Run mage_cli vectorize step after training')
    p.add_argument('--py-vectorize', action='store_true', help='Use Python vectorizer fallback instead of CLI')
    p.add_argument('--snapshot', action='store_true', help='Write a timestamped snapshot of generated vocab into snapshots/')
    p.add_argument('--manifest', help='Write a run manifest JSON path', default=None)
    args = p.parse_args()

    # Ensure output dirs
    os.makedirs(os.path.dirname(args.vocab_out), exist_ok=True)

    cli = os.path.join('.', 'bin', 'mage_cli')
    if not os.path.exists(cli):
        print('mage_cli binary not found at ./bin/mage_cli; build first', file=sys.stderr)
        raise SystemExit(2)

    # Generate vocab: prefer Python implementation to avoid C CLI crashes
    py_trainer = os.path.join('python', 'tools', 'train_vocab.py')
    if os.path.exists(py_trainer):
        try:
            run(['python3', py_trainer, '--input', args.input, '--out', args.vocab_out])
        except SystemExit as e:
            print('python train_vocab failed', file=sys.stderr)
            raise
    else:
        try:
            run([cli, 'generate-vocab', args.input, args.vocab_out])
        except SystemExit as e:
            print('generate-vocab failed', file=sys.stderr)
            raise

    # Optionally run gen_mphf
    if args.run_mphf:
        gm = find_tool('gen_mphf') or find_tool('gen_mphf_chd')
        if not gm:
            print('gen_mphf tool not found in bin/ or tools/; skipping MPHF generation', file=sys.stderr)
        else:
            out_mphf = 'data/mphf.bin'
            os.makedirs(os.path.dirname(out_mphf), exist_ok=True)
            try:
                run([gm, args.vocab_out, out_mphf])
            except SystemExit:
                print('gen_mphf failed', file=sys.stderr)
                raise

    # Optionally vectorize
    if args.vectorize:
        vec_out = os.path.join('data', 'vectors')
        os.makedirs(vec_out, exist_ok=True)
        if args.py_vectorize:
            # use python fallback
            py_vec = os.path.join('python', 'tools', 'vectorize.py')
            if os.path.exists(py_vec):
                try:
                    run(['python3', py_vec, '--input', args.input, '--output', vec_out])
                except SystemExit:
                    print('python vectorize failed', file=sys.stderr)
                    raise
            else:
                print('python vectorize not found; falling back to CLI vectorize', file=sys.stderr)
                run([cli, 'vectorize', '--input', args.input, '--output', vec_out])
        else:
            try:
                run([cli, 'vectorize', '--input', args.input, '--output', vec_out])
            except SystemExit:
                print('vectorize failed', file=sys.stderr)
                raise

    # snapshot generated vocab if requested
    if args.snapshot:
        import time
        ts = int(time.time())
        dst = os.path.join('snapshots', f'generated_vocab.jsonl.{ts}')
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        try:
            shutil.copyfile(args.vocab_out, dst)
            print('Wrote snapshot to', dst)
        except Exception as e:
            print('Failed to write snapshot:', e, file=sys.stderr)

    # write run manifest if requested
    if args.manifest:
        import json, time
        manifest = {'run_time': int(time.time()), 'input': args.input, 'vocab_out': args.vocab_out}
        try:
            with open(args.manifest, 'w', encoding='utf-8') as mf:
                json.dump(manifest, mf, ensure_ascii=False, indent=2)
            print('Wrote manifest to', args.manifest)
        except Exception as e:
            print('Failed to write manifest:', e, file=sys.stderr)

    print('Training pipeline completed. Vocab:', args.vocab_out)


if __name__ == '__main__':
    main()
