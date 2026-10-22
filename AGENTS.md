# AGENTS.md

yuanme is a Rust static site generator for programmers' and mathematicians' personal sites, rendering both Markdown and Org, and built to be driven by coding agents.

**Status: building v1.** The plan is done: its decisions live in the closed tickets of the wayfinder map (the issue labelled `wayfinder:map`). The build is tracked by [Build yuanme v1](https://github.com/yuann3/yuanme/issues/41): take an open, unblocked `build` ticket, and land it with commits pushed straight to `main`. Don't open a PR, and close the ticket once its work is on `main`.

## Before you start

- `CODE_STANDARD.md` is how code is written here. CI runs `cargo clippy --all-targets --locked -- -D warnings` and `cargo nextest run --locked`.
- `CONTEXT.md` is the glossary. Use its terms, and avoid the words it lists under _Avoid_.
- `docs/adr/` records the decisions that shape the Engine. Don't contradict one silently; flag it.
- `docs/research/` holds research notes that tickets link to.

## Agent skills

### Issue tracker

Issues and the wayfinder map live in GitHub Issues on `yuann3/yuanme`, worked with the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Domain docs

Single-context: one `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
