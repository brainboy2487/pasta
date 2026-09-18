# Vocabulary Contribution Protocol

Purpose
- Provide a standardized, auditable pipeline for adding and updating entries (commands, greetings, jokes, facts, responses) in the Mage vocabulary database.

Overview
- All vocabulary entries are stored as newline-delimited JSON (JSONL) in `data/*.jsonl` files. Each line is a single JSON object describing one logical entry.
- Authors must follow the field requirements and normalization rules implemented in `data/vocab_schema.json`.

File layout and naming
- Use `data/vocab_*.jsonl` for content files (e.g., `data/vocab_greetings.jsonl`, `data/vocab_jokes.jsonl`).
- Keep files under 10k lines for ease of review; split large datasets into numbered chunks (`vocab_jokes_001.jsonl`).

Entry lifecycle
1. Draft: Add or update entries in a local JSONL file following the schema.
2. Validation: Run `tools/validate_vocab.py <file>` and fix reported issues.
3. Review: Submit a pull request with the JSONL files and the test output from the validator.
4. Merge: After approval, merge to `main`; CI should re-run validation on merged files.
5. Deploy: The runtime loader (CLI or library) picks up entries from `data/` at startup.

Required fields (see full schema in `data/vocab_schema.json`)
- `id` (string): stable canonical identifier (kebab-case). Immutable once published.
- `key` (string): primary trigger text used for fuzzy matching (lower-case normalized form recommended).
- `aliases` (array[string]): alternative triggers.
- `type` (string): one of `greeting`, `farewell`, `joke`, `fact`, `command`, `info`, `other`.
- `responses` (array[string]): allowed responses; prefer multiple alternatives for variety.
- `tags` (array[string], optional): categorization for filtering/analytics.
- `language` (string, optional): BCP47 language code (default `en`).
- `source` (string, optional): human-readable provenance (e.g., `curated/team`, `import/third-party`).
- `license` (string, optional): license short-name if content originated externally.
- `version` (integer, optional): entry version for non-breaking edits.

Validation and QA
- Run `tools/validate_vocab.py` to check: required fields present, types correct, `id` uniqueness, `key` normalization, response length limits, and prohibited content.
- Entries failing validation should not be merged.

Acceptance criteria for PRs
- All new/changed JSONL files pass `tools/validate_vocab.py`.
- PR includes a short changelog in the description describing the scope and sample prompts/expected responses.
- For large imports, include a sampling plan demonstrating quality (e.g., 100 random samples with manual checks).

Versioning and migration
- Entries use `version` to indicate non-backward-breaking edits (typos, formatting, minor rewrites). Breaking changes (format, intended meaning) require a new `id`.
- Backfill/migration scripts should be provided when changing keys or normalization rules.

Operational notes
- Sensitive or copyrighted content must include `source` and `license` fields and be reviewed by legal.
- Use `language` to separate localized content; do not mix languages within a single `responses` array.
