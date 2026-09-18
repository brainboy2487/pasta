#!/usr/bin/env python3
"""Scraper + training helper for Mage.

This script supports three main modes:
- `--url <url>`: fetch a single URL and extract visible text.
- `--local <path>`: read a local JSONL or text file and repackage it.
- No args: iterate a small set of predefined sources.

It writes output as JSONL lines with `{"id":..., "text":...}` and can
optionally run the training step (`./bin/mage_cli train <out>`) and then
invoke a generation for a provided `--prompt`, piping the generated
reply through the coherence checker.
"""

import argparse
import sys
import json
import os
import time
import subprocess
from urllib.parse import urlparse
import tempfile
import shutil

try:
    import requests
    from bs4 import BeautifulSoup
    HAS_NET = True
except Exception:
    requests = None
    BeautifulSoup = None
    HAS_NET = False
import urllib.request
import urllib.error
import re
import html as _html


DEFAULT_SOURCES = [
    {"name": "wikipedia:Python", "type": "wikipedia", "id": "Python_(programming_language)"},
    {"name": "wikipedia:MachineLearning", "type": "wikipedia", "id": "Machine_learning"},
    {"name": "reddit:r/programming", "type": "reddit", "url": "https://www.reddit.com/r/programming/"},
    {"name": "quora:ai", "type": "quora", "url": "https://www.quora.com/topic/Artificial-Intelligence"},
]

HEADERS = {"User-Agent": "mage-scraper/1.0 (+https://example.invalid)"}


def extract_text_from_html(html_text):
    if BeautifulSoup:
        s = BeautifulSoup(html_text, 'html.parser')
        for sct in s(['script', 'style', 'noscript', 'header', 'footer', 'nav']):
            sct.decompose()
        parts = []
        for p in s.find_all('p'):
            t = p.get_text(separator=' ', strip=True)
            if t:
                parts.append(t)
        if parts:
            return '\n\n'.join(parts)
        return s.get_text(separator=' ', strip=True)
    # Fallback simple HTML stripper
    # Remove script/style blocks
    html_text = re.sub(r'<(script|style)[\s\S]*?</\1>', ' ', html_text, flags=re.I)
    # Strip tags
    text = re.sub(r'<[^>]+>', ' ', html_text)
    # Unescape HTML entities and normalize whitespace
    text = _html.unescape(text)
    return '\n\n'.join([p.strip() for p in text.splitlines() if p.strip()]) or ' '.join(text.split())


def fetch_url(url):
    """Fetch URL and return text using `requests` when available, otherwise
    fallback to `urllib.request`.
    """
    if requests and HAS_NET:
        r = requests.get(url, headers=HEADERS, timeout=15)
        r.raise_for_status()
        return r.text
    # Fallback using urllib
    req = urllib.request.Request(url, headers=HEADERS)
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = resp.read()
            # Attempt to decode using charset from headers
            ct = resp.headers.get_content_charset()
            if not ct:
                ct = 'utf-8'
            try:
                return data.decode(ct, errors='replace')
            except Exception:
                return data.decode('utf-8', errors='replace')
    except urllib.error.HTTPError as e:
        raise
    except Exception:
        raise


def fetch_wikipedia(title):
    if HAS_NET:
        api = f'https://en.wikipedia.org/api/rest_v1/page/mobile-sections/{title}'
        try:
            r = requests.get(api, headers=HEADERS, timeout=15)
            r.raise_for_status()
            data = r.json()
            chunks = []
            lead = data.get('lead', {})
            if 'sections' in lead:
                for s in lead['sections']:
                    if 'text' in s: chunks.append(s['text'])
            rem = data.get('remaining', {})
            if 'sections' in rem:
                for s in rem['sections']:
                    if 'text' in s: chunks.append(s['text'])
            if chunks:
                return extract_text_from_html('\n'.join(chunks))
        except Exception:
            pass
    page = f'https://en.wikipedia.org/wiki/{title}'
    html = fetch_url(page)
    return extract_text_from_html(html)


def fetch_reddit(url):
    try:
        jurl = url
        if not url.endswith('.json'):
            jurl = url.rstrip('/') + '/.json'
        txt = fetch_url(jurl)
        data = json.loads(txt)
        texts = []
        if isinstance(data, dict) and 'data' in data:
            for c in data['data'].get('children', []):
                d = c.get('data', {})
                if d.get('title'): texts.append(d.get('title'))
                if d.get('selftext'): texts.append(d.get('selftext'))
        elif isinstance(data, list):
            for item in data:
                if isinstance(item, dict) and 'data' in item:
                    for c in item['data'].get('children', []):
                        b = c.get('data', {}).get('body')
                        if b: texts.append(b)
        if texts:
            return '\n\n'.join(texts)
    except Exception:
        pass
    # fallback to HTML extraction
    try:
        html = fetch_url(url)
        return extract_text_from_html(html)
    except Exception:
        return ''


def fetch_generic(url):
    html = fetch_url(url)
    return extract_text_from_html(html)


def read_local_file(path):
    # Deprecated: kept for backward compatibility. Prefer read_local_entries.
    texts = []
    with open(path, 'r', encoding='utf-8') as f:
        for line in f:
            line = line.strip()
            if not line: continue
            try:
                obj = json.loads(line)
                if isinstance(obj, dict) and 'text' in obj:
                    texts.append(obj['text'])
                    continue
            except Exception:
                pass
            texts.append(line)
    return '\n\n'.join(texts)


def read_local_entries(path):
    """Return a list of document dicts from a local file.

    - If the file appears to be JSONL (extension '.jsonl' or lines parse as JSON objects),
      each JSON object becomes an entry (object must contain 'text' or 'content' or 'body').
    - Otherwise, treat the whole file as a single text document.
    """
    entries = []
    try:
        with open(path, 'r', encoding='utf-8') as f:
            # attempt JSONL first
            lineno = 0
            any_json = False
            for line in f:
                lineno += 1
                s = line.strip()
                if not s: continue
                try:
                    obj = json.loads(s)
                    any_json = True
                    if isinstance(obj, dict):
                        text = obj.get('text') or obj.get('content') or obj.get('body') or None
                        if text:
                            ent = {"id": obj.get('id') or f"local-{lineno}", "text": text}
                        else:
                            # stringify object as fallback
                            ent = {"id": obj.get('id') or f"local-{lineno}", "text": json.dumps(obj, ensure_ascii=False)}
                        entries.append(ent)
                        continue
                except Exception:
                    # not JSON; continue scanning
                    pass
                # not JSON: treat as plain text line entry
                entries.append({"id": f"local-{lineno}", "text": s})
            if any_json:
                return entries
    except Exception:
        # fall back to single-document read
        pass

    # Not JSONL or failed to parse: read whole file as one document
    try:
        with open(path, 'r', encoding='utf-8') as f:
            txt = f.read()
            if txt is None: txt = ''
            return [{"id": "local-0", "text": '\n\n'.join([l.strip() for l in txt.splitlines() if l.strip()])}]
    except Exception:
        return []


def write_jsonl(outpath, entries):
    os.makedirs(os.path.dirname(outpath), exist_ok=True)
    with open(outpath, 'w', encoding='utf-8') as f:
        for e in entries:
            f.write(json.dumps(e, ensure_ascii=False) + '\n')


def train_entries(entries, cli_path='./bin/mage_cli', keep_temp=False, timeout=None):
    """Train on an in-memory list of `entries` by writing them to a
    temporary JSONL file and invoking the trainer CLI.

    Returns (returncode, stdout, stderr, temp_path).
    The temporary path is removed unless `keep_temp` is True.
    """
    if not entries:
        return (1, '', 'no entries', None)

    td = tempfile.mkdtemp(prefix='mage_scrape_')
    tmp_path = os.path.join(td, 'scraped_tmp.jsonl')
    try:
        write_jsonl(tmp_path, entries)
        # Prefer the Python training pipeline to avoid invoking the C CLI trainer
        # which has exhibited segfaults in CI. Fall back to the CLI trainer if
        # the Python pipeline is not available.
        py_pipeline = os.path.join('python', 'tools', 'train_pipeline.py')
        if os.path.exists(py_pipeline):
            cmd = ['python3', py_pipeline, '--input', tmp_path, '--vocab-out', 'data/vocab/generated_vocab.jsonl']
        else:
            cmd = [cli_path, 'train', tmp_path]
        proc = subprocess.run(cmd, check=False, capture_output=True, text=True, timeout=timeout)
        rc = proc.returncode
        out = proc.stdout
        err = proc.stderr
    except subprocess.TimeoutExpired as e:
        return (124, '', f'timeout: {e}', tmp_path)
    except FileNotFoundError:
        return (127, '', f'CLI binary not found: {cli_path}', tmp_path)
    except Exception as e:
        return (1, '', str(e), tmp_path)
    finally:
        if not keep_temp and os.path.isdir(td):
            try:
                shutil.rmtree(td)
            except Exception:
                pass
    return (rc, out, err, tmp_path)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--out', help='Output JSONL path', default='data/corpus/scraped.jsonl')
    parser.add_argument('--url', help='Single URL to scrape')
    parser.add_argument('--local', help='Read local file instead of HTTP')
    parser.add_argument('--prompt', help='Prompt to generate after training')
    parser.add_argument('--no-train', action='store_true', help='Do not run training step')
    parser.add_argument('--dedup', action='store_true', help='Skip duplicate documents by content hash')
    parser.add_argument('--manifest', help='Write a run manifest JSON path', default=None)
    parser.add_argument('--sources', help='JSON file of sources to iterate (overrides builtins)', default=None)
    args = parser.parse_args()

    out = args.out
    entries = []
    idx = 0
    seen_hashes = set()
    manifest = {
        'run_time': int(time.time()),
        'out': out,
        'sources': [],
        'written': 0,
    }

    if args.local:
        try:
            local_entries = read_local_entries(args.local)
            if not local_entries:
                raise Exception('no entries parsed from local file')
            import hashlib
            for i, le in enumerate(local_entries):
                txt = le.get('text', '')
                norm = ' '.join(txt.split()) if txt else ''
                h = hashlib.sha256(norm.encode('utf-8')).hexdigest()
                if args.dedup and h in seen_hashes:
                    print('Skipping duplicate local entry:', le.get('id'))
                    continue
                seen_hashes.add(h)
                doc = {"id": le.get('id') or f"local-{i}", "text": norm, "source": os.path.basename(args.local), "hash": h, "scraped_at": int(time.time())}
                entries.append(doc)
        except Exception as e:
            print('Failed to read local file:', e, file=sys.stderr)
            sys.exit(2)
    elif args.url:
        try:
            if 'wikipedia.org' in args.url:
                p = urlparse(args.url)
                title = p.path.rsplit('/', 1)[-1]
                text = fetch_wikipedia(title)
            elif 'reddit.com' in args.url:
                text = fetch_reddit(args.url)
            else:
                text = fetch_generic(args.url)
            entries.append({"id": f"url-0", "text": text})
        except Exception as e:
            print('Failed to fetch URL:', e, file=sys.stderr)
            sys.exit(3)
    else:
        sources_iter = DEFAULT_SOURCES
        if args.sources:
            try:
                with open(args.sources, 'r', encoding='utf-8') as sf:
                    sources_iter = json.load(sf)
            except Exception as e:
                print('Failed to read sources file, falling back to defaults:', e, file=sys.stderr)
                sources_iter = DEFAULT_SOURCES

        for s in sources_iter:
            idx += 1
            name = s.get('name') or s.get('url') or f'source-{idx}'
            print('Processing', name)
            try:
                if s.get('type') == 'wikipedia':
                    text = fetch_wikipedia(s.get('id'))
                elif s.get('type') == 'reddit':
                    text = fetch_reddit(s.get('url'))
                else:
                    text = fetch_generic(s.get('url'))

                # Simple normalization: trim and collapse whitespace
                if text:
                    norm = ' '.join(text.split())
                else:
                    norm = ''

                # Dedup by sha256 of normalized text when requested
                import hashlib
                h = hashlib.sha256(norm.encode('utf-8')).hexdigest()
                if args.dedup:
                    if h in seen_hashes:
                        print('Skipping duplicate:', name)
                        continue
                    seen_hashes.add(h)

                doc = {"id": f"{name}-{idx}", "text": norm, "source": name, "hash": h, "scraped_at": int(time.time())}
                entries.append(doc)
                manifest['sources'].append({'name': name, 'id': s.get('id'), 'url': s.get('url'), 'written': True})
            except Exception as e:
                print(f'Warning: failed to fetch {name}: {e}', file=sys.stderr)
                manifest['sources'].append({'name': name, 'id': s.get('id'), 'url': s.get('url'), 'written': False, 'error': str(e)})
                continue

    if not entries:
        print('No entries collected; aborting', file=sys.stderr)
        sys.exit(4)

    write_jsonl(out, entries)
    print(f'Wrote {len(entries)} entries to {out}')
    manifest['written'] = len(entries)
    if args.manifest:
        try:
            with open(args.manifest, 'w', encoding='utf-8') as mf:
                json.dump(manifest, mf, ensure_ascii=False, indent=2)
            print('Wrote manifest to', args.manifest)
        except Exception as e:
            print('Failed to write manifest:', e, file=sys.stderr)

    if not args.no_train:
        print('Running training (via temp JSONL) using ./bin/mage_cli')
        rc, out_txt, err_txt, tmp_path = train_entries(entries, cli_path='./bin/mage_cli')
        if out_txt:
            print(out_txt)
        if rc != 0:
            if err_txt:
                print('Training failed:', err_txt, file=sys.stderr)
            else:
                print('Training failed with code', rc, file=sys.stderr)
            sys.exit(5)

    if args.prompt:
        gen_cmd = ['./bin/mage_cli'] + args.prompt.split()
        print('Generating reply for prompt:', args.prompt)
        try:
            rc = subprocess.run(gen_cmd, check=False, capture_output=True, text=True)
            out_text = rc.stdout.strip()
            if rc.returncode != 0:
                print('Generation failed:', rc.stderr, file=sys.stderr)
                sys.exit(7)
        except FileNotFoundError:
            print('CLI binary missing; build first', file=sys.stderr)
            sys.exit(8)

        coh_cmd = ['python3', 'python/coherence_checker.py']
        try:
            coh = subprocess.run(coh_cmd, input=out_text, check=False, capture_output=True, text=True)
            final = coh.stdout.strip() if coh.stdout else out_text
            print('\n--- Final (coherence-checked) reply ---')
            print(final)
            sys.exit(coh.returncode)
        except Exception as e:
            print('Coherence checker failed, returning raw output:', e, file=sys.stderr)
            print(out_text)
            sys.exit(0)

    print('Done')


if __name__ == '__main__':
    main()
