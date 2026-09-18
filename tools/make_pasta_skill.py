#!/usr/bin/env python3
"""
tools/make_pasta_skill.py
Extracts docs/README.txt and produces pasta_skill.txt (AI-readable skill file).
"""
import sys, re, textwrap, pathlib, datetime

SRC = pathlib.Path("docs/README.txt")
OUT = pathlib.Path("pasta_skill.txt")

if not SRC.exists():
    print(f"Error: {SRC} not found. Place your README at docs/README.txt and re-run.", file=sys.stderr)
    sys.exit(1)

text = SRC.read_text(encoding="utf-8")

# Heuristics to split README into sections by common headings
sections = []
current_title = "Introduction"
current_lines = []
for line in text.splitlines():
    m = re.match(r'^\s{0,3}(#{1,6}\s+|[A-Z][A-Z0-9 _-]{3,}\s*\n?-{3,}\s*$)', line)
    if m and current_lines:
        sections.append((current_title, "\n".join(current_lines).strip()))
        current_title = line.strip().strip('# ').strip()
        current_lines = []
    else:
        current_lines.append(line)
if current_lines:
    sections.append((current_title, "\n".join(current_lines).strip()))

# Short summarizer (lightweight): pick first paragraph and first code block if any
def summarize_block(s):
    s = s.strip()
    if not s:
        return ""
    # first paragraph
    para = next((p for p in s.split("\n\n") if p.strip()), "")
    # first code fence or indented block
    code = ""
    m = re.search(r'```(?:\w+)?\n(.*?)\n```', s, re.S)
    if m:
        code = m.group(1).strip()
    else:
        m2 = re.search(r'(^\s{4,}.*(?:\n\s{4,}.*)*)', s, re.M)
        if m2:
            code = "\n".join([ln[4:] for ln in m2.group(1).splitlines()])
    return para.strip(), code.strip()

now = datetime.datetime.utcnow().isoformat() + "Z"

# Build skill file content
out_lines = []
out_lines.append("NAME: pasta_language_skill")
out_lines.append("VERSION: 1")
out_lines.append(f"COLLECTED_ON: {now}")
out_lines.append("SOURCE: docs/README.txt")
out_lines.append("")
out_lines.append("META:")
out_lines.append("  format: ai_skill_v1")
out_lines.append("  description: |")
out_lines.append("    Machine-friendly summary of the Pasta language, runtime, and")
out_lines.append("    development tasks extracted from docs/README.txt.")
out_lines.append("")
out_lines.append("SECTIONS:")
for title, body in sections:
    para, code = summarize_block(body)
    out_lines.append(f"  - title: \"{title.replace('\"','\\\"')}\"")
    out_lines.append("    summary: |")
    for l in textwrap.wrap(para or "(no summary available)", width=80):
        out_lines.append("      " + l)
    if code:
        out_lines.append("    example_code: |")
        for l in code.splitlines():
            out_lines.append("      " + l)
    out_lines.append("")

# Actionable items: try to extract TODO or Checklist blocks
todos = re.findall(r'(?mi)^(?:-|\*|\d+\.)\s+(TODO|FIXME|TODO:|Checklist|Step|Task).*$|(?s)###\s*TODO(.*?)(?:\n###|\Z)', text)
# fallback: search for lines with "TODO" or "FIXME"
todo_lines = [ln for ln in text.splitlines() if "TODO" in ln or "FIXME" in ln or "Checklist" in ln]
out_lines.append("ACTIONABLE:")
if todo_lines:
    out_lines.append("  - extracted_items:")
    for ln in todo_lines[:40]:
        out_lines.append("    - \"" + ln.strip().replace('"','\\"') + "\"")
else:
    out_lines.append("  - extracted_items: []")
out_lines.append("")
out_lines.append("PATCH_GUIDANCE: |")
out_lines.append("  Use the 'SECTIONS' entries to locate relevant code and documentation.")
out_lines.append("  For interpreter changes, prefer using ex_frame helpers (push_scope, set_local, pop_scope).")
out_lines.append("")
out_lines.append("USAGE_EXAMPLE: |")
out_lines.append("  - Load this file into an agent as a skill resource.")
out_lines.append("  - Query for 'bind function parameters' or 'graphics builtins' and the agent")
out_lines.append("    should reference the relevant 'SECTIONS' entry and 'PATCH_GUIDANCE'.")
out_lines.append("")
OUT.write_text("\n".join(out_lines), encoding="utf-8")
print("Wrote", OUT)
