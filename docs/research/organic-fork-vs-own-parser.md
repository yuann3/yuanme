# Is fixing organic's performance cheaper than writing our own Org parser?

Ticket: [#30](https://github.com/yuann3/yuanme/issues/30) (wayfinder research). It follows up open question 7 of the Org parser research ([#3](https://github.com/yuann3/yuanme/issues/3), [`docs/research/org-parser.md` on `research/org-parser`](https://github.com/yuann3/yuanme/blob/research/org-parser/docs/research/org-parser.md)). Researched 2026-10-04. Vocabulary follows [`CONTEXT.md`](../../CONTEXT.md).

## Question

The Org parser research recommended writing our own parser, with a vendored fork of organic (0BSD) as the fallback. organic agrees with Emacs `org-element` on 99.99% of nodes, but it is 50–100× slower than the alternatives and needs nightly Rust. This ticket asks:

- Why is organic slow? Profile it on a realistic corpus.
- What does it cost to make a fork build on stable Rust, run fast enough for sub-100 ms rebuilds, expose byte spans for diagnostics, and stop failing hard on a missing `#+SETUPFILE`?
- How does that compare with writing our own parser? Fork or write?

## Short answer

**Fork organic.** This reverses the primary recommendation of the Org parser research. Its two blocking objections, speed and nightly Rust, turned out to be cheap to fix, and the fork starts at the best fidelity available. An independent re-check upheld this, but it found more exponential-time paths that survive the patches. The fork must therefore bound parse time in general, and the estimate rises to about 14–20 days. See [Verification](#verification).

- **Why it is slow.** The time goes into organic's architecture, not into one hot function. Inside plain text, organic advances one character at a time. At every character it walks the whole stack of "exit matchers" and tries all ~20 object parsers. One of those exit matchers, `paragraph_end`, fully parses a candidate element at every character. Measured over the 293-file Worg corpus, that comes to roughly one full element-parse attempt per character of plain text. The author's own notes name the same two costs ([`notes/optimization_ideas.org`](https://code.fizz.buzz/talexander/organic/src/branch/main/notes/optimization_ideas.org)).
- **What a few hours of patching bought.** Six small patches (127 added lines in 5 files) made organic **4.1× faster** on Worg: 1,489 → 362 ms, or 237 → 58 µs/KB. A 16 KB file now parses in 1.5 ms instead of 6.3 ms, and the 887 KB Org manual in 68 ms instead of 341 ms. The output was byte-identical, by AST hash, on all 627 corpus, sample and synthetic files and on 2,000 fuzz documents. A held-out re-check found 1 difference in 5,000 new fuzz documents, on input that both builds already parse differently from Emacs (see [Verification](#verification)). organic is still about 12× slower than orgize, but that is far inside the budget. The [#11](https://github.com/yuann3/yuanme/issues/11) plan re-parses only the edited Post and leaves about 55 ms for the browser.
- **The real risks are worse than slowness: exponential time and crashes.**
  - A 73-byte line of unclosed markup (`*a /b ` repeated 12 times) takes organic **88 seconds**, against 0.14 ms in Emacs. Patch P6 makes it 0.17 ms. P6 fixes only that one class: unclosed inline footnotes and sub/superscript braces are still exponential after P1–P6 (see [Verification](#verification)).
  - A 2-minute fuzz run found two crash classes in unpatched organic. A stray `:END:` line followed by a timestamp or a link **panics**. A line containing only `|` makes the **whole document fail to parse**. Emacs parses both.
  - The fork therefore needs a fuzz harness with a time budget, crash fixes and a `catch_unwind` boundary. Those are the largest line items in the estimate below.
- **The other asks are nearly free.**
  - **Stable Rust**: done in the earlier spike. It needed 3 compile errors fixed and one manifest line removed, plus the two `#[bench]`s and the wasm-only `gloo-utils` dependency removed.
  - **Byte spans**: every node's `source` is a slice of the input. All 173,083 nodes in the corpus check out, so a span is pointer arithmetic.
  - **Missing `#+SETUPFILE`**: needs **no fork change at all**. The Engine supplies its own `FileAccessInterface` that records the miss and returns empty contents. A spike reported `org-guide.org:2645: #+SETUPFILE "doc-setup.org" not found (bytes 86676..86702)` instead of aborting.
- **Cost.** Making the fork production-ready is about **2–3 focused weeks** (3–4 after verification). Writing our own is about **6–12 weeks**, with much more fidelity risk. organic's author needed 21 months and about 1,500 source commits to reach parity.

**When to revisit:** two conditions would reopen "write our own":

- Fuzzing with a time budget keeps finding new superlinear or crash classes after the first round of fixes. The re-check found three more exponential classes before that first round. They share P6's root cause, so the trigger now applies to classes that survive a general fix: per-construct close prechecks plus a fuel limit.
- v1 needs a lossless token-level CST, for example for agent-driven source edits.

## Comparison

### Measured speed, before and after patching

Machine: Apple M4 Max, rustc 1.99.0 stable, release builds. Each file's time is the best of 3 runs (Worg) or the median of 20 (single files). Method: see [Spike method](#spike-method-reproducible).

| | organic 0.1.16 (stable patch only) | organic + P1–P6 | orgize 0.10.0-alpha.10 |
|---|---|---|---|
| Worg, 293 files, 6.43 MB, total single-threaded | 1,488.5 ms (237 µs/KB) | **362.1 ms (57.7 µs/KB)** | 28.9 ms (4.6 µs/KB) |
| Worg per file: median / p90 / p99 / max | 2.17 / 10.6 / 52.6 / 229 ms | **0.46 / 2.13 / 19.7 / 33.2 ms** | 0.04 / 0.20 / 1.22 / 2.69 ms |
| 16 KB prose with math and links (`flat16.org`) | 7.47 ms | **1.48 ms** | not run |
| 16 KB Org guide excerpt (`guide-500.org`) | 6.25 ms | **1.50 ms** | ≈0.10 ms (org-parser verification) |
| 887 KB Org manual | 341 ms | **68 ms** | ≈6.5 ms (org-parser verification) |
| `*a /b ` × 12 (73 bytes) | **87,826 ms** | 0.17 ms | not run |
| Worst of 400 random 80-char markup strings | 16.3 ms | 0.13 ms | not run |
| Slowdown vs orgize on Worg | 51× | 12.5× | 1× |

### Fork versus write

| | **Fork organic (recommended)** | **Write our own** |
|---|---|---|
| Fidelity on day one | 99.99% of 24,536 nodes against Org 9.8.7 (org-parser spike); its oracle is pinned to Org `main` of 2023-10-13 | 0%; reaching ≥99.9% is the bulk of the work |
| Speed | 58 µs/KB after P1–P6; deeper refactors could plausibly give another 2–4× (estimate) | orgize-class (about 5 µs/KB) is realistic for a hand-written parser |
| Builds on stable | yes, after a 3-error patch | yes |
| Byte spans | node level, via `source` slices (verified) | node and token level, by design |
| Missing `#+SETUPFILE` | Engine-side `FileAccessInterface`, no fork change | by design |
| Known defects | at least 4 exponential-time classes (P6 fixes 1; inline footnotes, `[fn:x:` references and `_{`/`^{` remain), several quadratic ones, 2 crash classes (unfixed), link descriptions spanning blank lines, no inlinetask, empty dynamic block, `$n$-th` | unknown until written |
| Code we own | about 17.4k lines of nom 7 combinators with 4 lifetimes (`'b 'g 'r 's`) | estimated 5–8k lines (org-parser research) |
| Effort to production-ready (estimate) | **about 14–20 focused days** (10–14 before verification) | **about 30–60 focused days** |
| Main risk | more superlinear paths hidden in the re-parse-heavy design | the long tail of `org-element` quirks |

## Details

### Where organic's time goes

**Profile.** The Worg corpus was parsed 3 times under samply at 4 kHz with debug symbols, giving 4,322 samples. The script folded each stack into per-function inclusive and self time (see Spike method). Self time is flat: the top entry is 7.6%, and no single function is worth optimising in isolation. Inclusive time shows the structure:

| Inclusive | Function (organic 0.1.16) | What it is |
|---|---|---|
| 78% | `many_till(anychar, …)` inside `_plain_text` ([`plain_text.rs:46-66`](https://docs.rs/crate/organic/0.1.16/source/src/parser/plain_text.rs)) | the per-character plain-text loop |
| 55% | `standard_set_object_sans_plain_text` ([`object_parser.rs:70`](https://docs.rs/crate/organic/0.1.16/source/src/parser/object_parser.rs)), called through `detect_standard_set_object_sans_plain_text` (line 128) | at **every** character, try timestamp, sub/superscript, cookie, target, line break, inline src, babel call, citation, footnote, export snippet, entity, LaTeX, radio link and target, markup, regular, plain and angle links, macro |
| 34% | `Context::check_exit_matcher` ([`context.rs:97-117`](https://docs.rs/crate/organic/0.1.16/source/src/context/context.rs)) | at every character, walk the linked-list context stack and run every applicable exit matcher (56 exit-matcher sites in `src/parser`) |
| 25% | `paragraph_end` ([`paragraph.rs:133-145`](https://docs.rs/crate/organic/0.1.16/source/src/parser/paragraph.rs)) | the paragraph's exit matcher; at every character it calls `detect_element` |
| 23% | `_detect_element` ([`element_parser.rs:274-334`](https://docs.rs/crate/organic/0.1.16/source/src/parser/element_parser.rs)) | runs six cheap detectors, then falls back to a **full** `_element` parse (line 330) just to answer "does an element start here?" |
| 14% | `OrgSource::compare_no_case` ([`org_source.rs:171`](https://docs.rs/crate/organic/0.1.16/source/src/parser/org_source.rs)) | delegates to nom 7's `&str` version, which lowercases each char with Unicode tables (`to_lower` 3.6% and `ToTitlecase` 3.0% self time); mostly from `plain_link::protocol` (line 260) trying 23 link protocols at each position |

**Counts.** An instrumented copy counted calls over the same corpus (6,431,567 input bytes):

| Event | Count | Per input byte |
|---|---|---|
| characters consumed by plain text | 2,780,026 | 0.43 |
| `standard_set_object_sans_plain_text` attempts | 2,451,542 | 0.38 |
| `check_exit_matcher` walks | 4,575,651 | 0.71 |
| `_detect_element` calls | 2,360,547 | 0.37 |
| `_element` calls | 2,376,292 | 0.37 |

So nearly every plain-text character triggers one full element-parse attempt and one full object-set attempt. That works out to about 535 ns per plain-text character, against about 4.5 ns per byte for orgize. Scaling is linear in size (4 KB → 256 KB: 0.45 ms/KB throughout), and nesting adds a little: 40 ms for flat 50 KB lists against 50 ms at depth 16. The slowness is a large constant factor, except in the exponential case below.

The author documented these costs in 2023 and judged the fixes risky. "Make detect element function": "Avoiding parsing the entire element for an exit matcher would reduce redundant parses". "Grab multiple characters in plaintext parser before checking exit matcher": "This could significantly reduce our calls to exit matchers … I think targets would break this." Both are in [`notes/optimization_ideas.org`](https://code.fizz.buzz/talexander/organic/src/branch/main/notes/optimization_ideas.org). The forge log shows a burst of performance merges in October 2023 (`object_parser_perf`, `list_perf_improvement`, `perf_improvement`, `lesser_block_memory_optimization`) and none since.

### The patches (P1–P6) and what they bought

Each patch was checked the same way. Every file in the differential set was parsed by both builds, the `{:?}` dump of the `Document` was hashed, and the hashes were compared.

- **Differential set.** 627 files: the 293 Worg `.org` files, organic's own 301 `org_mode_samples` (0BSD), the Org manual, guide and syntax spec, the 16-file spike corpus, and synthetic scaling files.
- **Fuzz set.** 2,000 random Org-ish documents, comparing results including panics and errors.
- **Outcome.** Every patch kept **0 mismatches**, so fidelity to Emacs is exactly that of unpatched organic on these inputs.

| Patch | Change | Worg total after |
|---|---|---|
| — | stable-patched 0.1.16 | 1,488 ms |
| P1 | `paragraph_end` returns "not an exit" when the input is not at the start of a line (only `eof` can still match), instead of running `detect_element`. Instrumented first: across the 594-file corpus, `detect_element` never matched mid-line. | 1,182 ms |
| P2 | ASCII fast path in `OrgSource::compare_no_case`. It falls back to nom's Unicode compare when either prefix is not ASCII, so the Kelvin sign and similar characters behave as before. | 1,082 ms |
| P3 | `could_start_object` pre-filter before the object-set detectors. It skips characters that cannot begin any object, and it is off when the document has radio targets. | 599 ms |
| P4 | Tighter pre-filter: `s`/`c` only if followed by `src_`/`call_`; letters only as a plain-link protocol, which must follow a non-word character and reach `:` before any whitespace | 402 ms |
| P5 | In the plain-text loop, skip the exit-matcher walk between two ASCII alphanumerics. This rests on the observation that no exit matcher fires inside a word. It is an empirical claim, checked by the differential and fuzz sets, not proved. | 335 ms |
| P6 | In text markup, `could_close`: before the recursive re-parse, require a candidate closing marker (not after whitespace, followed by a POST character) before the next blank line. Its main purpose is to prevent exponential time (next section). | 362 ms (noise ±10 ms) |

The remaining profile is flat: `OrgSource::slice` rescans skipped bytes to track brackets and line starts (10.5% self), exit-matcher walks (6%), table cells and links. The next gains would come from deeper refactors:

- give each exit matcher an explicit trigger-byte set;
- replace the fall-back to full `_element` parses in `_detect_element` with real detectors;
- track bracket depth without rescanning.

These cost an estimated 1–3 weeks and are **not needed** for the budgets.

### Superlinear time and crashes

These findings change the risk picture more than the constant factor does.

- **Exponential time on unclosed markup.**
  - **Cause.** `_text_markup_object` ([`text_markup.rs:213-262`](https://docs.rs/crate/organic/0.1.16/source/src/parser/text_markup.rs)) scans ahead with `text_until_exit` to find the closing marker. It then re-parses the contents with a fresh context, and the plain-text detectors inside that re-parse try markup again.
  - **Growth.** With no closing marker, the cost about doubles for each opener: `*a /b ` × 4 / 6 / 8 / 10 / 12 took 1.4 / 21 / 337 / 5,409 / 87,826 ms.
  - **Realistic input.** `char *p, *q, *r, *s, *t, *u, *v, *w, *x, *y;` (47 bytes) took 4.4 ms, about 400× the corpus rate.
  - **Emacs.** `org-element-parse-buffer` parses the 73-byte line in 0.14 ms (`emacs --batch`, Org 9.8.7).
  - **Fix.** P6 brings that line to 0.17 ms, and the worst of 400 random 80-character markup strings from 16.3 ms down to 0.13 ms. It does not prove that no other exponential path exists: other parsers also re-parse after scanning ahead.
- **Panic on a stray `:END:`.** `:END:\n[2024-01-01 Mon]\n` (or `:END:\nhttps://example.org\n`, or `#+BEGIN: x\n<2024-01-01 Mon> y\n`) panics with `Unhandled first object type inside bullshitium`. The cause is `broken_end` and `broken_dynamic_block`, which assume the first object of the following paragraph is plain text ([`bullshitium.rs:69` and `:130`](https://docs.rs/crate/organic/0.1.16/source/src/parser/bullshitium.rs)). Emacs 9.8.7 parses the first input as `section > paragraph > timestamp`. This was 310 of the 2,000 fuzz documents, and P1–P6 do not change it.
- **Whole-document failure on a lone `|`.** Any line consisting only of `|` (or `|  `) makes `parse` return `Parsing Error: Parser(Eof)` for the entire file, for example `a\n|\nb\n`. Emacs parses `|` as `table > table-row`. This was 93 of the 2,000 fuzz documents. All 6 minimised cases reduced to `|`.
- **Radio targets double the work.** One `<<<radio>>>` anywhere makes organic parse the whole document twice ([`document.rs:157-161`](https://docs.rs/crate/organic/0.1.16/source/src/parser/document.rs)), and P3/P4 switch off for that document. This is correct but slow: a 1.7 KB fuzz file with a radio target took 33 ms unpatched and 10 ms patched.

Each crash fix looks small, about a day including a regression sample. The cost that matters is finding the next one, so the fork needs:

- continuous fuzzing that checks both crashes and a per-KB time budget;
- a `std::panic::catch_unwind` boundary in the Engine, so that a parser panic becomes a diagnostic for one file instead of a crashed build.

### Building on stable

The org-parser spike already showed the fix:

- Delete `cargo-features = ["codegen-backend"]` and the two `codegen-backend` profile keys from `Cargo.toml`.
- Delete the 7 `#![feature]`s from `lib.rs`.
- Fix the 3 errors that remain on stable: the two `trait` aliases in `src/context/mod.rs` become a trait plus a blanket impl, and `is_empty()` on an `ExactSizeIterator` becomes `len() == 0`.

That patch is what all measurements here ran on (rustc 1.99.0 stable). Two more items surfaced:

- `cargo test` still fails on stable because of two `#[bench]`s and `extern crate test`. The tests also `include_str!` files from `org_mode_samples/`, which the crate does not ship, so the fork should be taken from the [git repository](https://code.fizz.buzz/talexander/organic), not from crates.io.
- `gloo-utils` 0.2.0 is a non-optional dependency. It pulls `wasm-bindgen`, `js-sys`, `web-sys` and `serde_json` into native builds, but only `src/wasm_cli/mod.rs` uses it. The fork drops it together with the `compare`, `wasm` and `wasm_test` modules, which are about 9.9k of the 29.5k lines.

A clean release build of the library took 6.6 s. Its only remaining dependency is nom 7.1.3 (2023-01-15). nom 8.0.0 (2025-01-26) is current ([crates.io](https://crates.io/crates/nom/versions)), and porting to it is optional.

### Byte spans for diagnostics

Every node carries `source: &'s str` through `StandardProperties::get_source`. A harness walked `iter_all_ast_nodes()` over all 594 corpus and sample files: **173,083 nodes, 0 with a source outside the input buffer**. So `span = (src.as_ptr() - input.as_ptr(), + src.len())` holds for every node. Many sub-fields, such as `Keyword.key` and `.value`, are slices as well, so they get spans the same way. The fork adds a safe `span(&self, input: &str) -> Range<usize>` helper and line and column mapping, about half a day.

What it does **not** give:

- token-level spans, such as the `*` markers of bold;
- a lossless CST, as orgize's rowan tree provides;
- an error position. `parse_file_with_settings` turns errors into strings ([`document.rs:81`](https://docs.rs/crate/organic/0.1.16/source/src/parser/document.rs)), but Org has no syntax errors, so a parse error is always one of the bugs above.

### Missing `#+SETUPFILE`

organic reads setup files through `GlobalSettings.file_access: &dyn FileAccessInterface`, a public trait. A read error becomes `nom::Err::Failure`, which aborts the parse ([`document.rs:122-131`](https://docs.rs/crate/organic/0.1.16/source/src/parser/document.rs)). Emacs instead calls `(org-file-contents uri :noerror)` in `org--collect-keywords-1`, and a `file-error` there only logs `"Unable to read file %S"` and returns nil. Line numbers are from the `lisp/org/org.el` of the Org 9.8.7 bundled with Emacs 31.1: the `SETUPFILE` branch is at lines 4725–4741 and the error handling at 4912–4921.

**Fix without forking.** The Engine passes its own `FileAccessInterface`. The impl:

- enforces the path policy, so no path can escape the Site root;
- records misses and returns `Ok(String::new())`;
- after the parse, finds the `Keyword` node whose key is `setupfile` and value matches, and reports a warning or error with its span.

The spike printed `org-guide.org:2645: #+SETUPFILE "doc-setup.org" not found (bytes 86676..86702)` where unpatched organic failed with `Parsing Failure: IO(NotFound)`. A remaining gap: Emacs follows `SETUPFILE`s recursively, relative to each setup file's own directory, with cycle detection. organic reads one level. A fork change of a few dozen lines closes that if the Org subset keeps `#+SETUPFILE`.

### Effort to production-ready, item by item

These are estimates, in focused engineering days, for one person or agent with the spike harness in hand.

| Item | Fork | Own parser |
|---|---|---|
| Vendor and set up stable CI; drop wasm, compare and `gloo-utils` | 1 | — |
| Productionise P1–P6, with a regression benchmark and committed differential fixtures | 2–3 | — |
| Span API and line/column mapping | 0.5 | included |
| `#+SETUPFILE` via `FileAccessInterface`, path policy, recursion | 1 | 1 |
| Fix the two crash classes; add `catch_unwind` and a fuzz harness with crash and time budgets | 3–4 | 2 (fuzzing is needed either way) |
| Bound the remaining exponential and quadratic paths: close prechecks for each recursive object type, and a fuel limit that turns a runaway parse into a diagnostic (added by verification) | 4–6 | 0–1 (a linear design from the start) |
| Re-pin the oracle to Org 9.8.7 and fix divergences such as `$n$-th` | 2–4 | included below |
| Recognise all 30 element and 24 object types with spans (5–8k lines) | — | 15–25 |
| Long-tail fidelity from first pass to ≥99.9% on the corpus | — | 10–30 |
| **Total** | **≈14–20** | **≈30–60** |

The Org subset gate ([#16](https://github.com/yuann3/yuanme/issues/16)) costs the same either way, so it is left out of the table. It is an exhaustive `match` over a node enum that turns excluded kinds into build errors. organic's `AstNode` already has 59 variants to match on.

**Reference point for the long tail.** organic's first commit was 2022-07-16 and its last parser change 2024-04-11. In between, 1,505 commits touched `src/`, peaking at 408 in October 2023 (`git log` of the forge repository). That was one author working toward exact `org-element` parity. We would start with organic as a second oracle and its 301 samples, which should shorten this a lot. The quirks that remain are still the kind of thing a re-implementation gets wrong for months: trailing-whitespace ownership in lists, affiliated keywords before non-affiliable elements, and the `:END:` and `#+BEGIN:` "bullshitium" paragraphs.

### What we inherit, and what we give up

- **Inherit.** The best fidelity measured, in-buffer `#+TODO`, citations, all link kinds, a `FileAccessInterface` seam, a typed `AstNode` enum and 0BSD licensing. 0BSD allows relicensing under yuanme's MIT OR Apache-2.0 with no attribution requirement.
- **Give up, or must build on top.**
  - orgize-class speed: 12× slower, but within budget.
  - Token-level spans and lossless round-trip.
  - A codebase designed by us: nom 7 closures under four lifetimes, and a design that detects by re-parsing.
  - An upstream. The last parser change was 2024-04, so the fork *is* the upstream from now on.
- **Fidelity drift.** organic's oracle is Emacs 29.1 with Org `main` at [`abf5156`](https://github.com/emacs-straight/org-mode/commit/abf5156096c06ee5aa05795c3dc5a065f76ada97) (2023-10-13), according to its [Dockerfile](https://code.fizz.buzz/talexander/organic/src/branch/main/docker/organic_test/Dockerfile). It still matched Org 9.8.7 on 99.99% of nodes, so the drift is small, but it is ours to track.

### Cold builds

Org parsing is per file and embarrassingly parallel. The Worg rate after P1–P6 is 57.7 µs/KB. A Site of 1,000 Posts of 16 KB each would cost about 0.9 s of CPU. Over the 12 performance cores of this machine that is roughly 80–100 ms of wall time. This is an extrapolation, not a measurement. Under the #11 plan, an edit re-parses only the changed file; leaf results are cached by content hash. That is about 1.5 ms for a 16 KB Post.

## Spike method (reproducible)

Everything was done in `/private/tmp/organicprof` on 2026-10-04: rustc/cargo 1.99.0 (Homebrew, stable), Apple M4 Max, Emacs 31.1 with Org 9.8.7, samply 0.13.1 (`cargo install samply`).

1. **Sources.** The stable-patched organic 0.1.16 from the org-parser spike, and the organic git repository at `336b5d3` (2026-07-17), which supplied the samples, notes and history. Worg was a shallow clone of `https://git.sr.ht/~bzg/worg`: 293 `.org` files, 6,431,567 bytes, median 6.8 KB.
2. **Timing.** A `corpus` binary parses each file 3× with `parse_with_settings`, with `LocalFileAccessInterface` set to the file's directory, and keeps the best time. The orgize equivalent uses `Org::parse`. Single files used the earlier `bench` binary (median of 20).
3. **Profile.** `samply record --save-only --unstable-presymbolicate -r 4000` ran over 3 Worg passes. A 43-line Python script maps frame addresses to symbols through the `.syms.json` sidecar and folds stacks into self and inclusive percentages.
4. **Counts.** A copy of organic with `AtomicU64` counters in the five functions in the table above.
5. **Differential.** A `dump` binary prints `path, hash({:?} of Document), len, ok|ERR|PANIC` with `catch_unwind`. It runs on the unpatched build and on each patched build, and the two outputs are diffed.
6. **Fuzz.** A seeded generator draws random line starters (headings, bullets, `| `, `#+NAME:`, block delimiters, `:PROPERTIES:`/`:END:`, `\begin{equation}`, planning) and inline tokens (all markup, links, footnotes, citations, macros, timestamps, entities, LaTeX, non-ASCII). It produced 2,000 documents of 3–30 lines. A ddmin-style script minimised the failures.
7. **Emacs checks.** The org-parser spike's `dump.el` gave the type trees for the panic and `|` cases. An 8-line `timeparse.el` timed 10× `org-element-parse-buffer` on the exponential case.

## Open questions

1. **Do we adopt this reversal?** The ticket asks for a recommendation; this document recommends forking. If accepted, [#16](https://github.com/yuann3/yuanme/issues/16) and [#17](https://github.com/yuann3/yuanme/issues/17) should assume organic's `AstNode` as the parser surface, and the org-parser research's "Short answer" should be marked superseded.
2. **What is the fuzz time budget?** A per-input cap, for example "no input parses slower than 1 ms/KB plus 1 ms", turns "no more exponential paths" into a CI check. The number belongs to [#15](https://github.com/yuann3/yuanme/issues/15) (performance budgets).
3. **Do we upstream the patches?** The forge is self-hosted. Its parser code has not changed since 2024-04, but the author was still committing build and CI changes in July 2026 (see Verification). 0BSD does not require upstreaming. Upstreaming would be a courtesy, not a dependency.
4. **Token-level spans or a lossless CST?** If agent edits or [#21](https://github.com/yuann3/yuanme/issues/21) (the agent-native surface) need exact source rewriting of Org, organic's node-level slices may not be enough. That is the clearest trigger for revisiting "write our own".
5. **nom 7 or nom 8?** Staying on nom 7.1.3 costs nothing today. A port is mechanical but touches most of the 12k parser lines, so [#27](https://github.com/yuann3/yuanme/issues/27) (dependency policy) should say whether an unmaintained major version is acceptable.
6. **Which Org version is the oracle?** organic tests against Org `main` of 2023-10-13, and the Engine wants Org 9.8.7. Re-pinning means running organic's compare tooling against a newer Emacs and fixing the diffs. The size of that job is unknown until it is run.
7. **Fuel limit or per-construct prechecks?** A fuel limit, such as a counter in `check_exit_matcher`, guarantees termination. It fails only the pathological file, and that file would need a plain-text fallback for its paragraph. Prechecks keep exact output but have to be written for each construct. The fork probably needs both. [#15](https://github.com/yuann3/yuanme/issues/15) should set the fuel budget.

## Verification

An independent re-check on 2026-10-04 used the same machine, the spike's built binaries in `/private/tmp/organicprof` and Emacs 31.1 / Org 9.8.7. It confirmed most of the findings and corrected several. **The recommendation still holds**: forking costs about 14–20 days against 30–60 for our own parser. But the margin is smaller, and bounding parse time is now required work, not optional.

**Reproduced as stated.**
- Worg totals from the TSVs: 1,488.5 / 362.1 / 28.9 ms over 6,431,567 bytes (237.0 / 57.7 / 4.6 µs/KB). A fresh run gave 1,457 / 354 / 21.5 ms, so the orgize ratio is 12–16× depending on the run.
- Single files: manual 336 → 67 ms, `guide-500.org` 6.25 → 1.36 ms, `flat16.org` 7.40 → 1.47 ms.
- The patch diff is 127 insertions in 5 files.
- AST hashes are identical on the 594 + 33 files and on the original 2,000 fuzz documents. The fuzz set had 1,597 ok, 310 panics and 93 errors.
- `*a /b ` × 4 / 6 / 8 took 3.0 / 25.6 / 278 ms unpatched and 0.06 ms patched. Emacs parsed the 73-byte line in 0.135 ms.
- The `bullshitium.rs:69` and `:130` panics and the `a\n|\nb\n` → `Parser(Eof)` error reproduce. Emacs gives `section > paragraph > timestamp` and `table > table-row`.
- Spans: 173,083 nodes, 0 outside the input.
- The `#+SETUPFILE` spike: the default parse fails with `IO(NotFound)` when `doc-setup.org` is absent, and the lenient interface reports the spanned warning.
- Every cited organic line number, the `org.el` lines 4725–4741 and 4910–4921 (`org-file-contents uri :noerror` → `message "Unable to read file %S"`), the `optimization_ideas.org` quotes and the oracle pin (`emacs-29.1`, Org `abf5156`) check out.
- Repository history: 1,505 commits touch `src/`, and the last change under `src/parser` was 2024-04-11. Of the 29,490 lines in `src`, the compare, wasm, wasm_test and wasm_cli modules are 9,899. `gloo-utils` 0.2.0 is non-optional. nom 7.1.3 is from 2023-01-15 and nom 8.0.0 from 2025-01-26 (crates.io API). organic 0.1.16 (2024-04-12) is still the newest release.

**Corrections.**
1. **P6 does not remove exponential time. At least three more classes survive P1–P6.** They share P6's root cause: an opener scans ahead, fails to close, backtracks, and the plain-text loop then retries the inner opener. The rows below come from the patched build's `try` binary, with Emacs `org-element-parse-buffer` timed the same way as the 73-byte case.

   | Input | Bytes | organic + P1–P6 | unpatched | Emacs 9.8.7 |
   |---|---|---|---|---|
   | `[fn:: ` × 14 | 84 | **11,037 ms** (about 3× per repeat) | 21,249 ms | 0.03 ms |
   | `a_{b ` × 20 (same for `a^{b `) | 100 | **1,741 ms** (2× per repeat); × 40 > 60 s | — | 0.11 ms (× 40) |
   | `Let x_{i be y and ` × 20 (prose with unclosed subscripts) | 360 | **2,696 ms** | — | — |
   | `See[fn:: a note ` × 10 | 160 | **111 ms** | — | — |
   | `[fn:x: ` × 14 | 98 | 50 ms (2× per repeat) | 105 ms | — |

   Quadratic paths also remain. `[[a][` × 800 (4.8 KB) took 316 ms, `[[a]` × 800 233 ms, `[cite:@a ` × 800 168 ms, `{{{m(` × 800 64 ms, `<%%(a` × 800 64 ms, `\(a` × 800 50 ms and `src_c{` × 800 37 ms. Any of these breaks a per-KB time budget. This matters for the #11 plan, because it re-parses Posts while they are being typed, and a half-typed Post routinely contains unclosed openers. The fork needs close prechecks like P6 for each recursive object type: footnote references, sub/superscripts, links, citations, macros, inline source and babel calls, LaTeX fragments and diary timestamps. It also needs a fuel limit as a backstop. That adds an estimated 4–6 days, so the fork total becomes ≈14–20 days. The "When to revisit" trigger was reworded to match.
2. **The patches are not provably output-preserving.** A held-out run on 5,000 new fuzz documents (`gen_fuzz.py` seed 777) found **1 AST difference**. It minimised to `[[1][~]\n\nf~]]`. Unpatched organic parses this as one paragraph that spans the blank line, with a link whose description holds `Code "~]\n\nf~"`. The patched build has plain text there, because P6 assumes markup cannot cross a blank line. Emacs makes two plain paragraphs, so both builds are wrong. Emacs fidelity does not get worse, but "byte-identical output" holds only on the tested sets. A held-out real corpus showed no difference: 311 `.org` files from the doomemacs, emacs-straight/org-mode and spacemacs repositories, 3.18 MB, all parsing ok in both builds.
3. **New fidelity bug in organic (with or without the patches): regular-link descriptions can span blank lines.** For example, `x [[a][b\n\nc]] y` becomes one paragraph containing a link. Emacs ends the paragraph at the blank line.
4. **`#+SETUPFILE` values are not unquoted.** organic passes `kw.value` to `read_file` as it is, so `#+SETUPFILE: "s.org"` fails with `IO(NotFound)` while `#+SETUPFILE: s.org` works. Emacs applies `org-strip-quotes` (org.el 4727) and also accepts URLs. The Engine's `FileAccessInterface` can strip the quotes itself, so this still needs no fork change, but it is a requirement that was missing.
5. **The upstream is not entirely dormant.** The parser has been frozen since 2024-04, but the author made 16 commits in 2025–2026. They are build, CI, Nix and Dockerfile work, the latest being `336b5d3` on 2026-07-17, and include pinning `nightly-2026-05-23`. So the forge is maintained. The parser, however, is not.
6. **Small numeric differences.**
   - The first commit's author date is 2022-07-15; 2022-07-16 is its commit date.
   - October 2023 has 407 `src/` commits by author month, not 408.
   - Per-file percentiles depend on the percentile method: one gives p90/p99 of 10.4/31.6 ms unpatched and 2.12/18.2 ms patched, against the reported 10.6/52.6 and 2.13/19.7.
   - "99.99% of nodes", inherited from the org-parser research, is **node-type count agreement** on the manual, guide and syntax spec. It is not tree equality, and it does not detect issues such as correction 3.

**Newly surfaced questions.**
- Should the Engine fall back to rendering a paragraph as plain text, with a warning, when the fuel limit is hit? Or should the whole file fail?
- The fork's fuzz harness should include an adversarial generator for unclosed openers (the `adv.py` style), not only random Org. The random generator never produced the cases above.
