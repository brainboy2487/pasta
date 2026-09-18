#!/usr/bin/env python3
"""coherence_checker.py
Simple coherence/grammar checker using language_tool_python when available.

Usage:
  python3 python/coherence_checker.py --file path/to/file
  python3 python/coherence_checker.py --text "some reply"

The script prints a corrected version of the input to stdout. It exits
with code 0 when the checker judges the text coherent (few grammar issues),
1 when it judges the text incoherent, and 0 with unmodified text if the
checker library is not available.
"""
import argparse
import sys

parser = argparse.ArgumentParser()
parser.add_argument("--file", help="Path to file containing text")
parser.add_argument("--text", help="Text to check")
args = parser.parse_args()

if args.file:
    try:
        with open(args.file, "r", encoding="utf-8") as f:
            text = f.read()
    except Exception as e:
        print("", end="")
        sys.exit(1)
elif args.text:
    text = args.text
else:
    # read stdin
    text = sys.stdin.read()

if not text or text.strip() == "":
    print(text, end="")
    sys.exit(0)

# Attempt to import advanced parsing and grammar libraries.
# Prefer spaCy for sentence/token parsing and language_tool_python for grammar checks.
has_spacy = False
has_ltool = False
nlp = None
tool = None
try:
    import spacy
    try:
        nlp = spacy.load("en_core_web_sm")
    except Exception:
        # model not installed; fall back to a blank English pipeline
        nlp = spacy.blank("en")
    has_spacy = True
except Exception:
    has_spacy = False

try:
    import language_tool_python
    tool = language_tool_python.LanguageTool('en-US')
    has_ltool = True
except Exception:
    has_ltool = False

# If neither advanced library is available, echo the original text and succeed.
if not has_spacy and not has_ltool:
    print(text, end="")
    sys.exit(0)

# Run grammar check if available
matches = []
corrected = text
if has_ltool:
    try:
        matches = tool.check(text)
        corrected = tool.correct(text)
    except Exception:
        matches = []
        corrected = text

# Use spaCy for sentence segmentation and additional heuristics
sentences = []
if has_spacy and nlp is not None:
    doc = nlp(text)
    sentences = list(doc.sents)
    num_sentences = max(1, len(sentences))
else:
    num_sentences = max(1, text.count('.') + text.count('!') + text.count('?'))

# Map language-tool matches to sentence buckets (if both available)
issues_per_sentence = [0] * num_sentences
total_issues = len(matches) if matches else 0
if has_spacy and has_ltool and matches:
    for m in matches:
        off = getattr(m, 'offset', None)
        length = getattr(m, 'length', None)
        if off is None or length is None:
            continue
        # find sentence containing offset
        for i, s in enumerate(sentences):
            if s.start_char <= off < s.end_char:
                issues_per_sentence[i] += 1
                break

# Heuristic: decide incoherence based on issue density and sentence-level spikes
incoherent = False
if total_issues == 0:
    incoherent = False
else:
    avg_issues = float(total_issues) / float(num_sentences)
    if avg_issues > 1.5:
        incoherent = True
    if has_spacy:
        for i, s in enumerate(sentences):
            issue_count = issues_per_sentence[i] if i < len(issues_per_sentence) else 0
            if issue_count >= 4:
                incoherent = True
            # penalize very long sentences with multiple issues
            if len(s.text) > 200 and issue_count >= 2:
                incoherent = True
    else:
        # fallback rule similar to previous heuristic
        incoherent = total_issues > (2 * num_sentences + 2)

# Print corrected text when language tool is available, otherwise original
print(corrected, end="")
sys.exit(1 if incoherent else 0)
