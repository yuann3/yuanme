# Build-time math: MathML Core vs KaTeX-style HTML

Ticket: [yuann3/yuanme#4](https://github.com/yuann3/yuanme/issues/4). Researched 2026-10-04.

## Question

How should the Engine render LaTeX math at build time so that a Site ships zero client JS for it? The two candidate output formats are **MathML Core** (from pulldown-latex, latex2mathml, math-core, or a Temml-equivalent) and **KaTeX-style HTML+CSS** (from katex-rs, or KaTeX itself in an embedded JS engine, keeping ADR 0003 in mind). For each, we checked LaTeX coverage for mathematicians, `\label`/`\ref` numbering, `\newcommand`, output size, fonts, browser support and rendering quality, accessibility, and speed.

## Short answer

**Output MathML Core and render it with [`math-core`](https://crates.io/crates/math-core) 0.8.x.**

- It is the only Rust crate that resolves equation numbers and `\eqref` (forward references too) at build time. The numbers are written as plain text, with no CSS counters and no client JS ([lib.rs `convert_all`](https://docs.rs/math-core/0.8.2/math_core/struct.LatexToMathML.html#method.convert_all), spike below).
- It is the fastest option we measured (about 1.9 µs per expression).
- It produces the smallest markup: about 9x smaller than KaTeX HTML+MathML before gzip, about 5x after.
- It adds about 230 KB to a stripped release binary.
- It is pure Rust, so ADR 0003 holds.
- It is actively maintained: 100 commits since April and 8 releases since 2026-04-22 ([releases](https://github.com/tmke8/math-core/releases)).
- Rustdoc picked it for its own LaTeX math feature ([RFC 3958](https://github.com/rust-lang/rfcs/blob/master/text/3958-rustdoc-texmath.md), merged 2026-09-06).

MathML has been Baseline "widely available" since 2025-07-12 (Chrome/Edge 109+, Firefox, Safari) ([web-features](https://web-platform-dx.github.io/web-features-explorer/features/mathml/)). Browsers also expose it to screen readers as real math.

What it costs:

1. **Coverage gaps.** Some amsmath environments and commands that mathematicians use are missing: `split`, `alignat`, `CD`, `smallmatrix`, `\substack`, `\widetilde`, `\DeclareMathOperator`, plain `\ref` (only `\eqref` exists), and `\tag` outside a numbered environment. They fail loudly with a source span, which fits our "outside the subset is a build error" rule, and most have workarounds (see [Gaps](#coverage-gaps-in-math-core-and-workarounds)).
2. **A math font and about 2.4 KB of CSS.** The Engine must ship a math font (`@font-face` plus a subset WOFF2, about 21 KB for our 40-expression corpus) and math-core's `mathmlfixes.css`.
3. **Browser rendering differences.** Rendering is good but not pixel-identical across browsers. Minor MathML layout bugs remain in all three engines ([Temml status page](https://temml.org/docs/en/mathml-status), updated 2026-04-09; [math-core#209](https://github.com/tmke8/math-core/issues/209)), and "Mathematics Rendering" was **not** selected for Interop 2026 ([interop#1140](https://github.com/web-platform-tests/interop/issues/1140)).

**Fallback, not default:** [`katex-rs`](https://crates.io/crates/katex-rs) 0.3.0, a pure-Rust port of KaTeX, set to HTML+MathML output.

- **For:** pixel-identical layout everywhere and slightly wider coverage (30/40 vs 28/40 in our corpus).
- **Against:** no `\label`/`\ref`/`\eqref` at all, numbering done with CSS counters, 5–9x more markup, and the KaTeX CSS (25 KB) plus up to 20 KaTeX font files (260 KB total, fetched on demand).
- **Bus factor:** a single contributor.
- **Dependency clash:** it cannot share a dependency graph with math-core 0.8.2 today (phf build failure, below), so the Engine cannot ship both as switchable backends without upstream fixes.

**Rejected:**

- **KaTeX or Temml in QuickJS.** 170–400x slower than math-core, a C-built JS engine inside the binary, and against the spirit of ADR 0002/0003. Temml also needs client JS for `\ref`/`\eqref`.
- **pulldown-latex.** No `\label`/`\ref`/`\tag`, numbering by CSS counter, and no macro config API.
- **latex2mathml.** Abandoned in 2020, with no `cases`/`aligned`/`equation`.

## Comparison

Spike corpus: 40 mathematician-style expressions (appendix). Bytes are the output for the 20 cases every non-abandoned engine rendered. Times are per expression, averaged over 200 passes on an Apple M4 Max, release build.

| | **math-core 0.8.2** | pulldown-latex 0.8.0 | latex2mathml 0.2.3 | **katex-rs 0.3.0** (HTML+MathML) | KaTeX 0.19.0 in QuickJS | Temml 0.13.5 in QuickJS |
|---|---|---|---|---|---|---|
| Output | MathML Core | MathML Core | MathML | HTML spans + hidden MathML | same as katex-rs | MathML Core |
| Corpus passed | 28/40 | 25/40 | 24/40 | 30/40 | 30/40 | 37/40 |
| align / gather / multline / cases / matrices / array | yes / yes / yes / yes (+dcases) / yes / yes | yes / yes / yes / yes / yes / yes | no environments except matrix-like | yes / yes / **no multline** / yes / yes / yes | as katex-rs | all yes |
| split / alignat / CD / smallmatrix | no / no / no / no | yes / yes / no / yes | no | yes / yes / yes / yes | as katex-rs | yes |
| Equation numbers | **static text "(n)" emitted at build time** | CSS counter in `styles.css` | none | CSS counter (`.eqn-num::before`) | CSS counter | CSS counter |
| `\label` / `\eqref` / `\ref` | **yes / yes (with `<a href>`, forward refs) / no** | no / no / no | no | no / no / no | no / no / no | yes, but refs are empty `<a>` filled by client-side `temml.postProcess` |
| `\tag{...}` | inside numbered envs, text only | no | no | yes | yes | yes |
| `\newcommand` / config macros / persist across snippets | yes / yes / yes (`global_group`) | `\newcommand`(*n*-arg) and `\def` / **no API** / no | no | yes / yes / yes (`global_group`) | yes / yes / yes | yes / yes (`definePreamble`) / no for `\newcommand` |
| Errors | `LatexError` with byte span; optional ariadne report | span + context | message | message + position | message + position | message + position |
| Bytes, 20 common cases (raw / gzip) | **11,165 / 1,815** | n/a | n/a | 98,019 / 9,156 | 97,727 / 8,905 | 16,925 / 2,061 |
| Bytes, MathML-only mode | same | | | 19,533 / 3,124 (`OutputFormat::Mathml`) | | |
| Speed per expression | **1.9 µs** | 2.8 µs | 2.3 µs | 37.5 µs (16 µs MathML-only) | 794 µs + 13 ms runtime init | 321 µs + 8 ms init |
| Release-binary cost (stripped, LTO) | **+232 KB** | | | +1.18 MB | +1.30 MB (QuickJS + bundle) | similar |
| Client assets | math font (subset) + 2.4 KB `mathmlfixes.css` | `styles.css` + fonts | font | 25 KB `katex.min.css` + 20 WOFF2 (260 KB total, loaded per use) | same | `Temml-*.css` (9 KB) + font, + JS for refs |
| Maintenance (2026-10-04) | 0.8.2 on 2026-09-01; 1,445 commits by maintainer, 2 regular co-contributors; used by rustdoc RFC | 0.8.0 on 2026-07-28; active PRs | last release 2020 | 0.3.0 on 2026-09-05; 1 contributor | KaTeX 0.19.0 on 2026-10-01 | 0.13.5 on 2026-08-28 |

## Details

### MathML Core with math-core

**API fit.** `LatexToMathML::new(MathCoreConfig)` takes site-level `macros: Vec<(name, body)>`, plus several other options ([lib.rs](https://docs.rs/math-core/0.8.2/math_core/struct.MathCoreConfig.html)):

- `global_group`, so `\newcommand` persists across the snippets of one document.
- `ignore_unknown_commands`, which is false by default, so an unknown command is an error.
- `annotation`, to embed the TeX source.
- `id_prefix`, which namespaces `\label` ids.
- `pretty_print`.

`convert_all(&[(latex, display)])` parses every snippet of a document first and then emits them. That is how it "handles forward references correctly" ([lib.rs](https://docs.rs/math-core/0.8.2/math_core/struct.LatexToMathML.html#method.convert_all)). In the spike, `\eqref{eq:later}` placed before its equation rendered as `<a href="#eq:later">(3)</a>`, and numbered rows carry `id="eq:first"` and the literal text `(1)`. This is the one property no other zero-JS option has. The Engine should call `convert_all` once per document, with `id_prefix` set per page, so that index pages that inline several Posts don't get duplicate ids.

**Coverage.** math-core aims to "support all common LaTeX math commands, at least those that KaTeX supports", produce "concise, readable, and semantically correct MathML", and "definitely don't use JavaScript in any way" ([README](https://github.com/tmke8/math-core#goals)). The missing pieces are tracked in [#154 Missing environments](https://github.com/tmke8/math-core/issues/154) and [#155 Missing commands](https://github.com/tmke8/math-core/issues/155). Our spike confirmed these:

- `equation`, `align(*)`, `gather`, `multline`, `cases`/`dcases`/`rcases`, all `*matrix` variants, `array`, `aligned` and `\sideset` work.
- `split`, `alignat`, `CD`, `smallmatrix`, `\substack`, `\widetilde`, `\varinjlim`, `\DeclareMathOperator` and `\ref` do not.
- `\tag` works only inside numbered environments and only with ASCII text (`\tag{$\ast$}` fails).

**Size.** math-core's output was the smallest of all engines: 11.2 KB raw, 1.8 KB gzip for the 20 common cases. The MathML is pruned (for example `x_{2}` drops the extra `<mrow>`) ([README](https://github.com/tmke8/math-core#alternatives-to-this-library)).

**Fonts and CSS.** The README says LaTeX-like rendering "needs custom math fonts". It ships patched builds of New Computer Modern Math, Libertinus Math and Noto Sans Math in [math-core-fonts](https://github.com/tmke8/math-core-fonts), because of browser bugs: "Chromium does not look at `ssty` variants … Safari displays accents with incorrect vertical space … both Chromium and Safari do not horizontally center certain accents" ([README](https://github.com/tmke8/math-core#css-for-math-font)). `css/mathmlfixes.css` (2,395 bytes) must be included. It resets `<mtd>` padding for Firefox, polyfills `<menclose>` on Chromium, and adjusts accent gaps ([README](https://github.com/tmke8/math-core#css-for-rendering-fixes-and-polyfills)).

Full New Computer Modern Math is "almost 700kB as a `.woff2`". A subset of every glyph math-core can emit is about 450 KB, or about 200 KB without `ssty`, and per-site subsetting is recommended ([README § Font subsetting](https://github.com/tmke8/math-core#font-subsetting)). In our spike, `hb-subset --layout-features=ssty,kern,aalt` of upstream Libertinus Math 7.051 to the 116 characters our corpus uses gave a **21 KB WOFF2 with the MATH table kept** (full font: 380 KB WOFF2). Rustdoc's RFC also lists font choice and subsetting as unresolved ([RFC 3958 § Font](https://github.com/rust-lang/rfcs/blob/master/text/3958-rustdoc-texmath.md)).

**Browser rendering.**

- MathML is supported in Chrome/Edge 109+ (January 2023), Firefox and Safari, and is Baseline widely available since 2025-07-12 ([web-features](https://web-platform-dx.github.io/web-features-explorer/features/mathml/); [caniuse](https://caniuse.com/mathml), 95% global).
- Quality still varies. The Temml status page (updated 2026-04-09) lists open issues, mostly in Chromium and WebKit: supsub alignment, radical placement, `\widehat`/`\widetilde` stretching in Chromium, accents too high and excess delimiter padding in WebKit. Firefox has the fewest issues ([temml.org/docs/en/mathml-status](https://temml.org/docs/en/mathml-status)). math-core keeps its own list ([#209](https://github.com/tmke8/math-core/issues/209)).
- The Interop 2026 proposal to fix MathML presentation tests was declined on 2026-02-12 ([interop#1140](https://github.com/web-platform-tests/interop/issues/1140)), so cross-browser convergence is not funded this year.

**Our Safari check.** We rendered the corpus in WebKit (Safari 27.0.1 engine, macOS 27.0.1) via a `WKWebView` snapshot:

- With the system `math` font, math-core's output looked correct and LaTeX-like. Stretchy braces, matrices, numbering and `\eqref` links all rendered.
- With **unpatched upstream** Libertinus Math 7.051, WebKit did **not** stretch `{`, `(` or `[` around 2-row `cases`/`pmatrix`/`array`, and placed superscripts too high.

So the font choice is load-bearing, and the Engine should default to a math-core-fonts build and test it in all three engines. We could not test Chrome or Firefox here; no Chromium or Gecko build was installed.

**Accessibility.**

- Native MathML is exposed to assistive tech as math. JAWS, NVDA, Orca and VoiceOver can speak and navigate it, and NVDA merged MathCAT into core for 2026.1 ([nvaccess/nvda#18323](https://github.com/nvaccess/nvda/pull/18323)), so no add-on is needed.
- Equation numbers are real text in the DOM, not CSS-generated content.
- `annotation: true` embeds the TeX source in `<semantics>` for copy and paste, at a size cost.

**Speed and size cost.** 1.9 µs per expression means 1,000 expressions take about 2 ms, which is negligible next to the rest of a build. It adds 232 KB to a stripped LTO binary (hello-world 303 KB → 535 KB). MSRV is 1.96, edition 2024 ([Cargo.toml](https://docs.rs/crate/math-core/0.8.2/source/Cargo.toml.orig)).

### Other MathML Core crates

- **pulldown-latex 0.8.0** ([repo](https://github.com/carloskiki/pulldown-latex)). It claims "95% of what KaTeX and the likes support" but "not recommended for large scale production use" ([README](https://github.com/carloskiki/pulldown-latex#readme)). It has more amsmath environments than math-core (`split`, `alignat`, `smallmatrix`). But:
  - It has no `\label`/`\ref`/`\eqref`/`\tag`. Numbering comes from a CSS counter on `mtable.menv-with-eqn` in its `styles.css` (lines 110–118).
  - It has no `\boxed` and no `\mathscr`.
  - Macros exist only in-band. Its README lists "Macros preamble" as unsupported, and in our spike a 0-argument `\newcommand{\R}{\mathbb{R}}` failed with "expected an argument" (a `\def` worked).
  - Math-core's author notes it "can't strip away the unnecessary grouping … due to its architecture" ([math-core README](https://github.com/tmke8/math-core#alternatives-to-this-library)).
- **latex2mathml 0.2.3**: last release 2020-04-27, and math-core was forked from it ([math-core README § Acknowledgments](https://github.com/tmke8/math-core#acknowledgments)). It has no `cases`, `aligned`, `equation`, `array` or `\lVert` (spike). Not viable.
- **tex2math 2.4.1** exists (LGPL-3.0, 0 stars, 230 downloads) ([crates.io](https://crates.io/crates/tex2math)). It is too new to evaluate.
- **Temml** (JS, by KaTeX's former MathML maintainer) had the best coverage in our corpus (37/40) and good MathML. But in Rust it needs an embedded JS engine, and "everything in Temml will work except `\ref` and `\eqref`" without the browser-side `postProcess` ([Temml administration docs](https://temml.org/docs/en/administration)). It is a good reference for which commands math-core still lacks, not a dependency.

### KaTeX-style HTML+CSS

**katex-rs 0.3.0** ([repo](https://github.com/katex-rs/katex-rs)) is a native Rust re-implementation that "tracks KaTeX 0.18.5" and claims "pixel perfect rendering with KaTeX across all platforms" ([README](https://github.com/katex-rs/katex-rs#readme)). Its output matched KaTeX 0.19.0 run in QuickJS on every one of the 40 cases (same pass/fail set, byte sizes within 0.3%). Settings cover `macros`, `global_group`, `leqno`, `fleqn`, `output` (`HtmlAndMathml`/`Html`/`Mathml`), `trust` and `strict` ([settings.rs](https://docs.rs/katex-rs/0.3.0/katex/types/settings/struct.Settings.html)).

Limitations:

- **No `\label`, `\ref` or `\eqref`.** KaTeX's docs never mention `\label`, and only `\tag` is supported, "applied to individual rows of top-level environments" ([katex.org/docs/supported](https://katex.org/docs/supported)). The spike confirmed it: "Undefined control sequence: \label".
- **Numbers come from CSS counters.** Numbering is `.katex .eqn-num:before{content:"(" counter(katexEqnNo) ")"}` with `body{counter-reset:katexEqnNo}` in `katex.min.css`, so the numbers are not in the markup, cannot be linked to, and restart per page.
- **No `multline`, `\DeclareMathOperator` or `\sideset`** (spike).
- **Size.** About 98 KB raw and 9 KB gzip for the 20 common cases, 8.8x math-core raw. The default output duplicates every formula as visible HTML (`aria-hidden="true"`) plus hidden MathML for assistive tech ([KaTeX options: `htmlAndMathml` "includes MathML for accessibility"](https://katex.org/docs/options)).
- **Fonts.** `katex.min.css` is 24.8 KB and the 20 KaTeX WOFF2 files total 259,792 bytes, though browsers fetch only the faces a page uses ([jsDelivr listing of katex@0.19.0](https://data.jsdelivr.com/v1/packages/npm/katex@0.19.0)).
- **Maintenance risk.** One contributor (`sepcnt`), 5 commits since April 2026 ([contributors](https://github.com/katex-rs/katex-rs/graphs/contributors)).
- **Build conflict with math-core.** A crate depending on both `katex-rs 0.3.0` and `math-core 0.8.2` fails to compile katex-rs's build script: ``struct `phf::Map<char, AccentMapping>` has no field named `pilots` `` (phf 0.14 feature unification, reproduced in `/private/tmp/mathspike/both`). Either crate alone builds.

**KaTeX in an embedded JS engine** (rquickjs 0.14.0 + `katex.min.js`). This does not need Node, so it does not literally break ADR 0003. But ADR 0002 already rejected an embedded JS runtime as "slower and heavier", and the spike agrees:

- 794 µs per expression and 13 ms to start a runtime and evaluate the 273 KB bundle, about 400x slower than math-core.
- +1.3 MB of binary, plus a C toolchain to build QuickJS.

The old [`katex` crate](https://crates.io/crates/katex) (QuickJS-based) was last released 2023-02-05. Rustdoc's RFC rejects "katex run in quick-js" because "it's slow and seems to have poor error reporting" ([RFC 3958](https://github.com/rust-lang/rfcs/blob/master/text/3958-rustdoc-texmath.md)).

### Coverage gaps in math-core and workarounds

| Gap | Workaround the Engine can document | Upstream |
|---|---|---|
| `\DeclareMathOperator{\Tr}{Tr}` | Site config macro `Tr = \operatorname{Tr}` | [#155](https://github.com/tmke8/math-core/issues/155) |
| `split` inside `equation` | `\begin{equation}\begin{aligned}…\end{aligned}\end{equation}` (one number) | [#154](https://github.com/tmke8/math-core/issues/154) |
| `\ref{…}` (bare number) | Use `\eqref`. Or the Engine resolves `\ref` itself, but the label map is private, so we would need an upstream API | open question |
| `\tag` outside numbered env; `\tag{$…$}` | Wrap in `equation`; ASCII tags only | |
| `alignat`, `CD`, `smallmatrix`, `\substack`, `\widetilde`, `\varinjlim` | None; build error with span | [#154](https://github.com/tmke8/math-core/issues/154), [#155](https://github.com/tmke8/math-core/issues/155) |
| `\(…\)` nested in `\text{}` | None (KaTeX and Temml fail too) | [#431](https://github.com/tmke8/math-core/issues/431) |

### Integration notes

- **Markdown.** Both Markdown candidates for the Engine already tokenize math. pulldown-cmark 0.13 emits `Event::InlineMath`/`Event::DisplayMath` under `Options::ENABLE_MATH` ([spec](https://pulldown-cmark.github.io/pulldown-cmark/specs/math.html)), and comrak 0.55 has `extension.math_dollars` and `math_code` ([options.rs](https://docs.rs/comrak/0.55.0/comrak/options/struct.Extension.html)). The Engine collects every math span in a document, calls `convert_all` once, and splices the results back in.
- **Org.** Org's `\(…\)`, `\[…\]` and bare `\begin{align}…\end{align}` LaTeX environments map to the same snippets. The Org subset decides which environments are allowed.
- **Inline links.** `<a>` inside `<mtext>` works because `<mtext>` is a MathML text integration point in the HTML parser ([HTML § parsing](https://html.spec.whatwg.org/multipage/parsing.html#mathml-text-integration-point)). WebKit rendered the links in our spike.
- **Diagnostics.** Errors carry byte ranges into the snippet, and the `ariadne` feature turns them into pretty reports ([lib.rs features](https://docs.rs/math-core/0.8.2/math_core/)). That fits the Engine's compile-style diagnostics and the Agent eval loop.

## Open questions

1. **Default math font.** Which patched font from math-core-fonts should the Starter ship (New Computer Modern for a LaTeX look, or Libertinus)? It needs a visual check in Chrome, Firefox and Safari, since unpatched Libertinus broke delimiter stretching in WebKit.
2. **Font subsetting without Python or HarfBuzz.** Can the Engine subset the math font in Rust while keeping the `MATH` table and `ssty`? Candidates are [fontcull](https://crates.io/crates/fontcull), [allsorts](https://crates.io/crates/allsorts) and [subsetter](https://crates.io/crates/subsetter), plus [woofwoof](https://crates.io/crates/woofwoof)/[ttf2woff2](https://crates.io/crates/ttf2woff2) for WOFF2. This is unverified and needs its own spike.
3. **`\ref` support.** Should we upstream `\ref` (and a public label-map API) to math-core, or have the Engine number equations itself?
4. **Cross-page numbering.** Should equation numbering and `\eqref` work across a Post series (a shared counter or label map), or stay per document?
5. **Escape hatch.** Should a Site be allowed to opt into katex-rs per Post for unsupported constructs? Today the phf conflict blocks linking both, so this would need an upstream fix or a feature-gated build.
6. **Pinning.** math-core is pre-1.0 with breaking minors (0.6 → 0.7 → 0.8 in 2026), so pin the exact version and snapshot-test the output.

## Appendix: spike

Throwaway code lives in `/private/tmp/mathspike`. `spike/` holds math-core, pulldown-latex, latex2mathml, and KaTeX/Temml in rquickjs. `konly/` holds katex-rs, which had to be in a separate crate because of the phf conflict. `both/` reproduces that conflict. Outputs, WebKit snapshots and the subset font are in `out/`.

Each engine rendered the corpus in order as one "document": config macros `\R`, `\C`, and `\newcommand` persisting into later snippets where supported. We counted a case as passed when it rendered without error, and checked the key cases by eye in WebKit.

Corpus ids: inline-quadratic, inline-sets, fonts (`\mathcal`/`\mathscr`/`\mathfrak`/`\mathbb`/`\boldsymbol`), accents (incl. `\widehat`, `\widetilde`, `\vec`), bigops, limits (`\limsup`, `\varinjlim`), delims (`\left\langle`, `\middle|`, `\bigl`…`\Biggl`), binom (`\binom`, `\dfrac`, `\tfrac`, `\cfrac`), arrows (`\xrightarrow`, `\hookrightarrow`, `\overset`), braces (`\underbrace`, `\overbrace`, `\substack`), boxed-phantom (`\boxed`, `\operatorname*`), cases, dcases, matrices (p/b/v/V/B), big-matrix (with `\cdots`/`\vdots`/`\ddots`), smallmatrix, array (with `|` and `\hline`), aligned, align-star, align-numbered (`\label`, `\notag`, `\tag{A}`), tag-math-arg (`\tag{$\ast$}`), equation-label, eqref, ref, eqref-forward, equation-later, tag (top-level `\tag`), gather, multline, split, alignat, cd, newcommand-def, newcommand-use-later, config-macro, declaremathop, sideset, text-nested (`\text{… \(x\)}`), colors, coloneqq (`\coloneqq`, `\pmod`, `\hom`, `\ker`).

## Verification

Independent re-check on 2026-10-04. **The recommendation holds:** math-core 0.8.x, MathML Core output, katex-rs as the documented fallback. Confirmed against primary sources:

- `convert_all` resolves forward references because "all snippets need to be parsed first and can only then be emitted" ([docs.rs](https://docs.rs/math-core/0.8.2/math_core/struct.LatexToMathML.html#method.convert_all)). The spike output has `<a href="#eq:later"><mtext>(3)</mtext></a>`, so numbers are static text.
- The spike logs match the claimed pass rates and timings exactly (math-core 28/40 at 1.922 µs, katex-rs 30/40 at 37.5 µs, MathML-only 16.0 µs, Temml 37/40 at 321 µs, KaTeX in QuickJS 794 µs).
- RFC 3958 "Rustdoc LaTeX math" was merged on 2026-09-06 (rust-lang/rfcs#3958). It uses math-core and calls KaTeX in QuickJS "slow" with "poor error reporting". Its default font is Noto Sans Math.
- `katex.min.css` 0.19.0 (published 2026-10-01) contains `.katex .eqn-num:before{content:"(" counter(katexEqnNo) ")"…}`. KaTeX's supported list does not mention `\label`, `\ref` or `\eqref`.
- Temml docs: "If Temml is used server-side, `\ref` and `\eqref` are still implemented at runtime with client-side JavaScript."
- MathML is Baseline widely available since 2025-07-12 (web-features). The Interop team declined "Mathematics Rendering" on 2026-02-12, and the issue was closed on 2026-02-19.
- NVDA's MathCAT integration (nvaccess/nvda#18323) was merged on 2025-11-17 for milestone 2026.1.
- The phf build failure reproduces with `cargo build` in `/private/tmp/mathspike/both`.

Corrections and additions:

1. **math-core activity was understated.** It has 508 commits since 2026-04-01, not 100. The earlier count hit the API's 100-per-page cap. Contributors: tmke8 has 1,445 commits, Jules-Bertholet 63 and notriddle 29. The 8 releases since 2026-04-22 (0.6.1 to 0.8.2) are correct.
2. **Root cause of the phf conflict.** math-core turns on phf's **`ptrhash`** feature, which is not additive: under `cfg(feature = "ptrhash")`, `phf::Map` gains `pilots` and `remap` fields (`phf-0.14.0/src/map.rs`). The `phf_map!` code in katex-rs's build script does not use it. Feature unification therefore breaks katex-rs. Either crate could fix this upstream with a one-line feature change (katex-rs enables `ptrhash`, or math-core makes it optional), so the conflict is cheaper to clear than "needs upstream fixes" suggests.
3. **`multline` is supported in math-core 0.8.2.** The spike renders it correctly: first row left, last row right, number on the last row. The checklist in [math-core#154](https://github.com/tmke8/math-core/issues/154) still shows it unchecked, so that checklist is out of date and should not be used as the source for coverage.
4. **pulldown-latex has moved** to [Carlosted/pulldown-latex](https://github.com/Carlosted/pulldown-latex) (crates.io `repository`). The old `carloskiki` URL still resolves.
5. **A candidate that was missed but is not a contender.** `lo_math` 0.5.3 (MIT, "LaTeX formula parser with MathML and ODF emission") is a component of the `clark-labs-inc/libreoffice-rs` port and targets LibreOffice formulas. We did not evaluate it, and nothing suggests it supports `\label`/`\eqref`.
