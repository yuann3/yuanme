# Is fixing organic's performance cheaper than writing our own Org parser?

Ticket: [#30](https://github.com/yuann3/yuanme/issues/30) (wayfinder research). It follows up open question 7 of the Org parser research ([#3](https://github.com/yuann3/yuanme/issues/3), [`docs/research/org-parser.md` on `research/org-parser`](https://github.com/yuann3/yuanme/blob/research/org-parser/docs/research/org-parser.md)). Researched 2026-10-04. Vocabulary follows [`CONTEXT.md`](../../CONTEXT.md).

## Question

The Org parser research recommended writing our own parser, with a vendored fork of organic (0BSD) as the fallback. organic agrees with Emacs `org-element` on 99.99% of nodes, but it is 50–100× slower than the alternatives and needs nightly Rust. This ticket asks:

- Why is organic slow? Profile it on a realistic corpus.
- What does it cost to make a fork build on stable Rust, run fast enough for sub-100 ms rebuilds, expose byte spans for diagnostics, and stop failing hard on a missing `#+SETUPFILE`?
- How does that compare with writing our own parser? Fork or write?

## Short answer

**Fork organic.** This reverses the primary recommendation of the Org parser research. Its two blocking objections, speed and nightly Rust, turned out to be cheap to fix, and the fork starts at the best fidelity available.

- **Why it is slow.** The time goes into organic's architecture, not into one hot function. Inside plain text, organic advances one character at a time. At every character it walks the whole stack of "exit matchers" and tries all ~20 object parsers. One of those exit matchers, `paragraph_end`, fully parses a candidate element at every character. Measured over the 293-file Worg corpus, that comes to roughly one full element-parse attempt per character of plain text. The author's own notes name the same two costs ([`notes/optimization_ideas.org`](https://code.fizz.buzz/talexander/organic/src/branch/main/notes/optimization_ideas.org)).
- **What a few hours of patching bought.** Six small patches (127 added lines in 5 files) made organic **4.1× faster** on Worg: 1,489 → 362 ms, or 237 → 58 µs/KB. A 16 KB file now parses in 1.5 ms instead of 6.3 ms, and the 887 KB Org manual in 68 ms instead of 341 ms. The output was byte-identical, by AST hash, on all 627 corpus, sample and synthetic files and on 2,000 fuzz documents. organic is still about 12× slower than orgize, but that is far inside the budget. The [#11](https://github.com/yuann3/yuanme/issues/11) plan re-parses only the edited Post and leaves about 55 ms for the browser.
- **The real risks are worse than slowness: exponential time and crashes.**
  - A 73-byte line of unclosed markup (`*a /b ` repeated 12 times) takes organic **88 seconds**, against 0.14 ms in Emacs. Patch P6 makes it 0.17 ms.
  - A 2-minute fuzz run found two crash classes in unpatched organic. A stray `:END:` line followed by a timestamp or a link **panics**. A line containing only `|` makes the **whole document fail to parse**. Emacs parses both.
  - The fork therefore needs a fuzz harness with a time budget, crash fixes and a `catch_unwind` boundary. Those are the largest line items in the estimate below.
- **The other asks are nearly free.**
  - **Stable Rust**: done in the earlier spike. It needed 3 compile errors fixed and one manifest line removed, plus the two `#[bench]`s and the wasm-only `gloo-utils` dependency removed.
  - **Byte spans**: every node's `source` is a slice of the input. All 173,083 nodes in the corpus check out, so a span is pointer arithmetic.
  - **Missing `#+SETUPFILE`**: needs **no fork change at all**. The Engine supplies its own `FileAccessInterface` that records the miss and returns empty contents. A spike reported `org-guide.org:2645: #+SETUPFILE "doc-setup.org" not found (bytes 86676..86702)` instead of aborting.
- **Cost.** Making the fork production-ready is about **2–3 focused weeks**. Writing our own is about **6–12 weeks**, with much more fidelity risk. organic's author needed 21 months and about 1,500 source commits to reach parity.

**When to revisit:** two conditions would reopen "write our own":

- Fuzzing with a time budget keeps finding new superlinear or crash classes after the first round of fixes.
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
| Known defects | 1 exponential-time class (fixed by P6), 2 crash classes (unfixed), no inlinetask, empty dynamic block, `$n$-th` | unknown until written |
| Code we own | about 17.4k lines of nom 7 combinators with 4 lifetimes (`'b 'g 'r 's`) | estimated 5–8k lines (org-parser research) |
| Effort to production-ready (estimate) | **about 10–14 focused days** | **about 30–60 focused days** |
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
| Re-pin the oracle to Org 9.8.7 and fix divergences such as `$n$-th` | 2–4 | included below |
| Recognise all 30 element and 24 object types with spans (5–8k lines) | — | 15–25 |
| Long-tail fidelity from first pass to ≥99.9% on the corpus | — | 10–30 |
| **Total** | **≈10–14** | **≈30–60** |

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
3. **Do we upstream the patches?** The forge is self-hosted and dormant since 2024-04, and 0BSD does not require it. Upstreaming would be a courtesy, not a dependency.
4. **Token-level spans or a lossless CST?** If agent edits or [#21](https://github.com/yuann3/yuanme/issues/21) (the agent-native surface) need exact source rewriting of Org, organic's node-level slices may not be enough. That is the clearest trigger for revisiting "write our own".
5. **nom 7 or nom 8?** Staying on nom 7.1.3 costs nothing today. A port is mechanical but touches most of the 12k parser lines, so [#27](https://github.com/yuann3/yuanme/issues/27) (dependency policy) should say whether an unmaintained major version is acceptable.
6. **Which Org version is the oracle?** organic tests against Org `main` of 2023-10-13, and the Engine wants Org 9.8.7. Re-pinning means running organic's compare tooling against a newer Emacs and fixing the diffs. The size of that job is unknown until it is run.
