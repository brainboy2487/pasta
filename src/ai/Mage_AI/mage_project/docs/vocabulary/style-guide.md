# Vocabulary Style Guide

Purpose
- Ensure consistent tone, formatting, and safety across vocabulary entries so runtime behavior is predictable and maintainable.

Tone & voice
- Use short, direct sentences. Keep responses concise (recommended <= 120 characters per response line).
- For jokes/facts use neutral, generally inoffensive content; avoid political, religious, or explicit material.

Formatting rules
- `key` values: store normalized trigger text (lowercase, alphanumeric + spaces). Example: `"how are you"`.
- `aliases`: include common short forms and punctuation-free versions (e.g., `"hi"`, `"hello"`).
- `responses`: plain UTF-8 text, no HTML or markdown. Use plain punctuation and avoid line breaks.
- When variable substitution is needed, use a clear placeholder form `%{name}` and document allowed placeholders.

Normalization
- Trim and collapse whitespace. Convert to NFC unicode normalization.
- Lowercase keys and aliases for matching. Preserve case in `responses` where appropriate.

Length and size limits
- `key`: recommended <= 64 characters.
- `responses` entries: recommended <= 256 characters.
- `aliases` per entry: recommended <= 10.

Safety and content policies
- Avoid profanity and hateful content. If an entry references people/groups, keep language neutral.
- For third-party content include `source` and `license` and ensure licensing permits redistribution.

Metadata and tags
- Use `tags` for grouping (e.g., `greeting`, `smalltalk`, `joke.family`). Keep tag names lowercase and hyphenated.

Examples
- Greeting entry example:
  {
    "id": "greeting-basic-1",
    "key": "hello",
    "aliases": ["hi", "hey"],
    "type": "greeting",
    "responses": ["Hello!", "Hey — how can I help?"]
  }

Review checklist (before merging)
- Valid JSONL and passes `tools/validate_vocab.py`.
- Includes `id`, `key`, `type`, and `responses`.
- No disallowed content; third-party content includes attribution.
