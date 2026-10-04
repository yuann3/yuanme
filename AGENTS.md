# AGENTS.md

yuanme is a Rust static site generator for programmers' and mathematicians' personal sites, rendering both Markdown and Org, and built to be driven by coding agents.

**Status: planning.** There is no code yet. The project is being charted as a wayfinder map on this repo's GitHub Issues (the issue labelled `wayfinder:map`). Read the map before starting work; decisions live in its closed tickets.

## Before you start

- `CONTEXT.md` is the glossary. Use its terms, and avoid the words it lists under _Avoid_.
- `docs/adr/` records the decisions that shape the Engine. Don't contradict one silently; flag it.
- `docs/research/` holds research notes that tickets link to.

## Agent skills

### Issue tracker

Issues and the wayfinder map live in GitHub Issues on `yuann3/yuanme`, worked with the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Domain docs

Single-context: one `CONTEXT.md` and `docs/adr/` at the repo root. See `docs/agents/domain.md`.
