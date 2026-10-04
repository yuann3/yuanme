# Which Rust Markdown parser should the Engine build on?

Research for [#2](https://github.com/yuann3/yuanme/issues/2), written 2026-10-04. It compares the current releases: comrak 0.55.0, pulldown-cmark 0.13.4 and markdown-rs (crate `markdown`) 1.0.0.

## Question

Which of comrak, pulldown-cmark and markdown-rs should the Engine use to parse Markdown? Each was checked for:

- CommonMark and GFM conformance (tables, task lists, strikethrough, autolinks, footnotes)
- smart punctuation that matches smartypants
- heading-id generation, and whether github-slugger ids can be reproduced
- `$…$` and `$$…$$` math syntax
- attribute or directive syntax that could carry theorem/proof blocks
- access to the AST or event stream for custom rendering
- source spans for diagnostics
- speed
- maintenance health

## Short answer

**Use comrak (0.55.x) with `default-features = false, features = ["attributes"]`, pinned to an exact minor version.** The Engine should walk comrak's AST into its own document tree and write its own HTML.

Comrak is the only one of the three that has every feature the Engine needs in a release today:

- 100% of CommonMark 0.31.2, and every GFM extension example in the GFM spec.
- Full GFM, including bare-URL autolinks.
- `$…$` and `$$…$$` math that leaves prices like `$5 and $10` alone.
- `:::name` block directives that nest, which can carry theorem and proof blocks.
- Pandoc-style `{#id .class k=v}` attributes.
- A mutable arena AST with line:column source positions on every node, inline nodes included.
- By far the most active maintenance.

The trade-offs:

- **Speed.** Comrak is about 4× slower than pulldown-cmark (about 110 vs 480 MB/s here). That is still about 0.25 ms for the Reference site's whole corpus, so it is not a real cost.
- **API churn.** Comrak breaks its API on almost every monthly 0.x minor release.

Whichever parser is chosen, **three parts of Reference-site parity belong to the Engine, not the parser**:

1. **HTML serialisation.** The site's escaping comes from hast-util-to-html, and so do details like task-list classes and footnote markup. None of the three built-in renderers produces that output.
2. **A smartypants pass.** All three either lack smart punctuation or differ from retext-smartypants.
3. **A github-slugger port.** It uses Unicode 13 categories, and the Engine must add Astro's trailing-`-` strip itself.

So the parser choice comes down to syntax coverage, AST quality, spans and maintenance, and on those comrak wins.

**Do not use markdown-rs.** It has no commits on `main` and no release since 2025-04-23. It has no smart punctuation, heading attributes, directives or extension API. It parses `$5 and $10` as math. Its 1.0.0 release also renders one of the Reference site's own pages (`Pew.md`) differently from remark: it makes a tight list loose.

**pulldown-cmark** is the fallback if speed or API stability ever matters more than syntax. It lacks bare-URL autolinks, and its released versions have no directive or container syntax. Theorem blocks would need a pre-pass in the Engine.

## Comparison

| | comrak 0.55.0 | pulldown-cmark 0.13.4 | markdown-rs 1.0.0 |
|---|---|---|---|
| CommonMark 0.31.2 spec (official runner) | **652/652** | 652/652 (631 before treating `"` vs `&quot;` as equal) | **652/652** |
| GFM spec 0.29 extension examples (table, strikethrough, autolink, tagfilter) | **all pass** | fails 11 bare-URL autolinks + tagfilter; 2 tables differ only in render | all but 1 (`ftp://` autolink, dropped on purpose) |
| Task lists, footnotes | yes / yes (GFM-style) | yes / yes (GFM-style by default) | yes / yes |
| Bare-URL autolinks (`www.x.com`, `https://…`) | yes | **no** ([#518](https://github.com/pulldown-cmark/pulldown-cmark/issues/518) open) | yes |
| Smart punctuation | yes, CommonMark-style; ≠ smartypants | yes, CommonMark-style; ≠ smartypants | **none** |
| Heading ids | `Anchorizer`, close to github-slugger but not identical | none (only explicit `{#id}`) | none |
| Heading `{#id .class k=v}` | yes (`header_attributes`, since 0.54) | yes (`ENABLE_HEADING_ATTRIBUTES`) | no |
| Attributes on code, inline code, links, images | yes (since 0.54) | no | no |
| `$…$` / `$$…$$` | yes (`math_dollars`), plus `\(…\)`, `\[…\]` and ```` ```math ```` | yes (`ENABLE_MATH`) | yes (`math_text`/`math_flow`) |
| `$5 and $10` left as text | yes | yes | **no, parsed as math** |
| Block directive / container (`:::theorem`) | **yes** (`block_directive`, since 0.52), nests by fence length | not in any release (`ContainerBlock` only on unreleased `main`) | no ([#57](https://github.com/wooorm/markdown-rs/issues/57) open) |
| Tree access | mutable arena AST (`parse_document`) and a custom-formatter macro | pull event stream (no tree) | read-only `mdast` (event stream is private) |
| Custom syntax | via options only | via options only | none in a release (plugin PR [#270](https://github.com/wooorm/markdown-rs/pull/270) unmerged) |
| Source spans | line:column start/end on every node, inline too (byte columns, or chars via `sourcepos_chars`) | byte `Range` per event (`into_offset_iter`) | line:column + byte offset on every node |
| Speed, parse + HTML, Reference corpus ×10 (274 KB), M4 Max | ~110 MB/s | **~480 MB/s** | ~7.7 MB/s |
| Dependencies (`cargo tree`, normal) | 12 crates (with defaults off) | 7 crates | 2 crates |
| Last release / last commit to main | 2026-09-06 / 2026-10-03 | 2026-05-20 / 2026-09-30 | **2025-04-23 / 2025-04-23** |
| Commits to main in the 12 months to 2026-10-04 | 681 (522 from the maintainer) | 143 (spread across contributors) | **0** |
| Release cadence and API stability | about monthly; breaking changes in most 0.x minors | patch releases from `branch_0.13`; `main` is 205 commits ahead and unreleased | 1.0, but frozen |
| License | BSD-2-Clause | MIT | MIT |
| Notable users | docs.rs, crates.io | rustdoc, mdBook, Zola | — |

## Details

### How the facts were settled

I built a throwaway spike in `/private/tmp/mdspike`. It is not committed. It links all three crates at the versions above and exposes each one as a stdin-to-HTML program. The tests run against it were:

- **Spec suites.** The official runners: [commonmark-spec 0.31.2](https://github.com/commonmark/commonmark-spec/tree/0.31.2/test) `spec_tests.py` and [cmark-gfm](https://github.com/github/cmark-gfm/tree/master/test) `spec_tests.py` with GFM `spec.txt` 0.29.
- **The remark reference.** The Reference site's own remark stack from `eyuan.me/node_modules`: unified 11.0.5, remark-parse 11, remark-gfm 4.0.1, remark-smartypants 3.0.2 / retext-smartypants 6.2.0, remark-rehype 11.1.2, rehype-stringify 10.0.1, and github-slugger 2.0.0.
- **The site's content.** The site's 15 Markdown files from `eyuan.me/src/content`.

### CommonMark and GFM conformance

**CommonMark 0.31.2.** Comrak and markdown-rs pass all 652 examples with the official runner.

pulldown-cmark at first failed 21. Every one of them is because it writes `"` raw in text where the spec's expected output has `&quot;`, and the two are equivalent HTML. With that one equivalence added to the normaliser, it passes 652/652 too. Its own README makes the same claim ([README](https://github.com/pulldown-cmark/pulldown-cmark#readme)).

**GFM spec 0.29.** The GFM spec is built on CommonMark 0.29. Comrak and markdown-rs each "fail" the same 18 core examples, in HTML blocks 140–147, emphasis 398–477 and autolinks 610–620. The CommonMark examples that changed after 0.29 are in those sections, and both parsers implement the newer behaviour.

On the **extension** examples:

- **comrak** passes every one: tables, strikethrough, autolink and tagfilter.
- **markdown-rs** fails only #628, because it does not link `ftp://` addresses, on purpose.
- **pulldown-cmark** fails all 11 extended-autolink examples (621–631) and tagfilter (#652). Tables #199 and #205 differ only in rendering: `style="text-align"` instead of `align`, and an empty `<tbody>`.

pulldown-cmark has no bare-URL autolinks. That is a long-standing gap, tracked in issue [#518 "Full GFM Support"](https://github.com/pulldown-cmark/pulldown-cmark/issues/518), and its option list has no such flag ([lib.rs `Options`, v0.13.4](https://github.com/pulldown-cmark/pulldown-cmark/blob/v0.13.4/pulldown-cmark/src/lib.rs)).

**Footnotes and task lists.** All three parse task lists and GFM footnotes ([comrak `footnotes`](https://github.com/kivikakk/comrak/blob/v0.55.0/src/parser/options.rs), [pulldown `ENABLE_FOOTNOTES`/`ENABLE_TASKLISTS`](https://github.com/pulldown-cmark/pulldown-cmark/blob/v0.13.4/pulldown-cmark/src/lib.rs), [markdown-rs `gfm_footnote_definition`/`gfm_task_list_item`](https://github.com/wooorm/markdown-rs/blob/1.0.0/src/configuration.rs)). None of the built-in renderers emits remark's markup:

- remark's task lists are `<ul class="contains-task-list"><li class="task-list-item">`.
- remark's footnotes use `user-content-fn-*` ids and an `<h2 class="sr-only" id="footnote-label">`.

markdown-rs is the closest, because it shares remark's lineage, but even its footnote backref `aria-label` differs.

**The Reference site's content.** I compared the element tree and text of each parser's output with remark-gfm's on all 15 content files, ignoring attributes, escaping and whitespace:

- **comrak:** 15/15 identical.
- **pulldown-cmark:** 15/15 identical.
- **markdown-rs:** 14/15. On `Pew.md` it renders a tight list as loose, with each item wrapped in `<p>`.

The minimal repro is `- a\n- b\n\n\n# h`. A list followed by two blank lines comes out loose in markdown-rs, and tight in comrak, pulldown-cmark and remark. The upstream fix is PR [#269](https://github.com/wooorm/markdown-rs/pull/269), opened 2026-09-29 and unmerged.

### Escaping: the Engine owns the HTML writer

The Reference site's Markdown HTML is serialised by hast-util-to-html:

- `<` becomes `&#x3C;` and `&` becomes `&#x26;`.
- `>` and `"` stay raw in text.

This is in [astro-parity-inventory §2.2 "Escaping"](astro-parity-inventory.md). All three built-in renderers escape differently: comrak and markdown-rs write `&lt; &gt; &quot;`, and pulldown-cmark writes `&lt; &gt;` with `"` raw.

The Reference site also needs HTML no parser emits on its own: Shiki `<pre class="astro-code …">`, task-list classes and no anchor `<a>` in headings. So for byte parity, the Engine must serialise HTML from its own tree whichever parser it picks. That means the built-in renderers don't decide this question, and AST quality does.

### Smart punctuation (smartypants)

The Reference site runs remark-smartypants 3.0.2 over retext-smartypants 6.2.0 with default options. Two details of that setup matter:

- **Dashes.** `dashes: true` turns `--` into an em dash and leaves `---` alone ([retext-smartypants lib/index.js `dashesDefault`](https://github.com/retextjs/retext-smartypants/blob/main/lib/index.js)). `ellipses: true` turns both `...` and `. . .` into `…`.
- **Quotes across nodes.** remark-smartypants joins **all text in the document**, with inline code replaced by `A`s, before it works out quotes. That way a quote next to `*emph*`, a link or `` `code` `` comes out right ([remark-smartypants plugin.js](https://github.com/silvenon/remark-smartypants/blob/main/plugin.ts)).

On a test document, the differences between the libraries and remark were:

| Input | remark (Reference) | comrak `smart` | pulldown `SMART_PUNCTUATION` |
|---|---|---|---|
| `a--b` | a—b | a–b | a–b |
| `a---b` | a---b | a—b | a—b |
| `. . .` | … | . . . | . . . |
| ` ``backticks'' ` | “backticks” | \`\`backticks’’ | \`\`backticks’’ |
| `'90s` | ’90s | ‘90s | ’90s |
| `6'2" tall."` | 6’2” tall.” | 6’2” tall.” | 6’2” tall.“ |

Quotes in ordinary prose agree everywhere, including the site's own `It’s`, `“Rewrite everything in Rust”` and `Rust’s Best Idea`. markdown-rs has no smart punctuation at all; there is no such option in [configuration.rs](https://github.com/wooorm/markdown-rs/blob/1.0.0/src/configuration.rs).

**Conclusion:** turn the parser's smart option off. The Engine should port retext-smartypants as a pass over the text nodes of its own tree, and the same pass then serves Org. comrak's mutable arena makes that pass easy: collect the Text and Code nodes in order, run the pass, and write the results back. With pulldown-cmark the same pass works over a buffered `Vec<Event>`.

### Heading ids and github-slugger

The Reference site's ids come from Astro's `rehypeHeadingIds`:

```js
let slug = slugger.slug(text);
if (slug.endsWith("-")) slug = slug.slice(0, -1);
```

Two consequences follow (`@astrojs/markdown-remark/dist/rehype-collect-headings.js`, 6.3.10):

- **The slugger records the unstripped slug.** A heading `Ruskey 🦀` records `ruskey-` as used. A later `Ruskey` heading would then get `ruskey` too, a duplicate id. A port must reproduce this, or record it as a Parity exception.
- **The slug comes from the text after smartypants.**

github-slugger 2.0.0 works like this:

- It lowercases the text, deletes every character matched by a generated regex, and turns each space into `-` ([index.js](https://github.com/Flet/github-slugger/blob/master/index.js); the 2.0.0 tarball is in `eyuan.me/node_modules/github-slugger`).
- The regex is generated from **Unicode 13.0.0** data: the package's devDependency is `@unicode/unicode-13.0.0` ([package.json](https://github.com/Flet/github-slugger/blob/master/package.json)).
- It keeps letters, marks, `Nd` and `Nl` numbers, connector punctuation (`_`), `-` and space.
- It dedupes with a per-slug counter (`-1`, `-2`, …).

| Library | Heading ids |
|---|---|
| comrak | `Anchorizer` ([anchorizer.rs](https://github.com/kivikakk/comrak/blob/v0.55.0/src/html/anchorizer.rs)) uses the same algorithm, with two differences (below). It is only applied through `header_id_prefix`, which also adds an anchor `<a>` the site doesn't have ([options.rs](https://github.com/kivikakk/comrak/blob/v0.55.0/src/parser/options.rs)). |
| pulldown-cmark | Generates no ids. |
| markdown-rs | Generates no ids. |

The two differences between comrak's `Anchorizer` and github-slugger:

1. **Which numbers it keeps.** comrak keeps every Number category, so `x² + y₂ = ½` becomes `x²--y₂--½`. github-slugger drops `No` characters and gives `x--y--`.
2. **Unicode version.** comrak uses the current Unicode data from `finl_unicode`. A letter added after Unicode 13, such as U+11AB0, is kept by comrak and dropped by github-slugger.

The other 15 of 17 test headings matched exactly, including the dedupe sequences, emoji, CJK, Greek final sigma and titlecase letters.

**Conclusion:** none of the three can produce Reference-site ids by itself. The Engine should own a small `slugger` module: a ~30-line port plus a Unicode-13 category table generated at build time, and the trailing-`-` quirk. That module also covers Org headlines and file-stem slugs (inventory §2.1 "Slugs").

The parser only needs to expose the heading's text after smartypants. comrak (`Node::collect_text`, moved onto `Node` in 0.54, [CHANGELOG](https://github.com/kivikakk/comrak/blob/main/CHANGELOG.md)) and pulldown-cmark (the Text events between `Start(Heading)` and `End`) both do.

### Math (`$…$`, `$$…$$`)

**comrak.** `math_dollars` produces `NodeValue::Math { dollar_math, display_math, literal }` for both `$x$` and `$$x$$`. It also has `math_latex` for `\(…\)` and `\[…\]`, and `math_code` for `` $`…`$ `` and ```` ```math ```` ([options.rs](https://github.com/kivikakk/comrak/blob/v0.55.0/src/parser/options.rs)). The `\(…\)` delimiters arrived in 0.54 ([CHANGELOG](https://github.com/kivikakk/comrak/blob/main/CHANGELOG.md)).

**pulldown-cmark.** `ENABLE_MATH` emits `Event::InlineMath` and `Event::DisplayMath` ([lib.rs](https://github.com/pulldown-cmark/pulldown-cmark/blob/v0.13.4/pulldown-cmark/src/lib.rs)).

**markdown-rs.** `math_text` and `math_flow` produce `InlineMath` and flow `Math` mdast nodes ([configuration.rs](https://github.com/wooorm/markdown-rs/blob/1.0.0/src/configuration.rs), [mdast.rs](https://github.com/wooorm/markdown-rs/blob/1.0.0/src/mdast.rs)).

Spike results:

- **Prices.** `$5 and $10` stays text in comrak and pulldown-cmark, which use pandoc-like flanking rules. markdown-rs turns it into `<code class="language-math math-inline">5 and </code>`, a real hazard in prose. Its only fix is `math_text_single_dollar: false`, which forces `$$` even for inline math.
- **Block `$$` in comrak and pulldown-cmark.** A `$$…$$` block on its own lines becomes a display-math **inline** node inside a `<p>`. The Engine should lift a paragraph whose only child is display math into a block.
- **Blank lines inside `$$`.** In comrak and pulldown-cmark a blank line inside `$$` breaks the block, because it is inline syntax. That matches LaTeX, where a blank line inside display math is an error, so it costs little. ```` ```math ```` covers the rare case. markdown-rs's `math_flow` allows the blank line.

All three expose the raw TeX string, so build-time math rendering (a separate decision) can plug into any of them.

### Theorem and proof blocks (directives and attributes)

**comrak: `block_directive`.** Added in 0.52 ([CHANGELOG](https://github.com/kivikakk/comrak/blob/main/CHANGELOG.md), PR [#782](https://github.com/kivikakk/comrak/pull/782)). It parses `:::name …` / `:::` containers whose body is ordinary Markdown, as `NodeValue::BlockDirective { fence_length, fence_offset, info }` ([nodes.rs](https://github.com/kivikakk/comrak/blob/v0.55.0/src/nodes.rs)). In the spike:

- A longer outer fence nests: `::::theorem` around `:::proof`. Inner content, including `$n > 2$`, is parsed as usual.
- If the outer and inner fences are the same length, the inner closer also closes the outer. Same-length nesting **does not** work, so the Author docs must say "use more colons on the outer block".
- The `info` string comes through raw (`theorem {#thm-1 title="Fermat"}`). The Engine parses the name, label and attributes from it, so the syntax is the Engine's to define.
- The built-in renderer dumps `info` into `class`, so a custom formatter or the Engine's own writer is needed anyway.

**comrak: attributes.** The `attributes` feature, added in 0.54 ([CHANGELOG](https://github.com/kivikakk/comrak/blob/main/CHANGELOG.md)), parses `{#id .class k=v}` on headings, fenced code, inline code, links and images into `Ast.attrs: Option<Box<Attributes { id, classes, pairs }>>`. The spike confirmed `## Fermat {#fermat .thm}` gives id `fermat` and class `thm`. Note:

- The feature is only on by default as part of the `cli` set, so a library build must enable `features = ["attributes"]`. The defaults also pull in clap and syntect, so turn them off ([Cargo.toml features](https://github.com/kivikakk/comrak/blob/v0.55.0/Cargo.toml)).
- No formatter outputs attributes yet ([CHANGELOG 0.54](https://github.com/kivikakk/comrak/blob/main/CHANGELOG.md)).

**pulldown-cmark.** The 0.13.x releases only have `ENABLE_HEADING_ATTRIBUTES` ([lib.rs](https://github.com/pulldown-cmark/pulldown-cmark/blob/v0.13.4/pulldown-cmark/src/lib.rs)). In the spike, `:::theorem` came out as a paragraph of literal text.

`main` has a `ContainerBlock(ContainerKind, CowStr)` tag, where `ContainerKind` is `Default` or `Spoiler`, aimed at commonmark-hs compatibility ([main lib.rs](https://github.com/pulldown-cmark/pulldown-cmark/blob/main/pulldown-cmark/src/lib.rs)). It has sat unreleased since mid-2025: `main` is 205 commits ahead of v0.13.4 ([compare](https://github.com/pulldown-cmark/pulldown-cmark/compare/v0.13.4...main)), and 0.13.x patches ship from `branch_0.13`. Issue [#616](https://github.com/pulldown-cmark/pulldown-cmark/issues/616) asks for directives and is open.

**markdown-rs.** It has neither directives ([#57](https://github.com/wooorm/markdown-rs/issues/57), open) nor heading attributes. `{#fermat .thm}` stays in the heading text. The plugin and extension work ([#32](https://github.com/wooorm/markdown-rs/issues/32), PR [#270](https://github.com/wooorm/markdown-rs/pull/270), 13.8k lines) is an unmerged stack.

**Fit with Org.** Org's `#+BEGIN_theorem … #+END_theorem` special blocks map one-to-one onto comrak's `BlockDirective`. Both Source formats can then share one theorem/proof node in the Engine's tree.

### Custom rendering and tree access

**comrak** builds a typed-arena tree (`parse_document(&arena, src, &opts)`) of `Node<RefCell<Ast>>`. Every node is mutable, so the Engine can rewrite or insert nodes in place, then render with `format_html`. comrak also offers:

- A `create_formatter!` macro that overrides rendering per `NodeValue` and can skip children ([html.rs](https://github.com/kivikakk/comrak/blob/v0.55.0/src/html.rs)).
- Adapters for code-fence syntax highlighting and headings ([adapters.rs](https://github.com/kivikakk/comrak/blob/v0.55.0/src/adapters.rs)).

**pulldown-cmark** is a pull parser: an iterator of `Event`s that the Engine can map or filter. Any whole-tree pass needs a tree built from the events first: smartypants across nodes, heading-id dedupe, or lifting display math. The pulldown-cmark README presents this streaming design as its main feature ([README](https://github.com/pulldown-cmark/pulldown-cmark#readme)).

**markdown-rs** exposes a read-only `mdast::Node` through `to_mdast`. The event stream behind it is private: `mod event` is not `pub` in [lib.rs](https://github.com/wooorm/markdown-rs/blob/1.0.0/src/lib.rs). `to_html` compiles from events, not from mdast, so custom output means writing an mdast-to-HTML writer from scratch. "mdast to html" is tracked in [#27](https://github.com/wooorm/markdown-rs/issues/27).

### Source spans for diagnostics

**comrak.** Every node, inline nodes included, has `Sourcepos { start, end }` with 1-based line and column. Columns count bytes by default, or chars with `parse.sourcepos_chars`, which arrived in 0.52 ([nodes.rs](https://github.com/kivikakk/comrak/blob/v0.55.0/src/nodes.rs), [options.rs](https://github.com/kivikakk/comrak/blob/v0.55.0/src/parser/options.rs)). The 0.47 changelog says "fixed _all known sourcepos issues_", and fixes kept coming through 0.53 ([CHANGELOG](https://github.com/kivikakk/comrak/blob/main/CHANGELOG.md)). In the spike, `$n > 2$` inside a directive reported `10:5-10:11`, which is correct.

**pulldown-cmark.** `into_offset_iter()` gives a byte `Range<usize>` per event ([parse.rs](https://github.com/pulldown-cmark/pulldown-cmark/blob/v0.13.4/pulldown-cmark/src/parse.rs)). Diagnostics need a line index to turn those into line:column.

**markdown-rs.** Every mdast node has a `Position` with line, column and offset ([unist.rs](https://github.com/wooorm/markdown-rs/blob/1.0.0/src/unist.rs)). Its list and list-item ends are known to fall too late (PR [#269](https://github.com/wooorm/markdown-rs/pull/269)).

All three are good enough for compile-style diagnostics. comrak is the most convenient because the tree already carries line:column.

### Speed

The benchmark parses and renders HTML with GFM on and smart punctuation on (markdown-rs has no smart option). It ran 50 iterations after a warm-up, as a release build on an Apple M4 Max with rustc 1.99.0.

| Corpus | pulldown-cmark | comrak | markdown-rs |
|---|---|---|---|
| Reference site posts and projects ×10 (274 KB) | 0.57 ms, ~480 MB/s | 2.4 ms, ~110 MB/s | 36 ms, ~7.7 MB/s |
| CommonMark `spec.txt` ×3 (615 KB) | 1.6 ms, ~390 MB/s | 6.6 ms, ~93 MB/s | 228 ms, ~2.7 MB/s |

The Reference site's whole Markdown corpus is 27 KB. At that size comrak takes about 0.25 ms. A large site with 10 MB of Markdown would take about 90 ms in comrak against about 20 ms in pulldown-cmark, either way small next to syntax highlighting and image encoding. markdown-rs would take about 1.3 s there, which is noticeable in an agent's edit-build loop.

### Maintenance health (as of 2026-10-04)

**comrak** ([repo](https://github.com/kivikakk/comrak), [crates.io](https://crates.io/crates/comrak)):

- Releases: 0.55.0 on 2026-09-06, 0.54.0 on 07-12, 0.53.0 on 07-02, 0.52.0 on 04-04, 0.51.0 on 03-10.
- The last commit is from 2026-10-03, and there are 13 open issues.
- 681 commits in the past year, 522 of them from the maintainer kivikakk. Activity is high, but the bus factor is effectively one.
- It handles security advisories (GHSA-xg9p-p4jc-c46g, autolink DoS, fixed in 0.55.0).
- Its API is deliberately unstable: 0.54 removed everything deprecated and moved APIs, and `tagfilter` goes away in 0.56 ([CHANGELOG](https://github.com/kivikakk/comrak/blob/main/CHANGELOG.md)).
- MSRV 1.85. Used by docs.rs ([docs_rs_web/Cargo.toml](https://github.com/rust-lang/docs.rs/blob/master/crates/bin/docs_rs_web/Cargo.toml)) and crates.io ([crates_io_markdown/Cargo.toml](https://github.com/rust-lang/crates.io/blob/main/crates/crates_io_markdown/Cargo.toml)).

**pulldown-cmark** ([repo](https://github.com/pulldown-cmark/pulldown-cmark), [crates.io](https://crates.io/crates/pulldown-cmark)):

- Releases: 0.13.4 on 2026-05-20, then 0.13.3 and 0.13.2 in March.
- The last commit is from 2026-09-30, and there are 94 open issues.
- 143 commits in the past year from several contributors.
- New features are stuck on unreleased `main`.
- It has the biggest user base: rustdoc, mdBook ([Cargo.toml](https://github.com/rust-lang/mdBook/blob/master/Cargo.toml)) and Zola ([components/markdown/Cargo.toml](https://github.com/getzola/zola/blob/master/components/markdown/Cargo.toml)). Its API changes slowly.

**markdown-rs** ([repo](https://github.com/wooorm/markdown-rs), [crates.io](https://crates.io/crates/markdown)):

- The only release since alpha is 1.0.0 on 2025-04-23, and that is also the last commit to `main`: 0 commits in the past 12 months.
- There are 89 open issues and PRs. A batch of fix PRs (#263–#270) opened in late September 2026 is unmerged. One of them notes that "CI on `main` fails" (PR [#269](https://github.com/wooorm/markdown-rs/pull/269)).
- A collaborator says it "is still maintained" (issue [#214](https://github.com/wooorm/markdown-rs/issues/214)), but nothing has been released.

## What the Engine owns no matter which parser it uses

1. **The HTML writer**, matching hast-util-to-html escaping and remark-rehype's element shapes: task-list classes, footnote ids, table `align`, and no heading anchor.
2. **The smartypants pass**, a port of retext-smartypants 6.2.0 defaults including cross-node quote context. Org shares it.
3. **The slugger**, github-slugger 2.0.0 with Unicode 13 tables, the trailing-`-` strip and the duplicate-id quirk. Org and file stems share it.
4. **Lifting display math** out of `<p>`, and **parsing the theorem/proof directive's info string**.

## Open questions

1. **Theorem syntax.** Use the comrak directive form `:::theorem[Title]{#thm-x}` with the Engine parsing the info string, or adopt the generic-directives proposal syntax that remark-directive uses, so Astro-era tools could read the same files? Same-length nesting is not supported, so outer blocks need more colons. Is that acceptable for Authors and agents?
2. **Cross-references.** How do Authors refer to a theorem (`[](#thm-x)`, `@thm-x`, wikilinks)? None of the parsers has inline directives.
3. **Dependency policy for comrak's churn.** Pin an exact `=0.55.x` and upgrade on purpose with a golden-output test suite, or vendor or fork? Who absorbs the monthly breaking changes?
4. **The duplicate-id quirk.** Reproduce Astro's (`ruskey-` recorded, `ruskey` emitted, so a later `Ruskey` heading gets the same id), or fix it and record a Parity exception? It does not occur in today's content.
5. **Which math delimiters to support** beyond `$`/`$$`: comrak can also do `\(…\)`, `\[…\]` and ```` ```math ````. Astro's contract excluded ```` ```math ```` from Shiki, which suggests keeping it.
6. **Revisit pulldown-cmark** if it releases `ContainerBlock` and GFM autolinks in a 0.14. A streaming parser would then win on speed and API stability.
