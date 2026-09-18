
from __future__ import annotations
import re
import json
import hashlib
from pathlib import Path

ROOT = Path('.').resolve()
ART = ROOT / 'artifacts'
IN = ART / 'CHEWME.txt'
CLEAN = ART / 'CHEWME_clean.txt'
SUMMARY = ART / 'CHEWME_summary.txt'
BITE_PREFIX = ART / 'EATMEBITE'
BITE_SIZE = 25 * 1024  # 25 KB
MAX_LINE_LEN = 2000

# Build ANSI escape regex using chr(27) to avoid literal backslash issues
ESC = chr(27)
ANSI_RE = re.compile(re.escape(ESC) + r' \[[0-?]*[ -/]*[@-~]')

REPEATED_PUNC_RE = re.compile(r'^[\s\-\=\*_]{3,}$')
WARNING_LINE_RE = re.compile(r'^\s*(warning:|note:|error:)\b', re.IGNORECASE)
COMMENT_ONLY_RE = re.compile(r'^\s*(//|#)\s?.*$')
TIMESTAMP_LINE_RE = re.compile(r'^\s*\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z\s*$')

def read_input(path: Path) -> str:
    if not path.exists():
        raise FileNotFoundError(f"Input file not found: {path}")
    return path.read_text(encoding='utf-8', errors='replace')

def remove_ansi(s: str) -> str:
    return ANSI_RE.sub('', s)

def process_lines(text: str):
    lines = text.splitlines()
    out_lines = []
    removed = {
        'warning_lines': 0,
        'repeated_punc': 0,
        'comment_only': 0,
        'timestamp_lines': 0,
        'truncated_lines': 0
    }
    warning_samples = []
    for ln in lines:
        ln_stripped = ln.rstrip('\n\r')
        ln_stripped = ANSI_RE.sub('', ln_stripped)
        if TIMESTAMP_LINE_RE.match(ln_stripped):
            removed['timestamp_lines'] += 1
            continue
        if REPEATED_PUNC_RE.match(ln_stripped):
            removed['repeated_punc'] += 1
            continue
        if WARNING_LINE_RE.match(ln_stripped):
            removed['warning_lines'] += 1
            if len(warning_samples) < 20:
                warning_samples.append(ln_stripped)
            continue
        if COMMENT_ONLY_RE.match(ln_stripped):
            removed['comment_only'] += 1
            continue
        m = re.match(r'^(\s*)(.*)$', ln_stripped)
        if m:
            indent, rest = m.groups()
            rest = re.sub(r'\s{3,}', ' ', rest)
            ln_stripped = indent + rest
        if len(ln_stripped) > MAX_LINE_LEN:
            ln_stripped = ln_stripped[:MAX_LINE_LEN] + '...<truncated>'
            removed['truncated_lines'] += 1
        out_lines.append(ln_stripped)
    collapsed = []
    blank_run = 0
    for l in out_lines:
        if l.strip() == '':
            blank_run += 1
            if blank_run <= 1:
                collapsed.append('')
        else:
            blank_run = 0
            collapsed.append(l)
    return '\n'.join(collapsed) + '\n', removed, warning_samples

def split_into_bites(clean_text: str, bite_size: int = BITE_SIZE):
    data = clean_text.encode('utf-8')
    total = len(data)
    if total == 0:
        return []
    bites = []
    start = 0
    idx = 1
    while start < total:
        end = min(start + bite_size, total)
        chunk = data[start:end]
        if end < total:
            last_nl = chunk.rfind(b'\n')
            if last_nl != -1 and last_nl > int(bite_size * 0.2):
                end = start + last_nl + 1
                chunk = data[start:end]
        bites.append((idx, chunk))
        idx += 1
        start = end
    return bites

def write_bites(bites, prefix: Path):
    written = []
    for idx, chunk in bites:
        fname = prefix.with_name(f"{prefix.name}{idx}.txt")
        fname.parent.mkdir(parents=True, exist_ok=True)
        fname.write_bytes(chunk)
        written.append(fname)
    return written

def sha256_bytes(b: bytes) -> str:
    h = hashlib.sha256()
    h.update(b)
    return h.hexdigest()

def main():
    try:
        raw = read_input(IN)
    except FileNotFoundError as e:
        print(e)
        return 1
    orig_size = len(raw.encode('utf-8'))
    cleaned = remove_ansi(raw)
    cleaned, removed, warning_samples = process_lines(cleaned)
    cleaned_size = len(cleaned.encode('utf-8'))
    ART.mkdir(parents=True, exist_ok=True)
    CLEAN.write_text(cleaned, encoding='utf-8')
    bites = split_into_bites(cleaned, BITE_SIZE)
    written = write_bites(bites, BITE_PREFIX)
    manifest = {
        "input": str(IN),
        "original_size": orig_size,
        "cleaned_size": cleaned_size,
        "bite_size_target": BITE_SIZE,
        "bites": []
    }
    offset = 0
    for idx, chunk in bites:
        fname = str(BITE_PREFIX.name + str(idx) + '.txt')
        size = len(chunk)
        checksum = sha256_bytes(chunk)
        manifest["bites"].append({
            "filename": fname,
            "index": idx,
            "offset": offset,
            "size": size,
            "sha256": checksum
        })
        offset += size
    summary_lines = []
    summary_lines.append("CHEWME preprocessing summary")
    summary_lines.append(f"Input: {IN}")
    summary_lines.append(f"Original size (bytes): {orig_size}")
    summary_lines.append(f"Cleaned size (bytes): {cleaned_size}")
    summary_lines.append(f"Bite size target (bytes): {BITE_SIZE}")
    summary_lines.append(f"Number of bites written: {len(written)}")
    summary_lines.append("")
    summary_lines.append("Removed counts:")
    for k, v in removed.items():
        summary_lines.append(f"- {k}: {v}")
    summary_lines.append("")
    if warning_samples:
        summary_lines.append("Sample removed warning/note/error lines (up to 20):")
        for s in warning_samples:
            summary_lines.append(f"  {s}")
    summary_lines.append("")
    summary_lines.append("Bite files:")
    for p in written:
        summary_lines.append(f"- {p} ({p.stat().st_size} bytes)")
    summary_lines.append("")
    summary_lines.append("Notes:")
    summary_lines.append("- This preprocessing removes noisy compiler warnings, comment-only lines, long separators, and timestamps.")
    SUMMARY.write_text('\n'.join(summary_lines), encoding='utf-8')
    manifest_path = ART / 'CHEWME_manifest.json'
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding='utf-8')
    print(f"Wrote cleaned file: {CLEAN}")
    print(f"Wrote summary: {SUMMARY}")
    print(f"Wrote manifest: {manifest_path}")
    print(f"Wrote {len(written)} bite files (prefix {BITE_PREFIX.name}N.txt)")
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
