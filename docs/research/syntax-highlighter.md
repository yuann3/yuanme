# Syntax highlighter for the Engine

Ticket: [#5](https://github.com/yuann3/yuanme/issues/5). Researched 2026-10-04.

## Question

Which Rust syntax highlighter should the Engine use? It needs Catppuccin-quality output, class-based HTML for light and dark themes, the languages programmers and mathematicians write (C, Rust, shell, Haskell, Lean, Coq, Julia, Python), and enough fidelity to Shiki's TextMate output to keep the Reference site at parity.

## Short answer

**Use [giallo](https://github.com/getzola/giallo).** Pin its registry to the grammar and theme JSON that Shiki 3.19 ships, and give the Engine its own renderer that emits one short CSS class per colour combination.

giallo ports vscode-textmate's tokenizer to Rust ([src/tokenizer/mod.rs](https://github.com/getzola/giallo/blob/master/src/tokenizer/mod.rs) line 1: "This file replicates the logic of vscode-textmate"). It loads the same curated grammars and themes Shiki uses ([README](https://github.com/getzola/giallo#giallo)).

In a spike on every code fence in eyuan.me plus Shiki's own samples for ten languages:
- Its colours matched Shiki 3.19 on **100% of characters in both Catppuccin Latte and Mocha** for every language with a matching theme version.
- A small token-merge pass in the renderer brought it to **664/664 lines identical span for span**.
- The one mismatch, Haskell, came from a newer Catppuccin theme file in giallo's built-in dump. Loading Shiki 3.19's exact theme JSON made Haskell 100% too.

No other candidate comes close on fidelity:
- syntect plus the official Catppuccin tmTheme matched 80% of characters on the Reference site's code.
- arborium's tree-sitter Catppuccin matched 37%.

What giallo costs:
- **Licence.** It is EUPL-1.2, a weak copyleft licence (see Open questions).
- **Regex engine.** It depends on an Oniguruma (C) fork.
- **Bus factor.** It has effectively one maintainer.
- **Speed.** It is about 2× slower than syntect, though still about 3.5× faster than Shiki in Node.

## Comparison

All numbers marked † come from the spike described under Method. "Fidelity" means the share of non-whitespace characters whose light *and* dark colours equal Shiki 3.19 + Catppuccin Latte/Mocha.

| | **giallo 0.5.2** | syntect 5.3.0 (+ two-face 0.5.2) | arborium 2.18.2 | tree-sitter-highlight 0.27.0 | lumis 0.16.0 |
|---|---|---|---|---|---|
| Engine | vscode-textmate port, TextMate JSON grammars | Sublime `.sublime-syntax` engine | tree-sitter + query captures | tree-sitter (bring your own grammars) | tree-sitter + Neovim themes |
| Fidelity to Shiki, Reference site code (c, bash, sh, rust)† | **100%** (299/299 lines) | 79.8% (175/299 lines) | 36.7% (30/299 lines) | not run (same model as arborium) | not run (same model as arborium) |
| Fidelity, Shiki samples† | 100% for c, rust, shellscript, lean, coq, julia, python, latex, ocaml. Haskell 31.8% with the built-in theme, **100% with Shiki 3.19's theme JSON** | 18–98% by language. No Coq (`v` resolves to Verilog) | 27–67%. No Coq, no LaTeX | | |
| C / Rust / shell | yes / yes / yes | yes / yes / yes | yes / yes / yes | crates exist | yes / yes / yes |
| Haskell / Lean / Coq / Julia / Python | yes / yes (`lean`, `lean4`) / yes / yes / yes | yes / yes (two-face "Lean 4") / **no** / yes (two-face) / yes | yes / yes / **no** / yes / yes | yes / `tree-sitter-lean4` 0.3.0 / **no crate** / yes / yes | yes / **no** / **no** / yes / yes |
| Language count | 220+ built in ([README](https://github.com/getzola/giallo#grammars)) | ~75 default, more with two-face | 112 `lang-*` features (counted in repo) | one crate per grammar | ~110 ([README](https://github.com/leandrocp/lumis#features)) |
| Class-based HTML | yes: `css_class_prefix` plus `generate_dual_css` ([html.rs](https://github.com/getzola/giallo/blob/master/src/renderers/html.rs)). Tokens are also exposed for a custom renderer | yes: `ClassedHTMLGenerator` plus `css_for_theme_with_class_style` ([html.rs](https://github.com/trishume/syntect/blob/master/src/html.rs)), with nested scope classes | yes: `HtmlFormat::ClassNames` or custom elements (`<a-k>`) | via attribute callback | `HtmlLinked` plus per-theme CSS files |
| Catppuccin Latte/Mocha | built in, the same `tm-themes` JSON Shiki uses | official [catppuccin/bat](https://github.com/catppuccin/bat) tmTheme | hand-written ~35-slot TOML ([catppuccin-mocha.toml](https://github.com/bearcove/arborium/blob/main/crates/arborium-theme/themes/catppuccin-mocha.toml)) | DIY | from the Neovim port |
| Speed: 62 blocks, 18.8 KB, per pass† | 14.5 ms (dual theme to HTML) | 7.6 ms (classed HTML, single pass) | 3.1 ms | | |
| Stripped release binary, LTO† (base 0.30 MB) | **2.55 MB** with all 220+ grammars and ~60 themes | 1.30 MB default set; 1.92 MB with two-face | 5.7 MB c+rust+bash; **20.0 MB Lean alone**; 33.5 MB for 8 langs; **168 MB** all languages | similar per-grammar cost | similar per-grammar cost |
| Licence | **EUPL-1.2** | MIT | MIT OR Apache-2.0 (grammars vary) | MIT | MIT |
| Maintenance (2026-10-04) | 5 releases Jun–Aug 2026; last commit 2026-09-05; 132★; Keats wrote 57 of 65 commits; Zola depends on it | last release 5.3.0 on 2025-09-27; master last pushed 2026-04-28 with unreleased breaking changes; 2.4k★, 140 open issues | active, last release 2026-08-28; 503★ | 0.27.0 on 2026-08-30 | very active (commits 2026-10-03) |

Sources for the maintenance row: the crates.io API (`/api/v1/crates/<name>`) and the GitHub API (`repos/<owner>/<repo>`, `/contributors`), both queried 2026-10-04.

Two newer TextMate options were also checked and rejected:
- **[shiki](https://crates.io/crates/shiki) 0.0.7** (2026-07-26, MIT/Apache) is a TextMate engine on Oniguruma. It has 0 GitHub stars, ~570 downloads, and docs.rs reports 13% documentation coverage ([docs.rs](https://docs.rs/crate/shiki/latest)). Too young.
- **[zalo](https://crates.io/crates/zalo) 0.3.18** is a one-person EUPL-1.2 fork of giallo with 0 stars. No reason to prefer it over upstream.
- **[ferriki](https://crates.io/crates/ferriki) 0.10.0**, **[irosashi](https://crates.io/crates/irosashi) 0.2.0** and **[syntaxmate](https://crates.io/crates/syntaxmate) 0.2.1** were found during verification. All are MIT or MIT/Apache TextMate engines, weeks old, with 0 to 3 stars. ferriki was spiked; see Verification.

## Details

### Fidelity: why TextMate, and why giallo

The Reference site renders code with Shiki 3.19.0 and dual Catppuccin themes (`themes: { light: 'catppuccin-latte', dark: 'catppuccin-mocha' }`, eyuan.me `astro.config.mjs`). Its token boundaries come from VS Code TextMate grammars (`astro-parity-inventory.md` §4, "Shiki dual-theme markup"). Only an engine that runs the *same grammar files with the same algorithm* can match it. Of the candidates, only giallo does that.

- **giallo** uses the grammars and themes from [shikijs/textmate-grammars-themes](https://github.com/shikijs/textmate-grammars-themes), the packages Shiki itself bundles ([README](https://github.com/getzola/giallo#giallo)). Its tests compare tokenization and highlighting against output generated with `microsoft/vscode-textmate` for every grammar sample: `can_tokenize_like_vscode_textmate` and `can_highlight_like_vscode_textmate` in [src/registry.rs](https://github.com/getzola/giallo/blob/master/src/registry.rs), with snapshots from [scripts/generate-snapshots.js](https://github.com/getzola/giallo/blob/master/scripts/generate-snapshots.js). Zola's maintainer built it to replace syntect in Zola; he says it "output[s] the exact same thing as you see in VSCode" ([Zola forum, 2025-12-04](https://zola.discourse.group/t/new-syntax-highlighter/2885)). Zola 0.22 and later depend on `giallo = "0.5"` ([zola Cargo.toml](https://github.com/getzola/zola/blob/master/Cargo.toml)).
- **syntect** runs Sublime Text syntax definitions ([README](https://github.com/trishume/syntect#readme)), not the VS Code grammars Shiki uses. The bundled set is old: giallo's README notes the bundled Rust syntax predates async/await. The spike paired the official Catppuccin tmTheme with two-face's bat syntax set and got 79.8% character agreement on the Reference site's code. That is visible drift on every page, not span-level noise.
- **Tree-sitter highlighters** (arborium, lumis, raw tree-sitter-highlight) classify by parse-tree captures, mapped onto a small set of theme slots. arborium maps every capture name onto ~20 slots ([`capture_to_slot`](https://github.com/bearcove/arborium/blob/main/crates/arborium-theme/src/highlights.rs)). Their Catppuccin is a re-interpretation, not the VS Code theme. They are more *semantically* accurate in places, but can never be span-identical to Shiki. arborium scored 27–67%.

**The Haskell result shows that grammar and theme versions matter as much as the engine.**
- giallo 0.5.2's dump comes from textmate-grammars-themes commit `4cb1625` (`tm-grammars` 1.32.1). Shiki 3.19's `@shikijs/langs` was built against `tm-grammars ^1.26.0`, and `@shikijs/themes` against `tm-themes ^1.10.13` (their `package.json` files in eyuan.me's `node_modules`).
- The newer Catppuccin theme has 197 token rules; Shiki 3.19's has 179. The newer one recolours Haskell pragmas, for example.
- Running Shiki with giallo's newer theme JSON reproduced giallo's output exactly (100%).
- Loading Shiki 3.19's theme JSON into giallo through `Registry::add_theme_from_path` made giallo match Shiki exactly (100%).
- So the Engine controls parity by choosing which JSON it compiles into its registry.

### Span-for-span parity needs a small Engine-side merge rule

With default options, giallo merges whitespace into the following token (`"    void"`), while Shiki keeps it separate. Only 245 of 664 lines then had identical token lists.

With `merge_whitespace(false)`, and adjacent tokens merged when their (light colour, dark colour, italic) triples are equal, **664/664 lines matched Shiki exactly**. Merging on colour alone left 24 lines different, where an italic comment abuts same-coloured punctuation. That rule belongs in the Engine's own renderer. giallo exposes `HighlightedCode.tokens` (per line, with a light and a dark `Style` for each token) for exactly this ([src/registry.rs](https://github.com/getzola/giallo/blob/master/src/registry.rs) `HighlightedCode`; [examples/custom_rendering.rs](https://github.com/getzola/giallo/blob/master/examples/custom_rendering.rs)).

### Class-based light/dark markup

- giallo's built-in class mode writes two classes per span (`class="g-l-9 g-d-9"`), plus two small stylesheets of one rule per distinct colour (830 bytes each for Latte and Mocha†) ([src/themes/css.rs](https://github.com/getzola/giallo/blob/master/src/themes/css.rs)).
- Its default inline mode uses CSS `light-dark()` ([README, HTML renderer](https://github.com/getzola/giallo#html-renderer)). The README says this does not suit a manual light/dark switch, but `light-dark()` follows the element's `color-scheme`, so a toggle that sets `color-scheme` does drive it (see Verification). An [open issue (#61)](https://github.com/getzola/giallo/issues/61) asks for `light-dark()` in generated CSS.
- On the Reference site's largest code page (`/blog/extending-c-stdlib/`, 29 blocks), the code markup measured†:
  - Shiki today (inline `style` with `--shiki-dark` vars): 105,814 B.
  - giallo inline `light-dark()`: 98,216 B.
  - giallo class mode: 63,698 B.
  - A custom renderer with one class per (light, dark, italic) combination (only 11 combinations on that page) and default-coloured text left unwrapped: **~40,250 B (−62%)**.
- Keeping the `astro-code` wrapper and the `--shiki-*` variable names, or renaming CSS and markup together, is a parity decision. It is not a highlighter limitation.

### Speed

Measured on this Mac (Apple Silicon), release builds, 20 passes over the same 62 blocks (18,834 chars)†:
- giallo: 14.5 ms per pass (registry load 12 ms, once).
- syntect: 7.6 ms (classed output, which skips theme resolution).
- arborium: 3.1 ms.
- Shiki 3.19 `codeToHtml` in warm Node: 51.8 ms.

That giallo runs about 2× slower than syntect matches its maintainer's measurements. He attributes it to the VS Code algorithm making about 2× more regex calls, with ~80% of the time spent in Oniguruma ([giallo#40](https://github.com/getzola/giallo/issues/40)). The same issue records a per-block regex-compile cost that was fixed in PR #41: "a few ms for the first codeblock … the next ones should be in the microsecond range".

`giallo::Registry` is `Send + Sync`† (it uses the `papaya` concurrent map, per its [Cargo.toml](https://github.com/getzola/giallo/blob/master/Cargo.toml)), so the Engine can highlight pages in parallel from one registry. At about 0.8 ms per KB of code single-threaded, highlighting will not dominate build time.

### Binary size

giallo's built-in dump is a zstd-compressed bitcode file of 220+ grammars and ~60 themes, which the README puts at 1.14 MiB ([README](https://github.com/getzola/giallo#installation)). The measured binary cost was +2.25 MB over an empty binary†.

Tree-sitter's cost is per grammar and can be enormous:
- arborium's Lean grammar ships a 106 MB `parser.c`, and Lean alone produced a 20 MB binary†.
- Every permissively licensed arborium grammar together came to 168 MB†.
- giallo's README calls this out ("100MB+ … for ~50 languages, compared to ~1MiB for 4x more languages") as the reason it abandoned tree-sitter ([README, "Why not tree-sitter"](https://github.com/getzola/giallo#tree-sitter)).

Authors install one prebuilt binary (ADR 0002, ADR 0003), so this matters. It rules out shipping a broad tree-sitter language set.

### Maintenance health

- **giallo** is young: the repo was created 2022, active since late 2025. It has had steady releases: 0.4.0 (2026-06-01), 0.4.1 (06-22), 0.5.0 (07-17), 0.5.1 (08-03), 0.5.2 (08-06), plus a commit on 2026-09-05 (crates.io API; git log).
  - It is load-bearing for Zola, which keeps it alive, but it is a one-maintainer project: Keats wrote 57 of 65 commits (GitHub contributors API).
  - It depends on `onig-regset`, a fork of rust-onig, until an upstream PR lands ([README](https://github.com/getzola/giallo#installation); [rust-onig#210](https://github.com/rust-onig/rust-onig/pull/210)). That means a C compiler at Engine build time, but nothing for Authors, who get a prebuilt binary.
  - Open issues are feature requests plus the performance thread above ([issues](https://github.com/getzola/giallo/issues)).
- **syntect** is mature and widely used (30M downloads) but releases slowly. Nothing has been released since 5.3.0 (2025-09-27), while master carries unreleased breaking changes adding sublime-syntax v2 features ([CHANGELOG, Unreleased](https://github.com/trishume/syntect/blob/master/CHANGELOG.md)).
- **arborium** and **lumis** are actively maintained (crates.io; git log), but their architecture fails the fidelity and size requirements regardless.

## Recommendation for the Engine

1. Depend on `giallo` (0.5.x) without its built-in dump, or with it as a fallback.
   - Build the Engine's own registry at *Engine* build time from a pinned set of TextMate grammar and theme JSON, using `add_grammar_from_path` / `add_theme_from_path` and `dump()`, and embed it. No Node is involved for Authors, which keeps ADR 0003 intact.
   - For the Reference site, pin Catppuccin Latte/Mocha (and ideally the grammars) to the versions Shiki 3.19 bundles. That gives exact parity.
2. Write the Engine's own HTML renderer over `HighlightedCode.tokens`.
   - It merges adjacent tokens on (light fg, dark fg, font style) and emits short classes plus one generated stylesheet per Site.
   - It handles `plaintext`, trailing-newline stripping and the `<span class="line">` structure the inventory specifies.
3. Let Authors add VS Code / TextMate JSON grammars and themes from their Site directory at runtime through the same `add_*_from_path` calls.

Fallback if the EUPL licence is rejected: syntect + two-face + the official Catppuccin tmThemes, with Reference site highlighting declared a Parity exception (~80% colour agreement). Revisit the MIT/Apache `shiki` and `ferriki` crates once they mature. ferriki already matches Shiki span for span but is currently far too slow (see Verification).

## Method (spike, throwaway, in `/private/tmp/hlspike`)

1. Extracted all 52 code fences from eyuan.me `src/content/{blog,project}` (26 `c`, 15 `bash`, 3 `sh`, 1 `rust`, 7 bare; the 7 bare fences were excluded from scoring). Added Shiki's own samples for c, rust, shellscript, haskell, lean, coq, julia, python, latex and ocaml (62 blocks in all), from [textmate-grammars-themes `samples/`](https://github.com/shikijs/textmate-grammars-themes/tree/main/samples) at `37edd1b`.
2. Tokenized them with eyuan.me's installed Shiki 3.19.0 (`codeToTokens`, dual Catppuccin) as the reference.
3. Tokenized the same blocks in Rust with:
   - giallo 0.5.2 (built-in dump, dual theme);
   - syntect 5.3.0 with `two_face::syntax::extra_newlines()` and [catppuccin/bat](https://github.com/catppuccin/bat) Latte/Mocha tmThemes;
   - arborium 2.18.2 with its built-in `catppuccin_latte` / `catppuccin_mocha`, spans painted through `capture_to_slot`.
4. Compared light and dark colour per non-whitespace character, then exact per-line token lists.
5. Built release binaries (`lto = true`, `codegen-units = 1`, `strip = true`) that each highlight one string, to measure size.

All on rustc 1.99.0, macOS arm64.

## Open questions

- **Is an EUPL-1.2 dependency acceptable for an MIT/Apache Engine?**
  - The European Commission's guidance says EUPL has no "viral effect" through static or dynamic linking, and that what counts as a derivative work follows EU copyright law ([EUPL overview, Interoperable Europe](https://interoperable-europe.ec.europa.eu/sites/default/files/news/2020-12/EUPL%20overview%202020.pdf); [EUPL v1.2 rationale](https://joinup.ec.europa.eu/sites/default/files/document/2014-02/Rationale%20for%20the%20EUPL%20v1.2.%20v06.pdf)).
  - Even so, any modified giallo must stay EUPL, and the binary's licence notices must include it.
  - This needs the owner's sign-off. It is not settled here.
- **Pin or track?** Should the Engine pin Shiki 3.19's grammar and theme versions forever for the Reference site, or track `tm-grammars`/`tm-themes` and record colour changes (such as Haskell pragmas) as Parity exceptions?
- **Markup.** Should the Reference site switch from Shiki's inline-style markup to class-based markup (−62% code markup on the largest page)? This is inventory open question 18, now answerable in favour of classes.
- **Org source blocks.** How do Org `#+BEGIN_SRC` language names map onto TextMate grammar names and aliases (for example `emacs-lisp`, `sh`, `lean4`)?
- **Upstream dependence.** Should the Engine depend on giallo's crates.io releases, or vendor it given the single maintainer and the `onig-regset` fork?

## Verification

Independent re-check on 2026-10-04. The recommendation holds. Every key number was reproduced from the spike's outputs, and every external fact was re-read from its primary source. Corrections and additions:

**Confirmed as stated.**
- Fidelity: re-running `compare.py` gives giallo 100% on eyuan.me code (299/299 lines) and on every sample except Haskell (31.8%), syntect 79.8%, arborium 36.7%, with arborium missing Coq and LaTeX.
- Haskell: re-running the `pin` binary (Shiki 3.19's Catppuccin JSON loaded with `add_theme_from_path`) gives 368/368 characters.
- The Shiki reference is the only Shiki in eyuan.me (`node_modules/shiki` 3.19.0). `@shikijs/langs` pins `tm-grammars ^1.26.0`, `@shikijs/themes` pins `tm-themes ^1.10.13`.
- Binary sizes match the files in `size/target/release` (base 302,736 B, giallo 2,553,216 B, syntect + two-face 1,924,960 B, arborium Lean 19,990,080 B, all languages 167,672,448 B).
- Markup: `dist/blog/extending-c-stdlib/index.html` has 29 `astro-code` blocks totalling 105,814 B.
- crates.io: giallo 0.5.2 (EUPL-1.2, releases 06-01, 06-22, 07-17, 08-03, 08-06); syntect 5.3.0 on 2025-09-27 and nothing since; arborium 2.18.2 on 2026-08-28; tree-sitter-highlight 0.27.0 on 2026-08-30. No `tree-sitter-coq` or `tree-sitter-rocq` crate exists.
- GitHub: giallo has 132 stars and Keats has 57 of 65 commits; the last commit is 2026-09-05. Zola's workspace `Cargo.toml` (0.23.6) depends on `giallo = {version = "0.5", features = ["dump"]}`. syntect's CHANGELOG "Unreleased" section lists breaking changes.
- The giallo README, `src/tokenizer/mod.rs` line 1, `HtmlRenderer.css_class_prefix`, `Registry::generate_dual_css`, `merge_whitespace`, `add_*_from_path`, `dump`, and the `can_tokenize_like_vscode_textmate` / `can_highlight_like_vscode_textmate` tests all exist as cited.

**Corrections and caveats.**
- **664/664 leaves out Haskell.** The non-plaintext corpus has 687 lines. 664 is that total minus the 23-line Haskell sample, which used giallo's newer built-in theme. With the built-in themes, the span-level count is 674/687.
- **Why the merge rule works.** Shiki 3.19's dual-theme tokens never contain two adjacent tokens with the same (light, dark, italic) style: 0 such pairs in the whole corpus. Equal-style neighbours are merged before Shiki emits them. So merging giallo's tokens on exactly that triple reproduces Shiki's token list. Any further merging in the Engine, such as leaving default-coloured text unwrapped, saves markup but breaks the one-to-one match with Shiki's spans. That is a parity decision.
- **The speed numbers are not like-for-like.** giallo was timed rendering dual-theme inline HTML. syntect was timed producing classed HTML, which does no theme resolution. arborium was timed on `highlight` alone. The "about 2×" in [giallo#40](https://github.com/getzola/giallo/issues/40) comes from the jQuery file. On a 300-line TypeScript sample the same thread reports 42.6 ms for giallo against 7.7 ms for syntect (with its JS syntax), about 5.5×. Expect 2 to 5× depending on the grammar. That is still small for the Engine's workloads.
- **`light-dark()` can follow a manual toggle.** CSS `light-dark()` resolves against the element's used `color-scheme` ([MDN](https://developer.mozilla.org/en-US/docs/Web/CSS/color_value/light-dark)). A toggle that sets `color-scheme: light` or `dark` on the root therefore drives giallo's inline mode. The README's warning is about `prefers-color-scheme`-only setups. Classes remain the better choice for markup size.
- **Missed option: ferriki.** [ferriki](https://github.com/sebastian-software/ferriki) 0.10.0 (MIT OR Apache-2.0) is a Rust TextMate engine on [ferroni](https://github.com/sebastian-software/ferroni), a pure-Rust Oniguruma (BSD-2-Clause), so it needs no C compiler. It is tested against a pinned vscode-textmate oracle and Shiki's own test suite, and it has a dual-theme token API (`Highlighter::highlight_with_themes`). A spike on the same 62 blocks (`/private/tmp/hlspike/ferspike`, its repo's `assets/shiki` at tag v0.10.0) found:
  - 100% colour-and-italic agreement with Shiki 3.19, and raw tokens identical span for span with no merge pass (except Haskell at 31.8%, the same theme-version difference as giallo).
  - About **3 s per pass** against giallo's 14.5 ms, roughly 200× slower. The 294-byte Julia sample alone took about 5 s. The C blocks from eyuan.me took 70 to 150 ms each.
  - Its `Highlighter` is `!Send`, so each thread needs its own.
  - It was first published to crates.io on 2026-09-28, had 11 releases in 6 days, has 1 GitHub star, and one human author (271 commits, plus 41 co-authored by Claude).
  - Not viable today. It is the strongest licence-clean alternative to watch, and if its speed is fixed it would beat syntect as the EUPL fallback, since it keeps exact parity.
- **Other new crates.** [irosashi](https://github.com/frostybee/irosashi) 0.2.0 (MIT, first published 2026-09-16, 0 stars, one author) and [syntaxmate](https://github.com/phongndo/syntaxmate) 0.2.1 (MIT, 2026-08-02, 3 stars, one author) also claim TextMate/VS Code output. Both are too young to assess. They were not spiked.

