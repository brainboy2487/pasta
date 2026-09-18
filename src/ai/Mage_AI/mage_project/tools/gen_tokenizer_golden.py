#!/usr/bin/env python3
import unicodedata
tests = [
    "Hello, world!",
    "Café",
    "—em dash — and – en dash",
    "Smart quotes: “Hello” ’there’",
    "Fullwidth: ＡＢＣ１２３",
    "Non\u00A0breaking\u00A0space",
    # Combining marks: precomposed vs decomposed
    "e\u0301 (e + combining acute)",
    "é (precomposed)",
    "A\u030A (A + ring) and Å",
    # Ligatures and compatibility forms
    "Office ﬁle (ligature)",
    # Rare/other scripts
    "Привет мир",               # Cyrillic
    "γειά σου κόσμε",          # Greek
    "नमस्ते दुनिया",           # Devanagari (Hindi)
    "שלום עולם",               # Hebrew
    "مرحبا بالعالم",           # Arabic
    # Emoji sequences: ZWJ, skin tones, flags
    "🙂👍🏽",
    "woman technologist: 👩\u200d💻",
    "family: 👨\u200d👩\u200d👧\u200d👦",
    "flag: 🇺🇳",
    # Zero-width and control characters
    "Hello\u200bWorld (zwb)",
    # Combining marks on non-Latin scripts
    "क\u093f (Devanagari combining)",
    # Miscellaneous symbols
    "¹²³ superscripts and ¼ fraction",
]

out_path = 'tests/golden/tokenizer_golden.txt'
import os
os.makedirs('tests/golden', exist_ok=True)
with open(out_path, 'w', encoding='utf-8') as f:
    for s in tests:
        norm = unicodedata.normalize('NFKC', s).casefold()
        f.write(s + "\n")
        f.write(norm + "\n")
        f.write("\n")
print('Wrote', out_path)
