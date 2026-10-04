# Which Org parser should the Engine use, or must we write our own?

Ticket: [#3](https://github.com/yuann3/yuanme/issues/3) (blocks [#16](https://github.com/yuann3/yuanme/issues/16) Org subset, [#17](https://github.com/yuann3/yuanme/issues/17) math and cross-references). Researched 2026-10-04.

## Short answer

**We should write our own Org parser, and test it against Emacs `org-element` as the oracle.** No Rust crate today is both faithful to Org syntax and maintained:

- **orgize** (the obvious default) is fast, lossless and exposes positions. But it has had no commit since 2024-07, it never left alpha, and it gets basic mathematician input wrong:
  - `$a$,` and `$b$.` are not maths.
  - A `#+NAME:`d equation or a `#+CAPTION:`ed table turns into a paragraph.
  - It has no citations, plain links or angle links.
- **organic** matches Emacs on 99.99% of 24,536 nodes. But its parser code has been frozen since 2024, it only builds on nightly, and it is about 50–100× slower than orgize (roughly 20 ms for a 16 KB post).
- **org-rust-parser** is the only maintained one. It is also the least faithful (82.6%): no citations, no planning lines or property drawers, `\begin{align*}` is not recognised, and it produces false-positive `$…$` maths.

The Engine has to own an Org parser whatever we pick, because the **Org subset** contract ("outside the subset is a build error, never silently dropped") means it must *recognise* every construct, with a position, before it can reject one. A parser that misreads `[cite:@x]` as plain text makes that contract impossible.

**Fallback.** If our own parser slips, use a vendored, stable-patched fork of organic. It is 0BSD, the spike made it build on stable Rust with a 3-error patch, and it is the best fidelity available today. Its test corpus (0BSD) and organic itself can serve as an in-Rust second oracle. Do **not** adopt orgize or org-rust-parser as the long-term parser.

## Comparison of the three finalists

The spike section below explains how this table was measured. ✓ means it matches Emacs Org 9.8.7 on the spike corpus, ◐ means partial or with bugs, ✗ means not parsed.

| | **orgize** 0.10.0-alpha.10 | **organic** 0.1.16 | **org-rust-parser** 0.1.8 |
|---|---|---|---|
| Headings (level, TODO, priority, tags) | ◐ `COMMENT` subtrees not flagged | ✓ | ◐ `COMMENT` not flagged |
| Planning lines (SCHEDULED/DEADLINE/CLOSED) | ✓ | ✓ | ✗ rendered as paragraph text |
| In-buffer `#+TODO:` keywords | ◐ ignored; caller must pre-scan and pass `ParseConfig` | ✓ (also follows `#+SETUPFILE`) | ✗ hard-coded `TODO`/`DONE` |
| Markup (`* / _ = ~ +`) | ◐ misses `"*quoted*"`; on the Org manual, code 1865/1876 and verbatim 3099/3116 | ✓ (1 code and 1 verbatim off in 24.5k nodes) | ◐ manual: code 1370/1876, verbatim 2616/3116 |
| `#+KEYWORDS` | ✓ | ✓ | ✓ |
| Affiliated keywords (`#+NAME`, `#+CAPTION`, `#+ATTR_*`) | ✗ before a table or LaTeX environment, the element becomes a **paragraph** | ✓ | ✓ |
| src / example / quote / verse / center / export / special / comment blocks | ✓ | ✓ | ✓ (fixed-width `:` lines ✗) |
| Links: regular, image, internal, id | ✓ | ✓ | ✓ |
| Links: plain `https://…`, angle `<…>`, radio | ✗ | ✓ | ◐ plain only (angle read as plain); no radio |
| Footnotes (ref, inline, anonymous, definition) | ◐ definition body not parsed | ✓ | ✓ |
| Tables | ◐ empty cells dropped ([#89](https://github.com/PoiScript/orgize/issues/89)); captioned table → paragraph | ✓ | ✓ |
| LaTeX fragments `$…$ \(…\) \[…\] $$…$$` | ✗ `$x$` followed by `, . ; -` not recognised | ✓ (one spec-versus-Emacs divergence, see below) | ◐ false positive on `$5 or $ 6$`; `x_{1}` missed |
| LaTeX environments | ◐ `#+NAME`d environment → paragraph | ✓ | ✗ `\begin{align*}` not recognised |
| Citations `[cite:@key]` | ✗ (and `to_html` drops the `@`) | ✓ citation + citation-reference | ✗ |
| `#+INCLUDE` | ✓ kept as a keyword (correct, see below) | ✓ keyword | ✓ keyword (its exporter expands it, and fails on `:lines "1-3"`) |
| Drawers, property drawers, node properties, clock | ✓ | ✓ | ◐ no property-drawer node; `CLOCK` not parsed |
| Macros `{{{x(y)}}}` and `#+MACRO` | ✓ (parsed, not expanded) | ✓ | ◐ manual: 751/1314 macros |
| Babel call, diary sexp, dynamic block, inlinetask | ◐ diary sexp → paragraph; dynamic block → 2 keywords | ◐ empty dynamic block → paragraph; no inlinetask | ✗ |
| **Node-count agreement with Emacs** (Org manual + Org syntax spec + Org guide, 24,536 nodes) | **88.3%** | **99.99%** | **82.6%** |
| Positions for diagnostics | ✓ byte `TextRange` on every node *and* token (rowan CST); lossless; `replace_range` edits | ◐ every node is a `&str` slice of the input, so a byte offset is a pointer difference; no offset API | ✓ `start`/`end` byte offsets per node |
| Parse time, 16 KB / 88 KB / 887 KB (M4 Max, median of 20) | 0.30 / 0.8–1.5 / 7.5–9.9 ms | 20 / 51–150 / 485–548 ms | 0.59 / 2.0–3.5 / 15–21 ms |
| Builds on stable Rust | ✓ | ✗ nightly manifest plus 7 `#![feature]`s (patch in spike: 3 errors) | ✓ MSRV 1.91 |
| Licence | MIT | 0BSD | MIT |
| Last release / last parser change | 2024-06-11 / 2024-07-22 | 2024-04-12 / 2025-02 (tests only; CI and nix since) | 2026-05-07 / 2026-08-16 |
| Reverse dependencies on crates.io | 7 | 1 (the author's own SSG) | 2 (its own CLI and exporter) |

The other crates were screened out; see "Crates screened out" below.

## Details

### Is there a spec to measure "fidelity" against?

The Org syntax document says it "describes and comments on Org syntax as it is currently read by its parser (`org-element.el`)". It also names `org-element.el` "the canonical parser" ([org-syntax.org, Introduction and Appendix](https://orgmode.org/worg/org-syntax.html)). So fidelity here means "the same tree as `org-element-parse-buffer`". The oracle was the Emacs 31.1 build on this machine, which bundles **Org 9.8.7**: `org-element.el` is 8,785 lines, and it defines 30 element types and 24 object types (`org-element-all-elements` and `org-element-all-objects`, printed with `emacs --batch`).

The spec text and Org 9.8.7 disagree in at least one place that matters to mathematicians. The spec says the POST character after `$CHAR$` may be "any punctuation character … a space character, or the end of line" ([LaTeX Fragments](https://orgmode.org/worg/org-syntax.html#LaTeX_Fragments)). Org 9.8.7, though, does **not** parse `$d$-th` as a fragment, because `-` is not punctuation in Org's syntax table. organic follows the spec text there. The Engine has to choose one; see the open questions.

### Three things the ticket lists are not parser features

- **`#+INCLUDE`** is a keyword in the syntax. It is expanded at export time by `org-export-expand-include-keyword` (`lisp/org/ox.el:3318` in Emacs 31.1). All three parsers correctly leave it as a keyword, so the Engine must expand it itself (`:lines`, `src`/`example` wrapping, `:minlevel`). Diagnostics then have to map positions back across files.
- **Macros** are parsed as `macro` objects. Expansion (`{{{title}}}`, `{{{date}}}`, user `#+MACRO:` with `$1`) is `org-macro-replace-all` (`lisp/org/org-macro.el:222`), and is also an Engine job.
- **Citations** parse as `citation`/`citation-reference` (`org-element-citation-parser`, `org-element.el:3439`). Processing `#+bibliography`, `#+cite_export` and `#+print_bibliography` belongs to `oc.el` (`org-cite-process-citations`, `oc.el:1386`). The parser only has to recognise the citation. Rendering it is a separate decision (#17).
- In-buffer TODO keywords, by contrast, **do** change parsing: `#+TODO: TODO WAIT | DONE CANCELED` decides whether `* WAIT x` has a keyword. So does `#+SETUPFILE`. A parser needs either a pre-scan (orgize's `ParseConfig.todo_keywords`, see [README](https://docs.rs/crate/orgize/0.10.0-alpha.10/source/README.md)) or to handle it internally (organic does, and it reads the setup file from disk through its `FileAccessInterface`).

### orgize ([crates.io](https://crates.io/crates/orgize), [repo](https://github.com/PoiScript/orgize))

- **What it is.** A rowan-based lossless CST: `DOCUMENT@0..18 / HEADLINE@0..18 …`. It has typed AST wrappers, a `traverse` event API, an HTML/Markdown exporter and `replace_range` for incremental edits ([README](https://docs.rs/crate/orgize/0.10.0-alpha.10/source/README.md)). The author switched to rowan because it fixed "not able to get position of each parsed element" ([#70 Announcing v0.10](https://github.com/PoiScript/orgize/issues/70)).
- **Health.**
  - Latest is `0.10.0-alpha.10` (2024-06-11). There has been no stable 0.10 in 28 months ([crates.io versions](https://crates.io/crates/orgize/versions)).
  - The last commit on the default `v0.10` branch is 2024-07-22 (`gh api repos/PoiScript/orgize/commits`). There are 17 open issues and PRs, including unmerged footnote-export ([#84](https://github.com/PoiScript/orgize/pull/84)), rowan 0.16 ([#85](https://github.com/PoiScript/orgize/pull/85)), nom 8 ([#90](https://github.com/PoiScript/orgize/pull/90)) and a 2026-05 link fix ([#94](https://github.com/PoiScript/orgize/pull/94)).
  - The author's own v0.10 announcement starts "After leaving this crate for almost unmaintained for over three years" ([#70](https://github.com/PoiScript/orgize/issues/70)). The current gap is the second long dormancy.
  - Of the 47 forks, those active in 2026 are 0–5 commits ahead and only bump rowan or nom (`gh api …/compare`, e.g. [gyger/orgize](https://github.com/gyger/orgize)). No successor exists.
  - 73k total downloads and 8.1k recent; 7 reverse dependencies ([crates.io](https://crates.io/crates/orgize/reverse_dependencies)).
- **Fidelity bugs reproduced in the spike (minimal inputs):**
  - `Let $a$, then $b$. And $c$ here` gives only `$c$` as `LATEX_FRAGMENT`. The cause is that the POST check only accepts `) } ] ' "`, space and newline ([src/syntax/latex_fragment.rs:116-121](https://docs.rs/crate/orgize/0.10.0-alpha.10/source/src/syntax/latex_fragment.rs)), which contradicts the spec's "any punctuation character".
  - `#+CAPTION: Results\n| a | b |` gives a `PARAGRAPH` (Emacs gives `table`). `#+NAME: eq1\n\begin{equation}…` gives a `PARAGRAPH` containing `LATEX_FRAGMENT \begin{equation}` (Emacs gives `latex-environment`). `#+RESULTS:` before a table does the same. That one change accounts for orgize missing the whole 872-cell entity table in `org-syntax.org` (62.8% agreement on that file).
  - The plain link `https://example.org` and the angle link `<https://ex.net>` stay text. On the Org manual, links are 416/521.
  - `[cite:@knuth1984]` stays text and is rendered as `[cite:knuth1984]`.
  - `*** COMMENT x` is not marked as commented. `#+TODO: … WAIT` is ignored without a `ParseConfig`. The `[fn:1]` definition body stays unparsed (`*markup*` is output raw).
- **Strengths.**
  - Fastest: 0.30 ms for 16 KB and 9.9 ms for the 887 KB Org manual.
  - The best position story: byte ranges on every node and token, lossless round-trip (`to_org`).
  - MIT.
  - Each construct is a small nom module. The single-dollar fragment parser, for example, is 27 lines. So individual bugs are cheap to fix in a fork, but there are many of them, and citations, plain and angle links, radio links and inlinetasks are missing modules, not bugs.

### organic ([crates.io](https://crates.io/crates/organic), [repo](https://code.fizz.buzz/talexander/organic))

- **What it is.**
  - "An emacs-less implementation of an org-mode parser". Its stated goal is "perfect parity with the emacs org-mode parser … any document that parses differently between Emacs and Organic is considered a bug".
  - Its scope is "roughly the output of `(org-element-parse-buffer)`", with no renderer. The author also states "under HEAVY development … the API will be changing often" ([README](https://docs.rs/crate/organic/0.1.16/source/README.md)).
  - Its test suite compares against a pinned Org revision in Docker (same README). That revision is older than 9.8.7, which probably explains the `$n$-th` divergence.
- **Fidelity.**
  - It has the best fidelity measured: 99.99% node-count agreement over 24,536 nodes, and 100% on the Org guide and on the syntax spec itself.
  - It handles in-buffer `#+TODO`, `COMMENT`, citations, plain, angle and radio links, and affiliated keywords.
  - Misses seen: an *empty* dynamic block (`#+BEGIN: clocktable\n#+END:`) becomes paragraph + keyword; there is no inlinetask (also off by default in Emacs: `emacs -Q` parses `***************` as a level-15 headline); and `$n$-th` is a fragment where 9.8.7 says it is not.
- **Health.**
  - Latest is 0.1.16 (2024-04-12) ([crates.io](https://crates.io/crates/organic/versions)).
  - The forge's commit log shows the last parser-related commits as tests on 2025-02-01. Everything since is CI, Docker and nix (latest 2026-07-17) ([Forgejo API commits](https://code.fizz.buzz/api/v1/repos/talexander/organic/commits)).
  - It is a single maintainer on a self-hosted forge with 0 stars. Its one reverse dependency is the author's SSG `natter` 0.0.1 (2023) ([crates.io](https://crates.io/crates/natter)).
- **Nightly only.**
  - The published `Cargo.toml` begins `cargo-features = ["codegen-backend"]`, so stable cargo refuses to even resolve it: "the cargo feature `codegen-backend` requires a nightly version of Cargo".
  - `lib.rs` enables 7 `#![feature]`s ([src/lib.rs](https://docs.rs/crate/organic/0.1.16/source/src/lib.rs)).
  - Spike: after deleting those lines, stable rustc 1.99 reported exactly 3 errors: two `trait` aliases in `src/context/mod.rs` and one `exact_size_is_empty`. Rewriting the aliases as trait + blanket impl and using `len() == 0` made it build on stable with the same output and speed.
- **Speed.** It is roughly 0.6–1.7 ms per KB, linear: 6.7 ms for 6 KB, 20 ms for 16 KB, 34 ms for 34 KB, 51–150 ms for 88 KB and 548 ms for the 887 KB manual. That is about 50–100× orgize. Typical posts fit a sub-100 ms incremental rebuild, but Org parsing would dominate cold builds of large Sites. The cause was not profiled (open question).
- **Positions.** There are no offsets as such. Every node carries `source: &'s str`, a slice of the input (`StandardProperties::get_source`), so the byte offset is `src.as_ptr() - input.as_ptr()`. The spike harness did exactly this. There is no lossless token layer.

### org-rust-parser ([crates.io](https://crates.io/crates/org-rust-parser), [repo](https://github.com/hydrobeam/org-rust))

- **What it is.** A hand-written parser into an arena (`NodePool`). Every `Node` has `start`/`end` byte offsets and an `attrs` map from affiliated keywords ([src/types.rs](https://docs.rs/crate/org-rust-parser/0.1.8/source/src/types.rs)). A sibling crate, `org-rust-exporter`, renders HTML, expands macros and includes, and converts LaTeX to MathML through `latex2mathml`.
- **Health.** It is the only actively maintained option. Releases were 0.1.6 (2025-11), 0.1.7 and 0.1.8 (2026-05), with a fix merged 2026-08-16 ([commits](https://github.com/hydrobeam/org-rust/commits/main)). It has 25 stars, 2 open issues, a single maintainer, MIT.
- **Gaps.**
  - The `Expr` enum ([src/types.rs:433](https://docs.rs/crate/org-rust-parser/0.1.8/source/src/types.rs)) has no citation, timestamp, planning, clock, radio target, statistics cookie, babel call, diary sexp, dynamic block or section variant. TODO keywords are a hard-coded `["TODO", "DONE"]` (`src/element/heading.rs:11`).
  - Spike results: `\begin{align*}` is not a LaTeX environment; the exporter emitted MathML for `b e g i n a l i g n *`. `Prices $5 or $ 6$` produced a false fragment. `x_{1}` and `a_{i}` were not subscripts. One list split into four `PlainList`s.
  - It reached 82.6% overall agreement. Part of the gap is structural (no `section` and no property-drawer node), but macros (751/1314) and inline code (1370/1876) are real misses.

### Crates screened out

Each of these was found by searching crates.io for `orgize`, `org-mode`, `orgmode`, `org parser` and `org` on 2026-10-04.

| Crate | Why it is out |
|---|---|
| [windancer](https://crates.io/crates/windancer) 0.1.3 (2025-12) | **GPL-3.0**, which is incompatible with yuanme's MIT OR Apache-2.0. Nightly (`#![feature(test)]`). Spike: the whole parse **panicked** ("Parse failed") on the citation file and on the file holding a babel call, diary sexp, inlinetask and dynamic block. Its own status table lists citations, statistics cookies, inline src, export snippets, clock, diary sexp, dynamic blocks and inlinetasks as unimplemented ([docs/status.org](https://github.com/cnglen/windancer/blob/main/docs/status.org)). |
| [org-element](https://crates.io/crates/org-element) 0.1.0 (2026-07) | A tree-sitter wrapper that claims "complete coverage of all 26 Org element types". Spike: no keywords, footnotes, LaTeX, citations, macros, drawers or most bold were emitted. One release, 0 stars. It bundles a `tree-sitter-org` C grammar (`build.rs`), and the main `tree-sitter-org` grammar repository is [archived](https://github.com/milisims/tree-sitter-org). |
| [orgo](https://crates.io/crates/orgo) 0.24.0 (2026-08) | An Org-only SSG (0BSD), not a parser library. Its `model::Object` has no LaTeX fragment or environment, citation, macro or sub/superscript (`src/model.rs`). Diagnostics carry only a line number. Its policy is "out-of-scope constructs degrade, never crash", the opposite of our Org subset rule. Relevant to [#9](https://github.com/yuann3/yuanme/issues/9) as a competitor. |
| [tftio-org](https://crates.io/crates/tftio-org) 0.1.2 | An agenda/task AST of about 2k lines. No LaTeX, footnotes, macros or citations (`src/ast.rs`). |
| [oak-org-mode](https://crates.io/crates/oak-org-mode) 0.0.11 | About 1.2k lines inside a multi-language framework, MPL-2.0. Kinds stop at blocks, drawers, links and basic markup. |
| [org-tools-core](https://crates.io/crates/org-tools-core) 0.1.4 | **GPL-3.0-or-later**, a lint/format toolkit. |
| [starsector](https://crates.io/crates/starsector), [org-core](https://crates.io/crates/org-core) | Structural tools built on orgize; they inherit its parser. |
| [org](https://crates.io/crates/org) 0.3.1 (2019), [orgora](https://crates.io/crates/orgora), [org-spec](https://crates.io/crates/org-spec), [orgremode](https://crates.io/crates/orgremode) | Abandoned or toy (under 1k lines, or no release since 2019–2023). |

### Writing our own: what it would take

- **Size.** For scale, `org-element.el` covers everything in 8,785 lines of Elisp. orgize is about 14k lines of Rust (including its exporters), org-rust-parser about 10k (plus 3.5k exporter) and organic about 29.5k (`wc -l` over each crate's `src`). A parser that fully *renders* only the Org subset but *recognises* every element and object type, enough to reject it with a span, plausibly needs 5–8k lines. That is an estimate, not a measurement.
- **What we would get that no candidate gives:**
  - The Org subset as a first-class concept: an exhaustive node enum, with every unsupported kind producing a build error with file, line and column.
  - The same metadata model as Markdown (#16).
  - Spans on every node and token, like orgize. A rowan-style CST is a good template.
  - orgize-class speed.
  - The divergences we choose (`$n$-th`, `^:{}`, inlinetasks) written down instead of inherited.
- **How to make fidelity measurable from day one.** This spike's harness is cheap to keep:
  - `emacs --batch -Q -l dump.el` prints the `org-element` type tree for a file, and a count script turns that into per-type agreement.
  - Commit the Emacs outputs as fixtures, so CI does not need Emacs, and regenerate them when we bump the pinned Org version.
  - Seed the corpus with the Org manual, guide and syntax spec, with organic's tests (0BSD, copyable), and with the Org/math corpus from [#14](https://github.com/yuann3/yuanme/issues/14).
  - A stable-patched organic can serve as a fast in-Rust differential oracle for fuzzing.

### Spike method (reproducible)

All of this was in `/private/tmp/orgspike` on 2026-10-04, using rustc/cargo 1.99.0 (Homebrew), an Apple M4 Max, and Emacs 31.1 with Org 9.8.7.

1. **Corpus.**
   - 16 small files, one per ticket feature: headings and planning, markup, keywords and affiliated keywords, every block type, every link type, footnotes, tables with TBLFM, LaTeX fragments and environments, citations, `#+INCLUDE`, drawers and clock, in-buffer `#+TODO`, macros, lists, babel/diary/inlinetask/dynamic block, and a math-heavy page.
   - Plus three large real documents from the Org sources: `doc/org-manual.org` (887 KB), `doc/org-guide.org` (88 KB, needs `doc/doc-setup.org` beside it for `#+SETUPFILE`) and Worg's `org-syntax.org` (89 KB).
2. **Oracle.** For each file, a 28-line `dump.el` running `org-element-parse-buffer` printed the element and object type tree, and a `count.el` counted types (`org-element-map … t` with affiliated).
3. **Harnesses.** One small binary per crate printed node kind and byte span, plus a `bench` binary giving the median of 20 parses and per-kind counts. Kinds were mapped onto Emacs type names, for example orgize `ORG_TABLE_STANDARD_ROW`/`RULE_ROW` → `table-row` and organic `RegularLink|PlainLink|AngleLink|RadioLink` → `link`.
4. **Score.** For each document and Emacs type: `min(parser, emacs) − max(0, parser − emacs)`, summed and divided by the Emacs total. This checks counts, not tree shape, so it is an upper bound on structural agreement.
5. **organic.** It was built with `RUSTC_BOOTSTRAP=1`, and also, after the 3-error patch, on plain stable. Timings were the same.

## Open questions

These are the decisions this research surfaced.

1. **Which Org version is the oracle, and do we follow `org-element` or the spec text where they differ?** The first known case is `$n$-th`; Org 9.8.7 and the spec disagree. Recommendation: pin an Org release and follow its `org-element`, since the spec says it describes `org-element`.
2. **Does the parser *recognise* constructs the Org subset excludes?** (We recommend yes.) That sets its scope: all 30 element and 24 object types at the recognise-and-span level. This belongs to #16.
3. **`#+INCLUDE` and `#+SETUPFILE`.** Are they in the subset? If they are: which options, which path boundary (no escaping the Site root), and how do diagnostics report positions in included files?
4. **Citations.** Render `[cite:@key]` with a CSL processor, or put citations outside the subset in v1? This is #17's call, but the parser must recognise them either way.
5. **Subscripts in programmer prose.** `snake_case` parses as a subscript in Org unless `#+OPTIONS: ^:{}`. Should the Engine default to `^:{}` for programmers, and is that a Parity exception-style documented divergence?
6. **CST representation.** Use rowan (as orgize does, with lossless round-trip and edit APIs useful for agent edits and #11 incremental rebuilds) or a plain arena with spans (as org-rust-parser does)?
7. **Is organic's 50–100× slowdown cheap to fix?** It was not profiled. If it is, a fork of organic becomes a stronger alternative to writing our own.

## Verification

An independent re-check on 2026-10-04 re-ran the spike harness and re-queried the primary sources. The recommendation holds. Corrections and additions:

- **Confirmed.**
  - crates.io: orgize 0.10.0-alpha.10 (2024-06-11, MIT), organic 0.1.16 (2024-04-12, 0BSD), org-rust-parser 0.1.8 (2026-05-07, MIT, MSRV 1.91), windancer 0.1.3 (GPL-3.0).
  - GitHub: the orgize `v0.10` branch's last commit is 2024-07-22, with 17 open issues and PRs and 47 forks. org-rust-parser's last commit (`fix: source block HTML generation (#12)`) is 2026-08-16.
  - The organic forge shows no parser change since 2024-04-11. The one later `src/` touch, on 2024-09-30, only removed `#![feature(is_sorted)]` and added a clippy `allow`.
  - Emacs 31.1 reports Org 9.8.7 with 30 element and 24 object types. `Let $a$, then $b$. And $d$-th and $c$ here` yields `$a$ $b$ $c$`, so `$d$-th` is not a fragment.
  - `score.py` reproduces 88.32% / 99.99% / 82.58% over 24,536 nodes.
  - The orgize harness reproduces the `$a$,` miss (only `$c$` is a fragment), `#+CAPTION` table → `PARAGRAPH`, and `#+NAME`d equation → `PARAGRAPH` with `LATEX_FRAGMENT`.
  - The orgize POST check is at `src/syntax/latex_fragment.rs:116-121`. organic's `lib.rs` (as published in 0.1.16) has 7 `#![feature]`s. org-rust-parser hard-codes `["TODO", "DONE"]` at `src/element/heading.rs:11`.
- **Correction: absolute timings are about 3× too high; the ratio stands.** Re-running the same bench binaries on the same M4 Max gave, for 16 KB / 88 KB / 887 KB: organic 6.6–7.3 / 36–39 / 366–407 ms, orgize 0.10–0.11 / 0.54 / 6.5 ms, org-rust-parser 0.22 / 1.3 / 13.4 ms. organic is still about 55–75× slower than orgize. However, "roughly 20 ms for a 16 KB post" should read "roughly 7 ms". That makes organic's speed a weaker objection for typical posts, but not for cold builds of large Sites.
- **Addition: organic aborts the whole parse on a missing `#+SETUPFILE`.** Running the bench on `org-manual.org` from a directory where `doc-setup.org` was not resolvable failed the entire parse with `Parsing Failure: IO(... NotFound)`, with no partial tree. Emacs only warns in that case. A vendored organic fallback would need this changed, so that an Engine diagnostic with a span is reported instead.
- **Option not discussed: shelling out to Emacs at build time** (the ox-hugo style, with `org-element` as the parser itself). That gives perfect fidelity by definition, but it conflicts with ADR 0003's "one binary" toolchain goal and adds Emacs startup time to every build. It stays an oracle for fixtures, not a runtime dependency.
- **Crates search re-run** (crates.io, `org-mode`/`orgmode`/`org parser`/`org-syntax`, sorted by recent updates). It found no new general-purpose parser. `ikigai-org`, `vix-org` (regex-based), `markdown-org-extract`, `org-rust` (org-rust-parser's CLI) and `orgrender` (built on orgize) are agenda tools, editors or wrappers.
