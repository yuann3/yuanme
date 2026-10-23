# Org and math test corpus

Real-world Org and Markdown documents, collected to drive the Org subset decision and to serve as snapshot-test fixtures. The analysis is in [`docs/research/org-math-corpus.md`](../docs/research/org-math-corpus.md).

## Licences

**The files under `org/` and `md/` are not covered by this repository's licence.** Each file keeps its own licence, recorded in `manifest.json` along with its source URL (pinned to a commit) and the place where that licence is stated. Licences include GFDL-1.3-or-later, GPL-2.0/3.0, CC-BY-4.0, CC-BY-SA-4.0, CC0-1.0, MIT, Apache-2.0, MPL-2.0 and the Unlicense. Candidates without a redistributable licence were left out; `excluded.md` lists them.

The `md/eyuan-me/` files are the Author's own writing from the Reference site and are licensed MIT OR Apache-2.0 for this corpus. The `tools/` scripts are covered by the repository's licence.

## Layout

| Path | Contents |
|---|---|
| `org/<source>/` | 116 Org files from 21 sources |
| `md/<source>/` | 41 Markdown files from 14 sources |
| `manifest.json` | One entry per file: `path`, `format`, `source`, `source_url`, `licence`, `licence_evidence`, `math`, `notes` |
| `excluded.md` | Candidates rejected and why |
| `tools/org-features.el` | Counts Org constructs per file with Org's own parser (`org-element`) |
| `tools/aggregate.py` | Turns those counts, plus a math-syntax scan, into the frequency tables |

## Regenerating the frequency tables

Run this from `corpus/`. It needs Emacs with Org 9.8 or newer:

```sh
find org -name '*.org' -print0 | xargs -0 emacs --batch -l tools/org-features.el > /tmp/org-features.jsonl
python3 tools/aggregate.py . /tmp/org-features.jsonl
```
