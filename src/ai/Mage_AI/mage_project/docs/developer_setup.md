Developer Setup
=================

Prerequisites
-
- GCC (C compiler) supporting C11
- make
- python3
- pip
- Recommended: GLFW and OpenGL for GUI target
- Optional: Java (for language_tool_python), spaCy models for coherence checker

Python dependencies
-
Run:

```
python3 -m pip install -r python/requirements.txt
```

Building
-
From project root run:

```
make all
```

Notes
-
- The CLI (`bin/mage_cli`) expects to be run from the project root for relative data paths to resolve.
- `data/vocab/generated_vocab.jsonl` is created by the `train <file>` command.
