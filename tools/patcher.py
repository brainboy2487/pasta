#!/usr/bin/env python3
"""
tools/patcher.py

Usage:
  # Apply patches (patch modules live in tools/patches/<name>.py)
  python3 tools/patcher.py patch1 patch2 -- files <file1> <file2> ...

  # Revert a file to its most recent backup
  python3 tools/patcher.py -revert <file> --recent

Notes:
- Each patch module may define either:
    - OPS: a list of operation dicts (see supported ops below), or
    - apply(text: str) -> str and optional revert(text: str) -> str functions.
- The tool always creates a timestamped backup: <file>.bak_<YYYYmmdd_HHMMSS>
- A JSON log of applied patches is kept at artifacts/patch_log.json
"""
from __future__ import annotations
import argparse
import importlib.util
import json
import os
import re
import shutil
import sys
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional

ROOT = Path('.').resolve()
PATCH_DIR = ROOT / 'tools' / 'patches'
ARTIFACTS = ROOT / 'artifacts'
LOG_PATH = ARTIFACTS / 'patch_log.json'
TS_FMT = "%Y%m%d_%H%M%S"

ensure_dirs = [PATCH_DIR, ARTIFACTS]
for d in ensure_dirs:
    d.mkdir(parents=True, exist_ok=True)

def timestamp() -> str:
    return datetime.utcnow().strftime(TS_FMT)

def backup_file(path: Path) -> Path:
    ts = timestamp()
    bak = path.with_name(path.name + f".bak_{ts}")
    shutil.copy2(path, bak)
    return bak

def find_patch_module(name: str) -> Path:
    p = PATCH_DIR / f"{name}.py"
    if not p.exists():
        raise FileNotFoundError(f"Patch module not found: {p}")
    return p

def load_patch_module(path: Path):
    spec = importlib.util.spec_from_file_location(path.stem, str(path))
    if spec is None or spec.loader is None:
        raise ImportError(f"Cannot load patch module {path}")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod

def apply_ops(text: str, ops: List[Dict[str, Any]], filename: str) -> str:
    """
    Supported op types:
      - replace: { "type":"replace", "old": "...", "new": "...", "count": int (optional), "regex": bool (optional) }
      - insert_after: { "type":"insert_after", "anchor": "...", "content": "...", "first_only": bool (default True) }
      - insert_before: { "type":"insert_before", "anchor": "...", "content": "...", "first_only": bool (default True) }
      - append: { "type":"append", "content": "..." }
      - prepend: { "type":"prepend", "content": "..." }
      - ensure_contains: { "type":"ensure_contains", "needle": "...", "where":"append"|"prepend"|"after:<anchor>" }
    """
    out = text
    for op in ops:
        typ = op.get("type")
        if typ == "replace":
            old = op["old"]
            new = op["new"]
            count = op.get("count", 0)
            regex = op.get("regex", False)
            if regex:
                out, n = re.subn(old, new, out, count=count) if count else (re.sub(old, new, out), None)
            else:
                out = out.replace(old, new, count) if count else out.replace(old, new)
        elif typ in ("insert_after", "insert_before"):
            anchor = op["anchor"]
            content = op["content"]
            first_only = op.get("first_only", True)
            if anchor not in out:
                raise ValueError(f"Anchor not found in {filename}: {anchor[:80]!r}")
            if typ == "insert_after":
                if first_only:
                    out = out.replace(anchor, anchor + content, 1)
                else:
                    out = out.replace(anchor, anchor + content)
            else:
                if first_only:
                    out = out.replace(anchor, content + anchor, 1)
                else:
                    out = out.replace(anchor, content + anchor)
        elif typ == "append":
            out = out + op["content"]
        elif typ == "prepend":
            out = op["content"] + out
        elif typ == "ensure_contains":
            needle = op["needle"]
            if needle in out:
                continue
            where = op.get("where", "append")
            if where == "append":
                out = out + op.get("content", needle)
            elif where == "prepend":
                out = op.get("content", needle) + out
            elif where.startswith("after:"):
                anchor = where.split(":", 1)[1]
                if anchor not in out:
                    raise ValueError(f"Anchor for ensure_contains not found: {anchor}")
                out = out.replace(anchor, anchor + op.get("content", needle), 1)
            else:
                raise ValueError(f"Unknown where for ensure_contains: {where}")
        else:
            raise ValueError(f"Unsupported op type: {typ}")
    return out

def apply_patch_module_to_file(patch_mod, file_path: Path) -> Dict[str, Any]:
    """
    Returns a dict with metadata about the operation for logging.
    """
    original = file_path.read_text(encoding='utf-8')
    # create backup
    bak = backup_file(file_path)
    # apply
    if hasattr(patch_mod, "apply") and callable(patch_mod.apply):
        new_text = patch_mod.apply(original)
    elif hasattr(patch_mod, "OPS"):
        ops = getattr(patch_mod, "OPS")
        new_text = apply_ops(original, ops, str(file_path))
    else:
        raise ValueError(f"Patch module {patch_mod.__name__} has no apply() or OPS")
    if new_text == original:
        # no-op; still keep backup but note it
        return {"file": str(file_path), "backup": str(bak), "changed": False}
    file_path.write_text(new_text, encoding='utf-8')
    return {"file": str(file_path), "backup": str(bak), "changed": True}

def revert_file_to_recent_backup(file_path: Path) -> Optional[Path]:
    """
    Find the most recent backup matching file_path.name.bak_* and restore it.
    Returns the restored backup path or None if not found.
    """
    parent = file_path.parent
    pattern = f"{file_path.name}.bak_*"
    candidates = list(parent.glob(pattern))
    if not candidates:
        # also search artifacts for backups
        candidates = list(ARTIFACTS.glob(f"{file_path.name}.bak_*"))
    if not candidates:
        return None
    candidates.sort(key=lambda p: p.stat().st_mtime, reverse=True)
    chosen = candidates[0]
    shutil.copy2(chosen, file_path)
    return chosen

def log_patch_entry(entry: Dict[str, Any]):
    log = {}
    if LOG_PATH.exists():
        try:
            log = json.loads(LOG_PATH.read_text(encoding='utf-8'))
        except Exception:
            log = {}
    now = datetime.utcnow().isoformat() + "Z"
    log.setdefault("entries", []).append({"timestamp": now, **entry})
    LOG_PATH.write_text(json.dumps(log, indent=2), encoding='utf-8')

def main(argv: List[str]):
    parser = argparse.ArgumentParser(description="Apply or revert simple Python patch modules.")
    parser.add_argument("patches", nargs="*", help="Patch module names (without .py) to apply. If -revert is used, this is ignored.")
    parser.add_argument("--", dest="dash", action="store_true", help=argparse.SUPPRESS)
    parser.add_argument("files", nargs="*", help="Files to patch (pass after --).")
    parser.add_argument("-revert", action="store_true", help="Revert the given file(s) to most recent backup.")
    parser.add_argument("--recent", action="store_true", help="When reverting, restore the most recent backup without prompting.")
    args = parser.parse_args(argv)

    # If -revert mode
    if args.revert:
        if not args.files:
            print("No files specified to revert.", file=sys.stderr)
            sys.exit(2)
        for f in args.files:
            fp = Path(f)
            if not fp.exists():
                print(f"Target file not found: {fp}", file=sys.stderr)
                continue
            restored = revert_file_to_recent_backup(fp) if args.recent else revert_file_to_recent_backup(fp)
            if restored:
                print(f"Restored {fp} from {restored}")
                log_patch_entry({"action": "revert", "file": str(fp), "restored_from": str(restored)})
            else:
                print(f"No backup found to restore for {fp}", file=sys.stderr)
        return

    # Normal apply mode
    if not args.patches:
        print("No patch modules specified.", file=sys.stderr)
        parser.print_help()
        sys.exit(2)
    if not args.files:
        print("No target files specified. Use -- files <file1> <file2> ...", file=sys.stderr)
        parser.print_help()
        sys.exit(2)

    # Load patch modules
    patch_modules = []
    for pname in args.patches:
        ppath = find_patch_module(pname)
        mod = load_patch_module(ppath)
        patch_modules.append((pname, mod))

    # Apply each patch to each file
    for fname in args.files:
        fpath = Path(fname)
        if not fpath.exists():
            print(f"Target file not found: {fpath}", file=sys.stderr)
            continue
        for pname, mod in patch_modules:
            try:
                meta = apply_patch_module_to_file(mod, fpath)
                print(f"Applied patch {pname} -> {fpath} (changed={meta['changed']})")
                log_patch_entry({"action": "apply", "patch": pname, "file": str(fpath), "backup": meta["backup"], "changed": meta["changed"]})
            except Exception as e:
                print(f"ERROR applying patch {pname} to {fpath}: {e}", file=sys.stderr)
                # leave file as-is (backup exists)
                log_patch_entry({"action": "error", "patch": pname, "file": str(fpath), "error": str(e)})
                # continue to next patch/file
    print("Done.")

if __name__ == "__main__":
    main(sys.argv[1:])
