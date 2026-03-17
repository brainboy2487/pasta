Contributing to Pasta — guidelines and process

Thank you for helping build Pasta. This document explains how contributors can get started, how contributions are reviewed and merged, and how the project is governed. It also lists practical onboarding steps, coding standards, testing expectations, and community norms so contributions are fast, safe, and high quality.

---

Quick start (developer setup)
- Clone the repo — fork the repository, then git clone your fork and add the upstream remote.  
- Install toolchain — install Rust (stable toolchain), Python 3.10+ for scripts, and system libraries required by the project (X11 dev headers on Linux).  
- Build locally — run cargo build at the repository root; run cargo test to verify baseline tests.  
- Run smoke examples — use the provided examples (e.g., examples/mandelbrot.ps) in the pasta shell to confirm the runtime and graphics stack work on your machine.  
- Create a feature branch — use git checkout -b feat/short-description for new work.

---

How to contribute code (workflow)
- Issue first — open an issue describing the problem or feature before starting major work. Include motivation, acceptance criteria, and any design notes.  
- Design discussion — for non-trivial changes (language features, runtime changes, async), post a short design RFC in the issue or a dedicated design/ doc and get maintainers’ feedback.  
- Small, focused PRs — prefer many small PRs over one large PR. Each PR should implement a single logical change and include tests.  
- Branch naming — use feat/, fix/, chore/, refactor/, or docs/ prefixes.  
- Commit messages — use imperative style and reference the issue number (e.g., parser: support pipeline token | (fix #123)).  
- Pull request checklist — include a description, link to the issue, test plan, and any migration notes. Tag reviewers and add relevant labels.

---

Code review and merging
- Review policy — at least one maintainer review required for non-trivial changes; two approvals for core runtime or API changes.  
- Automated checks — PRs must pass CI (lint, unit tests, integration tests, and examples). Fix CI failures before requesting final review.  
- Squash or rebase — maintainers may request a rebase or a squash merge to keep history tidy.  
- Breaking changes — require an RFC and a migration guide; mark the PR with breaking-change and coordinate a release.

---

Testing expectations
- Unit tests — every new function or module should include unit tests covering edge cases.  
- Integration tests — language features and runtime changes must include integration tests that run in the pasta test harness.  
- Graphics tests — include deterministic image checksums for rendering tests (compare PPM checksums).  
- Performance tests — for performance-sensitive changes, add benchmark scripts and document expected improvements or regressions.  
- CI matrix — tests should run on Linux and macOS; include an X11 headless smoke test for graphics where possible.

---

Coding standards and style
- Rust style — follow rustfmt and clippy rules. Add #[allow(...)] only with a comment explaining why.  
- API stability — prefer small, explicit public APIs. Document public types and functions with /// comments.  
- Error handling — use typed error enums; avoid unwrap() in library code. Surface helpful error messages for users.  
- Documentation — every public module must have a short module-level doc and examples where applicable. Update CHANGELOG.md for user-visible changes.

---

Pipeline module contribution specifics
- Design doc required — any change to src/pipelines/* must include a short design note describing channel semantics, backpressure, and thread-safety.  
- Safety tests — add tests that validate ordering, backpressure, and canvas safety (PIXEL_BATCH behavior).  
- Integration with pasta_async — changes that touch |&| must include thread-safety reasoning and a small concurrency test.  
- Shell integration — ensure shell command stages behave like native stages and include tests for subprocess I/O mapping.

---

Issue triage and labels
- Labels to use — good first issue, help wanted, design, bug, performance, docs, security.  
- Good first issues — small, well-scoped tasks with clear acceptance criteria and pointers to relevant code. Mark them good first issue.  
- Security issues — mark as security and follow the private disclosure process (see Security section).

---

Communication and community norms
- Code of Conduct — all contributors must follow the project Code of Conduct. Be respectful, constructive, and inclusive.  
- Discussion channels — use GitHub issues for design and bug discussion; use a dedicated chat (Discord/Matrix/Slack) for real-time coordination if enabled. Keep design decisions in issues or design/ docs.  
- Mentorship — maintainers will label and triage beginner issues and offer guidance; contributors are encouraged to ask for help on small PRs.

---

Security and responsible disclosure
- Private reporting — report security vulnerabilities privately to the maintainers via the repository’s security contact. Do not open public issues for vulnerabilities.  
- Response timeline — maintainers will acknowledge reports within 72 hours and provide a remediation plan.  
- Patch process — security fixes will be coordinated and released with a CVE-style advisory if appropriate.

---

Documentation and examples
- Docs site — keep the language reference, runtime API, and pipeline docs up to date. Add examples for |, ||, |&|, and |:|.  
- Examples folder — include runnable examples (examples/mandelbrotpipe.ps, examples/pipelineshell.ps) with README notes on expected output and runtime flags.  
- Tutorials — add a short tutorial showing how to convert an existing script into a pipeline and how to use PIXEL_BATCH safely.

---

Release process and versioning
- Semantic versioning — follow semver for public API changes. Patch releases for bug fixes, minor for new features, major for breaking changes.  
- Release checklist — update CHANGELOG.md, run full CI, tag the release, and publish release notes with migration guidance.  
- Nightly builds — provide CI artifacts for nightly builds and a stable channel for releases.

---

Recognition and contributor experience
- Contributor guide — maintain a short CONTRIBUTORS.md listing maintainers, review rotation, and how to get commit access.  
- Attribution — add contributors to AUTHORS.md and include notable contributions in release notes.  
- Onboarding tasks — create a set of good first issue tasks and a short onboarding checklist for new contributors.

---

Governance and decision-making
- Maintainer model — core maintainers approve merges and manage releases; decisions on major design changes require an RFC and majority approval from maintainers.  
- RFC process — submit RFCs to design/ with motivation, alternatives, and migration plan. Allow a two-week comment period before finalizing.

---

Promotion and attracting contributors (practical next steps)
- Polish README and landing page — add architecture diagrams, demo GIFs, and a short “Why Pasta” elevator pitch.  
- Create starter issues — prepare 10–20 good first issue tasks with clear steps and links to relevant code.  
- Onboarding docs — produce a short video or written walkthrough showing local setup, running tests, and submitting a PR.  
- Community outreach — announce on relevant forums (Rust, compilers, language design, systems), post a short technical blog post about the pipeline design, and present at local meetups or conferences.  
- Hackathon / sprint — organize a weekend contributor sprint with a small set of goals (pipeline | prototype, tests, docs).  
- Badges and CI — add CI status, codecov, and contribution guidelines badges to the README to lower friction for new contributors.

---

Templates and automation to add now
- Issue templates — bug report, feature request, design RFC, security report.  
- PR template — checklist for tests, docs, changelog, and migration notes.  
- CODEOWNERS — assign owners for src/pipelines/, src/runtime/, and docs.  
- GitHub Actions — CI for lint, unit tests, integration tests, and example smoke runs.  
- Dependabot — enable for dependency updates.

---

Maintenance and long-term health
- Regular triage — schedule weekly triage to keep issues fresh and label stale issues.  
- Technical debt sprints — dedicate periodic sprints to refactor hotspots (e.g., channel performance, async integration).  
- Benchmarks — maintain a benchmark suite and publish performance numbers for major changes.
