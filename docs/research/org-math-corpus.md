# Org and math test corpus

Ticket: [Assemble the Org and math test corpus](https://github.com/yuann3/yuanme/issues/14). Assembled on 2026-10-05. The corpus is in [`corpus/`](../../corpus/); its README covers layout, licences and how to regenerate these tables.

It feeds [What is the Org subset, and how do Org and Markdown share one metadata model?](https://github.com/yuann3/yuanme/issues/16) and [How do Authors write math, theorems and cross-references in each Source format?](https://github.com/yuann3/yuanme/issues/17), and later becomes snapshot-test fixtures.

## What's in it

157 files, 5.7 MB, from 35 sources. Every source is pinned to a commit, and every file's licence is recorded with evidence in `corpus/manifest.json`.

| Group | Files | Sources | Licences |
|---|---|---|---|
| Org manual, compact guide, test suite examples (GNU Org mode) | 17 | 3 | GFDL-1.3+, GPL-3.0+ |
| Worg pages | 58 | 1 | GFDL-1.3+ AND GPL-3.0+ |
| Org blogs, posts and lecture notes (**"posts and notes" below**) | 33 | 11 | CC-BY-4.0, CC-BY-SA-4.0, CC0, GPL-3.0, MIT |
| Other Org: ox-hugo manual and tests, org-ref, org-special-block-extras, Denote manual, 2 literate configs | 8 | 6 | GPL-3.0, GFDL-1.3+, CC0, MIT |
| Math-heavy Markdown (Pandoc, Quarto, kramdown, mdBook+KaTeX, MyST, d2l, Hugo) | 26 | 13 | MIT, Apache-2.0, MPL-2.0, CC-BY-SA-4.0, CC0, GPL-2.0+, Unlicense |
| The Author's own writing: eyuan.me blog and project entries | 15 | 1 | MIT OR Apache-2.0 (the Author's grant) |

73 of the third-party files are flagged as math. The Author has no Org writing of their own (they confirmed this in the ticket's HITL step), so the Author's own sample is Markdown only, and it contains no math.

## Method

- **Org constructs** are counted with Org's own parser: `org-element-parse-buffer` in Emacs 31.1 / Org 9.8.7, which is the reference implementation the organic fork has to match (`corpus/tools/org-features.el`). Each element and object type is counted, with details where they matter: keyword names, special-block types, source-block languages, LaTeX environment names, LaTeX fragment delimiters, link types, list kinds and affiliated keywords. All 116 Org files parsed without errors. `#+SETUPFILE` targets are not in the corpus, so custom TODO keywords defined there are not recognised. That affects only `headline:todo:*`.
- **Math syntax** is a regex scan over both formats (`corpus/tools/aggregate.py`). It is approximate: a dollar amount in prose can count as inline `$…$`.
- **Three columns** for each Org construct: files in the whole corpus, files among the 33 **posts and notes** (what an Author writes, as opposed to manuals, test files and wiki pages), and distinct sources. Occurrence totals are dominated by the Org manual (for example, 1,438 `{{{kbd}}}` macros come from 3 files), so use the file and source columns to judge prevalence.

## What the corpus says about the Org subset

Shares are over the 33 posts and notes.

- **Universal (≥ 70%)**: `#+TITLE` (31), `#+DATE` (25), headlines to level 3, links (30; `https` 22, `file` 22 (mostly images), described links 25), plain lists (24), source blocks (22), emphasis, `=verbatim=` and `~code~`.
- **Common (15–45%)**: LaTeX fragments (13), LaTeX environments (11, of which `align*` is 10), header arguments on source blocks (11), `#+FILETAGS` (11), footnotes (10), `#+CAPTION` (10), `#+RESULTS` (10), `#+ATTR_HTML` (9), example blocks (8), `#+SETUPFILE` (8), ox-hugo keywords `HUGO_*` (8), line breaks `\\` (7), tables (6), the `ignore` tag (6, an ox-extra convention), `#+LATEX_HEADER` (6).
- **Rare in posts (1–5)**: property drawers (5), `:noexport:` (5), `#+NAME` (5), descriptive lists (4), special blocks (4), quote blocks (4), fuzzy and `#+CUSTOM_ID` links (4 and 2), `id:` links (2), inline footnotes (2), export blocks and snippets (1).
- **Absent from posts**: entities (`\alpha` as text), `<<targets>>`, radio targets, macros, timestamps, planning lines, clocks, babel calls, inline source blocks, verse and center blocks, citations, dynamic blocks. These occur only in the manual, the test suite, Worg or tool documentation.
- **Org-only constructs that show up across sources even though posts rarely use them**: `#+INCLUDE` (7 files, 99 uses), `{{{macro}}}` (7 files / 6 sources), `#+begin_export html` (13 files), `@@html:…@@` snippets (6 files), `:PROPERTIES:` with `CUSTOM_ID` (25 files) and `ID` (20 files), `info:` links (9 files).
- **Exporter-specific front matter is real.** 8 of 33 posts carry `HUGO_*` keywords, and ox-hugo files put per-post metadata in `EXPORT_*` properties on subtrees (one file holds many posts). The metadata-model decision has to say whether those are mapped, ignored or rejected.

## What the corpus says about math

| | Org | Markdown |
|---|---|---|
| Inline | `$…$` (2,762 in 30 files) and `\(…\)` (684 in 30 files) are about equally widespread. org-element sees `\(…\)` in 13 files and `$…$` in 11. | `$…$` (2,719 in 20 files). `\(…\)` is rare (6 files). |
| Display | `\[…\]` (22 files) beats `$$…$$` (11 files). Bare LaTeX environments are common: `align*` in 10 files, `equation` 8, `align` 4. | `$$…$$` dominates (26 of 41 files). |
| Numbering and refs | `\label` in 12 files and `\ref`/`\eqref` in 11, across 7–8 sources. | `\label` in only 2 files. Pandoc/Quarto `{#eq-…}` and `@eq-…` in 5–8 files. d2l uses `:eqlabel:`/`:eqref:`. |
| Theorems | `#+begin_theorem`-style special blocks: 144 blocks in 4 files (proof 43, definition 38, proposition 21, lemma 14, remark 13, theorem 12, corollary 3). Only 2 of those files are notes, and both are by one author; see Gaps. | Quarto `::: {#thm-…}` divs (74 in 8 files, 3 sources), MyST `{prf:theorem}`, and one LaTeX `\begin{theorem}`. There is no single dominant syntax. |
| Macros | `\newcommand` and similar in 8 files (5 sources), mostly via `#+LATEX_HEADER`. | 4 uses in 2 files. |

## Gaps

- **No Org from the Author**, so Org coverage rests on third parties. Posts from 11 sources is enough for frequency, but not for the Author's own habits.
- **Org theorem blocks rest on one author.** Of the 4 files that use them, two are tool documentation (org-special-block-extras with 15 blocks, org-ref with 3). The other two, a dissertation (107 blocks) and PAC-learning notes (19), are both by Ignacio Cordón. Licensed Org math blogs are rare: most candidates had no licence or a NonCommercial one (`corpus/excluded.md`). The theorem syntax decision should lean on Org convention (special blocks, as ox-latex exports them) more than on these counts.
- **No R Markdown or bookdown** (the bookdown book is NonCommercial), and **Quarto's own docs are unlicensed**, so Quarto is covered only by its CLI test files plus two Quarto books or notes.
- **Licence mix**: about 40% of the files are GPL or GFDL. They are redistributable as fixtures under their own licences, not under the repo's. [What is yuanme's dependency and licence policy?](https://github.com/yuann3/yuanme/issues/27) should confirm this is acceptable for test fixtures that ship in the repo but not in the binary.
- **The GPL-3.0 blog repos** state no "or later", so they are recorded as `GPL-3.0-only`.

## Full frequency tables

Generated by `corpus/tools/aggregate.py`. "Posts and notes" is the 33-file subset described above.

### Element and object types

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `keyword` | 4,827 | 110 | 33 | 21 |
| `headline` | 4,532 | 107 | 29 | 21 |
| `link` | 3,880 | 92 | 30 | 20 |
| `verbatim` | 10,238 | 77 | 16 | 18 |
| `src-block` | 2,381 | 77 | 22 | 19 |
| `plain-list` | 1,667 | 75 | 24 | 21 |
| `comment` | 276 | 62 | 4 | 9 |
| `italic` | 1,401 | 58 | 15 | 19 |
| `bold` | 941 | 56 | 15 | 17 |
| `fixed-width` | 764 | 48 | 6 | 11 |
| `node-property` | 2,417 | 45 | 5 | 13 |
| `property-drawer` | 1,923 | 45 | 5 | 13 |
| `code` | 6,095 | 42 | 12 | 15 |
| `table` | 288 | 38 | 6 | 13 |
| `example-block` | 698 | 36 | 8 | 13 |
| `footnote-reference` | 414 | 33 | 10 | 10 |
| `latex-fragment` | 3,279 | 30 | 13 | 12 |
| `footnote-definition` | 188 | 30 | 8 | 8 |
| `quote-block` | 138 | 22 | 4 | 12 |
| `special-block` | 639 | 19 | 4 | 10 |
| `underline` | 84 | 18 | 6 | 8 |
| `subscript` | 322 | 17 | 5 | 8 |
| `entity` | 834 | 15 | 0 | 6 |
| `latex-environment` | 177 | 15 | 11 | 7 |
| `line-break` | 128 | 15 | 7 | 8 |
| `export-block` | 52 | 15 | 1 | 6 |
| `target` | 42 | 12 | 0 | 4 |
| `export-snippet` | 120 | 9 | 1 | 5 |
| `drawer` | 44 | 8 | 2 | 7 |
| `macro` | 1,923 | 7 | 0 | 6 |
| `timestamp` | 60 | 7 | 0 | 2 |
| `horizontal-rule` | 61 | 6 | 3 | 4 |
| `superscript` | 19 | 6 | 2 | 6 |
| `strike-through` | 15 | 6 | 2 | 5 |
| `statistics-cookie` | 12 | 6 | 0 | 2 |
| `babel-call` | 131 | 5 | 0 | 3 |
| `inline-src-block` | 48 | 4 | 0 | 4 |
| `verse-block` | 16 | 4 | 0 | 2 |
| `center-block` | 11 | 4 | 0 | 4 |
| `radio-target` | 8 | 4 | 0 | 2 |
| `planning` | 28 | 2 | 0 | 2 |
| `inline-babel-call` | 14 | 1 | 0 | 1 |
| `citation` | 1 | 1 | 0 | 1 |
| `clock` | 1 | 1 | 0 | 1 |
| `comment-block` | 1 | 1 | 0 | 1 |

_Keywords (`#+KEY:`): 68 rarer values omitted: `MACRO`, `HUGO_DRAFT`, `SELECT_TAGS`, `RESULTS`, `EXPORT_FILE_NAME`, `INDEX`, `KEYWORDS`, `HTML`, `TOC`, `LATEX_HEADER_EXTRA`, `HTML_HEAD`, `CREATOR`, `LATEX_CLASS`, `HUGO_CATEGORIES`, `FINDEX`, `TEXINFO`, `LATEX`, `TEXINFO_DIR_CATEGORY`, `TEXINFO_DIR_DESC`, `BIND`, `CINDEX`, `VINDEX`, `KINDEX`, `REVEAL`, `DOWNLOADED`, `HUGO`, `COMMENT`, `TEXINFO_DIR_TITLE`, `BIBLIOGRAPHY`, `HUGO_CUSTOM_FRONT_MATTER`, `TEACHER`, `COLUMNBREAK`, `TEXT`, `REPLACEWITH`, `TEXINFO_HEADER`, `ORGTBL`, `PINDEX`, `TEXINFO_FILENAME`, `PRINT_BIBLIOGRAPHY`, `CATEGORIES`…_

### Keywords (`#+KEY:`)

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `keyword:TITLE` | 107 | 106 | 31 | 21 |
| `keyword:OPTIONS` | 100 | 79 | 16 | 17 |
| `keyword:AUTHOR` | 72 | 72 | 10 | 15 |
| `keyword:STARTUP` | 69 | 61 | 12 | 13 |
| `keyword:LANGUAGE` | 57 | 57 | 5 | 9 |
| `keyword:EMAIL` | 45 | 45 | 4 | 6 |
| `keyword:HTML_LINK_HOME` | 42 | 42 | 1 | 2 |
| `keyword:HTML_LINK_UP` | 42 | 42 | 1 | 2 |
| `keyword:SEQ_TODO` | 42 | 41 | 1 | 3 |
| `keyword:TAGS` | 39 | 39 | 1 | 3 |
| `keyword:DATE` | 34 | 34 | 25 | 12 |
| `keyword:CATEGORY` | 29 | 29 | 0 | 1 |
| `keyword:PRIORITIES` | 26 | 26 | 0 | 1 |
| `keyword:PROPERTY` | 38 | 15 | 5 | 10 |
| `keyword:LATEX_HEADER` | 130 | 13 | 6 | 9 |
| `keyword:EXCLUDE_TAGS` | 13 | 13 | 5 | 5 |
| `keyword:SETUPFILE` | 13 | 13 | 8 | 6 |
| `keyword:DESCRIPTION` | 12 | 12 | 4 | 6 |
| `keyword:SUBTITLE` | 12 | 12 | 6 | 7 |
| `keyword:FILETAGS` | 11 | 11 | 11 | 3 |
| `keyword:HUGO_BASE_DIR` | 10 | 10 | 8 | 4 |
| `keyword:HUGO_SECTION` | 9 | 9 | 8 | 4 |
| `keyword:HUGO_TAGS` | 11 | 8 | 8 | 3 |
| `keyword:INFOJS_OPT` | 8 | 8 | 1 | 3 |
| `keyword:INCLUDE` | 99 | 7 | 0 | 5 |

### Affiliated keywords

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `affiliated:NAME` | 389 | 30 | 5 | 12 |
| `affiliated:CAPTION` | 224 | 23 | 10 | 11 |
| `affiliated:RESULTS` | 187 | 23 | 10 | 12 |
| `affiliated:ATTR_HTML` | 170 | 18 | 9 | 9 |
| `affiliated:ATTR_LATEX` | 39 | 7 | 2 | 6 |
| `affiliated:ATTR_TEXINFO` | 80 | 2 | 0 | 2 |
| `affiliated:ATTR_SHORTCODE` | 7 | 2 | 0 | 1 |
| `affiliated:ATTR_CSS` | 20 | 1 | 0 | 1 |
| `affiliated:ATTR_ORG` | 2 | 1 | 0 | 1 |

### Headline features

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `headline:level:1` | 740 | 107 | 29 | 21 |
| `headline:level:2` | 1,706 | 82 | 24 | 21 |
| `headline:level:3` | 1,143 | 48 | 15 | 19 |
| `headline:level:4` | 593 | 22 | 7 | 12 |
| `headline:tag:noexport` | 62 | 21 | 5 | 8 |
| `headline:todo:TODO` | 38 | 11 | 0 | 5 |
| `headline:tag:other` | 545 | 8 | 3 | 5 |
| `headline:tag:ignore` | 36 | 7 | 6 | 3 |
| `headline:level:5` | 238 | 6 | 1 | 5 |
| `headline:todo:DONE` | 53 | 4 | 0 | 2 |
| `headline:COMMENT` | 42 | 4 | 0 | 3 |
| `headline:level:6` | 112 | 3 | 0 | 2 |
| `headline:ARCHIVE` | 3 | 2 | 0 | 1 |
| `headline:todo:DRAFT` | 4 | 1 | 0 | 1 |
| `headline:todo:TEST__DONE` | 2 | 1 | 0 | 1 |
| `headline:todo:TEST__TODO` | 2 | 1 | 0 | 1 |
| `headline:priority` | 2 | 1 | 0 | 1 |
| `headline:todo:CANCELED` | 1 | 1 | 0 | 1 |

_Special blocks: 51 rarer values omitted: `org-demo`, `blindtext`, `minipage`, `spoiler`, `calc`, `solution`, `defopt`, `tooltip`, `fact`, `warning`, `mdshortcode`, `myshortcode`, `article`, `bar`, `cite`, `inline`, `katex`, `mdshortcode-named`, `myshortcode-named`, `myshortcode-pos`, `progress`, `section`, `tikzjax`, `video`, `alert-heading`, `alert2`, `black`, `blue`, `brown`, `darkgray`, `documentation`, `green`, `lightgray`, `lime`, `magenta`, `margin`, `olive`, `orange`, `pink`, `purple`…_

### Special blocks

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `special-block:theorem` | 12 | 4 | 2 | 4 |
| `special-block:infobox` | 7 | 4 | 0 | 1 |
| `special-block:details` | 69 | 3 | 0 | 2 |
| `special-block:lemma` | 14 | 3 | 2 | 3 |
| `special-block:description` | 120 | 2 | 0 | 1 |
| `special-block:note` | 46 | 2 | 0 | 1 |
| `special-block:proof` | 43 | 2 | 2 | 2 |
| `special-block:definition` | 38 | 2 | 2 | 2 |
| `special-block:notes` | 27 | 2 | 2 | 1 |
| `special-block:mark` | 24 | 2 | 0 | 1 |
| `special-block:summary` | 11 | 2 | 0 | 1 |
| `special-block:foo` | 8 | 2 | 0 | 2 |
| `special-block:alert` | 5 | 2 | 0 | 1 |
| `special-block:red` | 3 | 2 | 0 | 2 |
| `special-block:corollary` | 3 | 2 | 1 | 2 |
| `special-block:aside` | 2 | 2 | 0 | 1 |
| `special-block:gray` | 2 | 2 | 0 | 2 |
| `special-block:something` | 2 | 2 | 0 | 1 |
| `special-block:latex` | 2 | 2 | 0 | 2 |
| `special-block:warningbox` | 2 | 2 | 0 | 1 |
| `special-block:box` | 27 | 1 | 0 | 1 |
| `special-block:proposition` | 21 | 1 | 1 | 1 |
| `special-block:parallel` | 19 | 1 | 0 | 1 |
| `special-block:defun` | 18 | 1 | 0 | 1 |
| `special-block:remark` | 13 | 1 | 0 | 1 |

_Source block languages: 30 rarer values omitted: `dot`, `ini`, `conf`, `goat`, `cpp`, `d`, `nim`, `makefile`, `haskell`, `rust`, `go-html-template`, `systemd`, `pikchr`, `ruby`, `shell-script-mode`, `gitconfig`, `ipython`, `http`, `mermaid`, `restclient`, `scheme`, `none`, `awk`, `sql`, `makefile-gmake`, `shell-script`, `fundamental`, `perl`, `gitattributes`, `xml`_

### Source block languages

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `src-block:emacs-lisp` | 1,341 | 48 | 8 | 14 |
| `src-block:has-header-args` | 756 | 40 | 11 | 13 |
| `src-block:org` | 394 | 29 | 4 | 9 |
| `src-block:sh` | 48 | 14 | 3 | 10 |
| `src-block:elisp` | 48 | 12 | 3 | 7 |
| `src-block:latex` | 44 | 11 | 1 | 5 |
| `src-block:r` | 121 | 9 | 3 | 4 |
| `src-block:python` | 24 | 8 | 4 | 6 |
| `src-block:shell` | 46 | 7 | 0 | 6 |
| `src-block:html` | 19 | 7 | 1 | 6 |
| `src-block:text` | 7 | 7 | 4 | 4 |
| `src-block:julia` | 74 | 4 | 4 | 1 |
| `src-block:bash` | 7 | 4 | 4 | 2 |
| `src-block:plantuml` | 6 | 4 | 1 | 3 |
| `src-block:js` | 7 | 3 | 1 | 3 |
| `src-block:c` | 5 | 3 | 1 | 3 |
| `src-block:lisp` | 4 | 3 | 1 | 2 |
| `src-block:ditaa` | 4 | 3 | 0 | 1 |
| `src-block:maxima` | 30 | 2 | 0 | 2 |
| `src-block:md` | 21 | 2 | 0 | 1 |
| `src-block:toml` | 20 | 2 | 0 | 1 |
| `src-block:css` | 9 | 2 | 0 | 1 |
| `src-block:conf-toml` | 5 | 2 | 0 | 1 |
| `src-block:jupyter-python` | 5 | 2 | 0 | 2 |
| `src-block:yaml` | 5 | 2 | 1 | 2 |

### Inline source blocks

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `inline-src-block:emacs-lisp` | 7 | 3 | 0 | 3 |
| `inline-src-block:latex` | 5 | 2 | 0 | 2 |
| `inline-src-block:elisp` | 17 | 1 | 0 | 1 |
| `inline-src-block:sh` | 7 | 1 | 0 | 1 |
| `inline-src-block:shell` | 5 | 1 | 0 | 1 |
| `inline-src-block:md` | 3 | 1 | 0 | 1 |
| `inline-src-block:org` | 2 | 1 | 0 | 1 |
| `inline-src-block:go-html-template` | 1 | 1 | 0 | 1 |
| `inline-src-block:nim` | 1 | 1 | 0 | 1 |

### Export blocks and snippets

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `export-block` | 52 | 15 | 1 | 6 |
| `export-block:html` | 42 | 13 | 0 | 5 |
| `export-snippet` | 120 | 9 | 1 | 5 |
| `export-snippet:html` | 111 | 6 | 1 | 4 |
| `export-block:latex` | 5 | 3 | 1 | 3 |
| `export-block:hugo` | 3 | 2 | 0 | 1 |
| `export-snippet:hugo` | 3 | 2 | 0 | 1 |
| `export-snippet:latex` | 3 | 2 | 0 | 2 |
| `export-block:markdown` | 1 | 1 | 0 | 1 |
| `export-block:md` | 1 | 1 | 0 | 1 |
| `export-snippet:markdown` | 1 | 1 | 0 | 1 |
| `export-snippet:md` | 1 | 1 | 0 | 1 |
| `export-snippet:htl` | 1 | 1 | 0 | 1 |

### LaTeX environments

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `latex-environment:align*` | 67 | 10 | 10 | 5 |
| `latex-environment:equation` | 36 | 8 | 4 | 5 |
| `latex-environment:align` | 35 | 4 | 3 | 3 |
| `latex-environment:equation*` | 7 | 3 | 2 | 3 |
| `latex-environment:algorithm` | 19 | 1 | 1 | 1 |
| `latex-environment:cases` | 3 | 1 | 1 | 1 |
| `latex-environment:center` | 3 | 1 | 1 | 1 |
| `latex-environment:gather` | 2 | 1 | 1 | 1 |
| `latex-environment:eqnarray` | 2 | 1 | 1 | 1 |
| `latex-environment:eqnarray*` | 2 | 1 | 1 | 1 |
| `latex-environment:flushright` | 1 | 1 | 1 | 1 |

### LaTeX fragments

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `latex-fragment:command` | 255 | 15 | 2 | 7 |
| `latex-fragment:inline-paren` | 487 | 13 | 9 | 6 |
| `latex-fragment:display-bracket` | 214 | 12 | 8 | 8 |
| `latex-fragment:inline-dollar` | 2,265 | 11 | 7 | 8 |
| `latex-fragment:display-dollars` | 58 | 5 | 0 | 4 |

### Link types

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `link:has-description` | 2,651 | 85 | 25 | 19 |
| `link:https` | 1,353 | 74 | 22 | 17 |
| `link:file` | 484 | 64 | 22 | 16 |
| `link:http` | 345 | 51 | 7 | 11 |
| `link:fuzzy` | 752 | 33 | 4 | 12 |
| `link:custom-id` | 769 | 17 | 2 | 6 |
| `link:info` | 74 | 9 | 0 | 6 |
| `link:id` | 22 | 8 | 2 | 5 |
| `link:mailto` | 10 | 5 | 0 | 5 |
| `link:radio` | 29 | 4 | 0 | 2 |
| `link:coderef` | 23 | 4 | 0 | 3 |
| `link:elisp` | 3 | 2 | 0 | 2 |
| `link:ftp` | 2 | 2 | 0 | 2 |
| `link:shell` | 2 | 2 | 0 | 2 |
| `link:help` | 11 | 1 | 0 | 1 |
| `link:doi` | 1 | 1 | 0 | 1 |

### Lists and items

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `plain-list:unordered` | 788 | 65 | 20 | 21 |
| `plain-list:ordered` | 203 | 42 | 13 | 15 |
| `plain-list:descriptive` | 676 | 35 | 4 | 11 |
| `item:checkbox` | 86 | 8 | 1 | 4 |
| `item:counter` | 12 | 2 | 0 | 2 |

### Tables

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `table:org` | 288 | 38 | 6 | 13 |
| `table:tblfm` | 13 | 7 | 0 | 3 |

### Drawers

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `drawer:LOGBOOK` | 14 | 3 | 0 | 3 |
| `drawer:CONTENTS` | 2 | 2 | 2 | 1 |
| `drawer:LATEX_PROPERTIES` | 2 | 2 | 2 | 1 |
| `drawer:OLD` | 5 | 1 | 0 | 1 |
| `drawer:HIDE` | 4 | 1 | 0 | 1 |
| `drawer:HEADER` | 2 | 1 | 0 | 1 |
| `drawer:EXAMPLES` | 1 | 1 | 0 | 1 |
| `drawer:EXAMPLE_USES` | 1 | 1 | 0 | 1 |
| `drawer:HIDE_STARTUP_CODE` | 1 | 1 | 0 | 1 |
| `drawer:LINKS_FROM_TDEHAEZE_REDDIT` | 1 | 1 | 0 | 1 |
| `drawer:MULTIMETHOD_INVOCATION_EXAMPLES_OF_SHOUT` | 1 | 1 | 0 | 1 |
| `drawer:OLDER_APPROACH` | 1 | 1 | 0 | 1 |
| `drawer:OLDER_VERSION` | 1 | 1 | 0 | 1 |
| `drawer:OLD_UNNECESSARY_IMPLEMENTAITON` | 1 | 1 | 0 | 1 |
| `drawer:ONMOUSEOVER_ONMOUSEOUT_APPROACH` | 1 | 1 | 0 | 1 |
| `drawer:OUTDATED_HIDE` | 1 | 1 | 0 | 1 |
| `drawer:OUTDATED_PICS` | 1 | 1 | 0 | 1 |
| `drawer:PICS_OLD` | 1 | 1 | 0 | 1 |
| `drawer:POSTERITY_OLD_ORG_EXPORT_PARSE` | 1 | 1 | 0 | 1 |
| `drawer:RESULTS` | 1 | 1 | 0 | 1 |
| `drawer:INTRO` | 1 | 1 | 0 | 1 |

_Properties: 63 rarer values omitted: `AUTHOR`, `TOC`, `EXPORT_TITLE`, `EXPORT_DATE`, `EXPORT_HUGO_CUSTOM_FRONT_MATTER+`, `EXPORT_HUGO_RESOURCES+`, `EXPORT_HUGO_WEIGHT`, `EXPORT_HUGO_FRONT_MATTER_FORMAT`, `EXPORT_HUGO_FRONT_MATTER_KEY_REPLACE`, `EXPORT_AUTHOR`, `EXPORT_HUGO_GOLDMARK`, `EXPORT_HUGO_RESOURCES`, `EXPORT_HUGO_WEIGHT+`, `REVEAL_BACKGROUND`, `EXPORT_DESCRIPTION`, `EXPORT_HUGO_LEVEL_OFFSET`, `EXPORT_HUGO_SECTION_FRAG`, `EXPORT_HTML_CONTAINER_CLASS`, `EXPORT_HUGO_CODE_FENCE`, `EXPORT_HUGO_LOCALE`, `EXPORT_HUGO_PRESERVE_FILLING`, `EXPORT_HTML_CONTAINER`, `EXPORT_HUGO_ALLOW_SPACES_IN_TAGS`, `EXPORT_HUGO_PANDOC_CITATIONS`, `EXPORT_HUGO_PREFER_HYPHEN_IN_TAGS`, `EXPORT_KEYWORDS`, `LOGGING`, `LOG_INTO_DRAWER`, `HEADER-ARGS:EMACS-LISP+`, `EXPORT_BIBLIOGRAPHY`, `EXPORT_HUGO_DATE_FORMAT`, `EXPORT_HUGO_DRAFT`, `EXPORT_HUGO_EXPIRYDATE`, `EXPORT_HUGO_IMAGES`, `EXPORT_HUGO_IMAGES+`, `EXPORT_HUGO_LASTMOD`, `EXPORT_HUGO_LINKTITLE`, `EXPORT_HUGO_PUBLISHDATE`, `EXPORT_HUGO_SERIES`, `EXPORT_HUGO_SLUG`…_

### Properties

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `node-property:CUSTOM_ID` | 798 | 25 | 0 | 8 |
| `node-property:ID` | 62 | 20 | 2 | 5 |
| `node-property:UNNUMBERED` | 141 | 4 | 1 | 4 |
| `node-property:HEADER-ARGS` | 10 | 4 | 0 | 1 |
| `node-property:ALT_TITLE` | 49 | 3 | 0 | 3 |
| `node-property:HEADER-ARGS+` | 5 | 3 | 0 | 1 |
| `node-property:COPYING` | 3 | 3 | 0 | 3 |
| `node-property:EXPORT_FILE_NAME` | 473 | 2 | 0 | 1 |
| `node-property:DESCRIPTION` | 374 | 2 | 0 | 2 |
| `node-property:EXPORT_OPTIONS` | 43 | 2 | 0 | 1 |
| `node-property:EXPORT_HUGO_CUSTOM_FRONT_MATTER` | 23 | 2 | 0 | 1 |
| `node-property:EXPORT_HUGO_SECTION` | 21 | 2 | 0 | 1 |
| `node-property:EXPORT_HUGO_MENU` | 20 | 2 | 0 | 1 |
| `node-property:EXPORT_HUGO_BUNDLE` | 14 | 2 | 0 | 1 |
| `node-property:EXPORT_HUGO_ALIASES` | 13 | 2 | 0 | 1 |
| `node-property:HEADER-ARGS:EMACS-LISP` | 11 | 2 | 0 | 2 |
| `node-property:INDEX` | 7 | 2 | 0 | 2 |
| `node-property:EXPORT_HUGO_LAYOUT` | 7 | 2 | 0 | 1 |
| `node-property:EXPORT_HUGO_PAIRED_SHORTCODES` | 6 | 2 | 0 | 1 |
| `node-property:APPENDIX` | 4 | 2 | 0 | 2 |
| `node-property:EXPORT_HUGO_MENU_OVERRIDE` | 4 | 2 | 0 | 1 |
| `node-property:EXPORT_HUGO_OUTPUTS` | 4 | 2 | 0 | 1 |
| `node-property:EXPORT_HUGO_USE_CODE_FOR_KBD` | 4 | 2 | 0 | 1 |
| `node-property:DIR` | 4 | 2 | 0 | 2 |
| `node-property:EXPORT_HUGO_TYPE` | 3 | 2 | 0 | 1 |

_Macros: 11 rarer values omitted: `stable-version`, `hugopr`, `youtube`, `min_emacs_version`, `min_org_version`, `org_mode_version`, `pr`, `user`, `property`, `cite`, `version`_

### Macros

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `macro:kbd` | 1,438 | 3 | 0 | 3 |
| `macro:var` | 126 | 2 | 0 | 2 |
| `macro:commit` | 10 | 2 | 0 | 1 |
| `macro:latex` | 10 | 2 | 0 | 1 |
| `macro:bfissue` | 7 | 2 | 0 | 1 |
| `macro:hugoissue` | 4 | 2 | 0 | 1 |
| `macro:showicon` | 92 | 1 | 0 | 1 |
| `macro:oxhugoissue` | 80 | 1 | 0 | 1 |
| `macro:issue` | 35 | 1 | 0 | 1 |
| `macro:relref` | 24 | 1 | 0 | 1 |
| `macro:withsrc` | 21 | 1 | 0 | 1 |
| `macro:titleref` | 18 | 1 | 0 | 1 |
| `macro:results` | 6 | 1 | 0 | 1 |
| `macro:doc` | 6 | 1 | 0 | 1 |
| `macro:pandoc_version` | 5 | 1 | 0 | 1 |
| `macro:sec` | 5 | 1 | 0 | 1 |
| `macro:n` | 4 | 1 | 0 | 1 |
| `macro:ox-hugo-test-file` | 4 | 1 | 0 | 1 |
| `macro:imageclick` | 3 | 1 | 0 | 1 |
| `macro:test-search` | 3 | 1 | 0 | 1 |
| `macro:testtag` | 3 | 1 | 0 | 1 |
| `macro:newline` | 3 | 1 | 0 | 1 |
| `macro:development-version` | 2 | 1 | 0 | 1 |
| `macro:min_hugo_version` | 2 | 1 | 0 | 1 |
| `macro:release-date` | 1 | 1 | 0 | 1 |

### Timestamps

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `timestamp:inactive` | 28 | 6 | 0 | 2 |
| `timestamp:active` | 32 | 2 | 0 | 1 |

### Citations

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `citation:style:default` | 1 | 1 | 0 | 1 |

### Footnotes

| Construct | Occurrences | Files (of 116) | Posts and notes (of 33) | Sources (of 21) |
|---|---|---|---|---|
| `footnote-reference:standard` | 251 | 30 | 8 | 8 |
| `footnote-reference:inline` | 163 | 6 | 2 | 4 |

## Math syntax by format

Regex scan over every corpus file, so it is approximate: `$` amounts in prose can be counted as inline math.

| Syntax | md: occ / files (of 41) / sources | org: occ / files (of 116) / sources |
|---|---|---|
| inline `$…$` | 2,719 / 20 / 11 | 2,762 / 30 / 14 |
| inline `\(…\)` | 25 / 6 / 5 | 684 / 30 / 11 |
| display `$$…$$` | 2,194 / 26 / 13 | 143 / 11 / 8 |
| display `\[…\]` | 35 / 6 / 4 | 280 / 22 / 13 |
| `\begin{equation}` | 6 / 2 / 2 | 50 / 13 / 10 |
| `\begin{align}` | 14 / 5 / 4 | 108 / 15 / 9 |
| other `\begin{…}` (gather, cases, matrix…) | 111 / 13 / 8 | 213 / 24 / 12 |
| `\label{…}` | 22 / 2 / 2 | 100 / 12 / 7 |
| `\ref` / `\eqref` / `\cref` | 13 / 1 / 1 | 119 / 11 / 8 |
| `\tag{…}` | — | 2 / 1 / 1 |
| macro defs (`\newcommand`, `\def`, `\DeclareMathOperator`) | 4 / 2 / 2 | 100 / 8 / 5 |
| Org theorem-like special blocks | — | 144 / 4 / 4 |
| LaTeX theorem environments | 1 / 1 / 1 | 3 / 2 / 2 |
| Pandoc/Quarto fenced theorem divs (`::: {#thm-…}`) | 74 / 8 / 3 | — |
| Quarto equation ids (`$$ {#eq-…}`) | 22 / 5 / 2 | — |
| Pandoc/Quarto cross-refs (`@eq-…`, `@thm-…`) | 78 / 8 / 3 | — |
| HTML theorem divs (`<div class="theorem">`) | — | 3 / 2 / 2 |
