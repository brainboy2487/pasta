#!/usr/bin/env python3
"""
generate_project_tree.py

Scan the current working directory and its descendants only, writing a clean
ASCII tree to a text file. Hidden files and common VCS/build dirs are excluded
by default.

Usage (run from the directory you want scanned):
  python3 generate_project_tree.py
  python3 generate_project_tree.py -o project_tree.txt --sizes -d 3 -e docs -e '*.lock'

Options:
  -o, --output     Output filename (default: project_tree.txt)
  -d, --max-depth  Max recursion depth (0 = only root). Default: unlimited.
  --no-hidden      Include hidden files/dirs (default: hidden are excluded).
  --sizes          Show file sizes in human-readable form.
  -e, --exclude    Glob pattern to exclude (repeatable).
  --format         'tree' (default) or 'list' (one path per line).
"""
from pathlib import Path
import os
import argparse
import fnmatch
import stat

DEFAULT_EXCLUDES = [
    ".git", ".github", ".gitignore", ".gitattributes", ".gitmodules",
    ".DS_Store", ".vscode", ".idea", "node_modules", "target", "build",
    ".venv", "venv", ".env", ".pytest_cache", "__pycache__"
]

def human_size(n):
    for unit in ('B','KB','MB','GB','TB'):
        if n < 1024.0:
            return f"{n:.0f}{unit}"
        n /= 1024.0
    return f"{n:.0f}PB"

def matches_any(name, patterns):
    for pat in patterns:
        if fnmatch.fnmatch(name, pat):
            return True
    return False

def build_tree(root: Path, max_depth=None, include_hidden=False, show_sizes=False, extra_excludes=None, out_format='tree'):
    if extra_excludes is None:
        extra_excludes = []
    # combine default excludes and user-specified patterns
    excludes = list(DEFAULT_EXCLUDES) + extra_excludes

    root = root.resolve()
    lines = []

    if out_format == 'list':
        for dirpath, dirnames, filenames in os.walk(root):
            # compute depth relative to root
            rel = Path(dirpath).relative_to(root)
            depth = 0 if rel == Path('.') else len(rel.parts)
            if max_depth is not None and depth > max_depth:
                dirnames[:] = []
                continue
            # prune directories we should not descend into
            dirnames[:] = sorted([d for d in dirnames
                                  if (include_hidden or not d.startswith('.'))
                                  and not matches_any(d, excludes)])
            filenames = sorted([f for f in filenames
                                if (include_hidden or not f.startswith('.'))
                                and not matches_any(f, excludes)])
            for f in filenames:
                p = Path(dirpath) / f
                if show_sizes:
                    try:
                        size = human_size(p.stat().st_size)
                    except Exception:
                        size = "?"
                    lines.append(f"{p.relative_to(root)}\t{size}")
                else:
                    lines.append(str(p.relative_to(root)))
        return lines

    # tree format
    def walk(path: Path, prefix: str = "", depth: int = 0):
        if max_depth is not None and depth > max_depth:
            return
        try:
            entries = sorted([e for e in path.iterdir()
                              if (include_hidden or not e.name.startswith('.'))
                              and not matches_any(e.name, excludes)],
                             key=lambda p: (p.is_file(), p.name.lower()))
        except PermissionError:
            lines.append(prefix + "[permission denied] " + path.name)
            return
        for i, entry in enumerate(entries):
            connector = "└── " if i == len(entries)-1 else "├── "
            if entry.is_dir():
                lines.append(prefix + connector + entry.name + "/")
                new_prefix = prefix + ("    " if i == len(entries)-1 else "│   ")
                walk(entry, new_prefix, depth+1)
            else:
                if show_sizes:
                    try:
                        size = human_size(entry.stat().st_size)
                    except Exception:
                        size = "?"
                    lines.append(prefix + connector + f"{entry.name}  [{size}]")
                else:
                    lines.append(prefix + connector + entry.name)

    lines.append(f"{root.name}/")
    walk(root)
    return lines

def main():
    parser = argparse.ArgumentParser(description="Generate a project tree for the current directory (downward only).")
    parser.add_argument("-o", "--output", default="project_tree.txt", help="Output filename (relative to cwd)")
    parser.add_argument("-d", "--max-depth", type=int, default=None, help="Max recursion depth (0 = only root)")
    parser.add_argument("--no-hidden", dest="include_hidden", action="store_false", help="Include hidden files/dirs (default: hidden excluded)")
    parser.add_argument("--sizes", dest="sizes", action="store_true", help="Show file sizes")
    parser.add_argument("--format", choices=("tree","list"), default="tree", help="Output format")
    parser.add_argument("-e", "--exclude", action="append", default=[], help="Additional glob pattern to exclude (repeatable)")
    args = parser.parse_args()

    root = Path.cwd()
    lines = build_tree(root, max_depth=args.max_depth, include_hidden=args.include_hidden, show_sizes=args.sizes, extra_excludes=args.exclude, out_format=args.format)

    out_path = root / args.output
    out_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"Wrote {out_path} ({len(lines)} lines)")

if __name__ == "__main__":
    main()
