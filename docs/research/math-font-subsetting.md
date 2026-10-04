# Math font subsetting and WOFF2 in pure Rust

Ticket: [yuann3/yuanme#29](https://github.com/yuann3/yuanme/issues/29). Researched 2026-10-04. Follows [build-time math](https://github.com/yuann3/yuanme/blob/research/build-time-math/docs/research/build-time-math.md), which chose MathML Core from math-core with a patched font from [math-core-fonts](https://github.com/tmke8/math-core-fonts).

## Question

Can the Engine subset the math font to the glyphs a Site uses and compress it to WOFF2 in pure Rust, keeping the OpenType `MATH` table and the `ssty` feature, with no Python or HarfBuzz? We checked fontcull, allsorts, subsetter, woofwoof and ttf2woff2, plus every other candidate we found. We measured output size and rendering in Chrome, Firefox and Safari, and this note recommends a stack.

## Short answer

**Yes, but no crate does it alone. The Engine has to put four crates together and own two small pieces of code itself.** Our pure-Rust spike gave the same pixels as `hb-subset` + `woff2_compress` in Chromium 153, Firefox 155 and 157, WebKit 26.6 and Safari 27.0.1, for all three math-core-fonts faces. Its WOFF2 files were 1–15% larger than HarfBuzz's.

Recommended stack (all versions pinned):

| Job | Crate | Notes |
|---|---|---|
| Glyph closure (cmap + cmap14 + bidi mirrors, GSUB, `MATH` variants and assembly parts) | [`read-fonts`](https://crates.io/crates/read-fonts) 0.45.0 + [`unicode-bidi-mirroring`](https://crates.io/crates/unicode-bidi-mirroring) 0.4.0 | `Gsub::closure_glyphs`, `Cmap::closure_glyphs` and the `MATH` reader are all public. 0.45.0 is the first release with `MATH` ([fontations#2105](https://github.com/googlefonts/fontations/pull/2105), merged 2026-09-08). |
| cmap, hmtx, hhea, maxp, GSUB, GPOS, GDEF, name, OS/2, post, head | [`skera`](https://crates.io/crates/skera) 0.8.0 | Google's HarfBuzz-port subsetter, formerly klippa. We pass the closure as explicit gids and set `SUBSET_FLAGS_NO_LAYOUT_CLOSURE`. |
| `CFF ` outlines | [`allsorts`](https://crates.io/crates/allsorts) 0.17.0, `cff.subset(gids, false)` | We then blank the String INDEX entries that are no longer used. The `false` matters: CID conversion changed glyph rasterization. |
| `MATH` | Engine code, about 120 lines on [`write-fonts`](https://crates.io/crates/write-fonts) 0.54.0 | No published crate subsets `MATH`. |
| WOFF2 | [`ttf2woff2`](https://crates.io/crates/ttf2woff2) 0.13.3 with a one-line patch to accept `OTTO` | Pure Rust, with Brotli from the `brotli` crate. Stock 0.13.3 rejects CFF fonts. |
| Character set | Engine code: a port of math-core's [`extract_math_chars.py`](https://github.com/tmke8/math-core/blob/fe8ce3e0c73acf03b4e01d174438a9b96c9abb97/scripts/extract_math_chars.py), plus U+0020, U+00AF, `1`, U+2212 and U+221A every time | Without these, all three browsers rendered visibly wrong. See [Which characters to keep](#which-characters-to-keep). |

Results:

- **Speed.** Subsetting takes 1–5 ms per font. WOFF2 at Brotli quality 11 takes 30–820 ms, so the Engine should cache the output by font hash and character-set hash.
- **Binary size.** The stack adds about 2.5 MB to a stripped LTO binary. The largest parts are skera (440 KB of code) and the Brotli encoder (330 KB).
- **No native toolchain.** No C or C++ is compiled, and there is no Python and no HarfBuzz. The only native dependency is `libc`.

None of the named crates works on its own today:

- **fontcull 2.0.1** (a fork of klippa, now inside Dodeca) and **skera 0.8.0** have no CFF subsetter and no `MATH` subsetter. Run on New Computer Modern Math, skera silently drops `CFF ` and `MATH`. fontcull passes both through unchanged with glyph ids that are now wrong. OTS, the sanitizer that Chrome and Firefox run on every web font, then rejects the font.
- **allsorts 0.17.0** subsets CFF but never writes GSUB, GPOS or `MATH`, even when they are requested.
- **subsetter 0.2.6** is for PDF only and writes no cmap or OS/2.
- **font-subset 0.1.0** handles TrueType only and drops layout tables.
- **woofwoof 1.0.2** works, but it compiles Google's C++ woff2 through `cc`.
- **ttf2woff2 0.13.3** is pure Rust but refuses `OTTO` input.

All of these are spike results; see the [table](#comparison) and the [appendix](#appendix-spike).

**A correction to the build-time-math note.** Its "21 KB WOFF2 for our 40-expression corpus" counted only the characters in the source text. That missed the italic letters that MathML Core's `math-auto` transform draws, the radical, and characters that browsers measure for themselves. With the right character set, the same page needs 27.5 KB (Libertinus Math, hb-subset) to 51.8 KB (New Computer Modern Math, our Rust pipeline).

## Comparison

### Candidate crates (checked 2026-10-04)

| Crate | Version (date) | Pure Rust? | CFF outlines | GSUB/`ssty` | `MATH` | WOFF2 out | Verdict |
|---|---|---|---|---|---|---|---|
| [fontcull](https://crates.io/crates/fontcull) | 2.0.1 (2026-01-23) | Subsetting yes; WOFF2 through woofwoof (C++) | **no**, passed through unsubset | yes | **passed through with stale gids** | through woofwoof | No. OTS rejects the output ("CFF: Failed to parse Top DICT Data"). The repo is archived and development moved into [Dodeca](https://github.com/bearcove/dodeca) ([README](https://github.com/bearcove/fontcull/blob/b5d9b45c7b3e39d50dae76b8c221b7e516b71fe7/README.md)). |
| [skera](https://crates.io/crates/skera) (formerly klippa) | 0.8.0 (2026-10-03) | yes | **no**, table dropped | yes (HarfBuzz port) | **no**, dropped. MATH closure "not supported yet" ([lib.rs L564](https://github.com/googlefonts/fontations/blob/skera-v0.8.0/skera/src/lib.rs#L564)) | no | **Use it for everything except CFF and `MATH`.** |
| [allsorts](https://crates.io/crates/allsorts) | 0.17.0 (2026-05-13) | yes (`flate2_rust`) | **yes** (keeps used subrs; optional CID conversion) | **no**. `gsub` is accepted by `parse_custom` but `build_otf` never writes it ([subset.rs L440–L520](https://github.com/yeslogic/allsorts/blob/efab0fc769287f4ced68bbd723c328c9d9b89698/src/subset.rs#L440-L520)) | no | read only | **Use it for CFF only.** |
| [subsetter](https://crates.io/crates/subsetter) (typst) | 0.2.6 (2026-06-04) | yes | yes | no | no | no | No. "unusable in any other contexts than PDF writing" ([README](https://github.com/typst/subsetter/blob/b0adcc1cdf5283d8a4014ff31c174f1ca5deced4/README.md)). OTS: "OS/2: missing required table". |
| [font-subset](https://crates.io/crates/font-subset) | 0.1.0 (2026-02-02) | yes | **no** (needs `glyf`) | no ("drops advanced layout tables") | no | yes | No. It errors on `OTTO` ([README](https://github.com/slowli/font-tools/blob/40a809aebab5ab1bc2c58927304d3203d558a149/crates/font-subset/README.md)). |
| [woofwoof](https://crates.io/crates/woofwoof) | 1.0.2 (2026-01-02) | **no**: Google woff2 C++ through `cc`, Rust Brotli ([build.rs](https://github.com/fasterthanlime/woofwoof/blob/5313ae379f48038f381fbacb23b8d468d6262824/build.rs)) | n/a | n/a | keeps it intact (spike) | yes, CFF and TTF | Works. It is the fallback if we refuse to patch ttf2woff2. |
| [ttf2woff2](https://crates.io/crates/ttf2woff2) | 0.13.3 (2026-09-10) | yes | **rejects `OTTO`** ([sfnt.rs L37](https://github.com/0x6b/ttf2woff2/blob/f10b4d7304d62a22dc4836677adf4f96f00a8c5a/src/woff2/sfnt.rs#L37)) | n/a | keeps it intact once patched (spike) | TTF only | **Use it with the one-line `OTTO` patch.** |
| [woff](https://crates.io/crates/woff) (bodoni) | 0.6.3 | no (C++ woff2, C Brotli, zlib) | | | | yes | No. |
| [wuff](https://crates.io/crates/wuff) | 0.2.9 | yes | | | | **decode only** | Useful in tests. |
| [hb-subset](https://crates.io/crates/hb-subset) | 0.3.0 (2023) | no (HarfBuzz) | | | | | Out of scope. |

### Size (WOFF2 bytes)

Fonts are the patched builds from math-core's playground: New Computer Modern Math (`NewCMMath-Book-prime-roundhand-vec.otf`, 8,598 glyphs), Libertinus Math (4,221 glyphs) and Noto Sans Math (5,138 glyphs) ([playground/fonts](https://github.com/tmke8/math-core/tree/fe8ce3e0c73acf03b4e01d174438a9b96c9abb97/playground/fonts)). All three are CFF-flavoured (`OTTO`).

"Page" is the 40-case math-core corpus page from the build-time-math spike. With [correct character extraction](#which-characters-to-keep) it needs 129 code points. "All symbols" is math-core's [`all_symbols.txt`](https://github.com/tmke8/math-core/blob/fe8ce3e0c73acf03b4e01d174438a9b96c9abb97/scripts/generate_symbol_document.py): every character math-core can emit, 3,552 code points. HarfBuzz used `--layout-features=ssty,kern,aalt --desubroutinize` (math-core's [recommended command](https://github.com/tmke8/math-core/blob/fe8ce3e0c73acf03b4e01d174438a9b96c9abb97/README.md#font-subsetting)) and `woff2_compress`. Rust used the same three features and ttf2woff2 at quality 11.

| Font | Full font | Page: hb-subset 14.5.1 | Page: **Rust** | Page: Rust with CID CFF | All symbols: hb | All symbols: **Rust** | All symbols: Rust with CID CFF |
|---|---|---|---|---|---|---|---|
| New Computer Modern Math | 760,768 | 49,004 (463 glyphs) | **51,792** (463) | 49,468 | 475,996 | **492,240** | 474,108 |
| Libertinus Math | 341,356 | 27,540 (323) | **31,644** (323) | 30,096 | 237,624 | **255,044** | 245,072 |
| Noto Sans Math | 297,040 | 28,780 (554) | **33,812** (572) | 31,320 | 230,848 | **251,240** | 236,196 |

Where the 6–15% gap comes from on small subsets, for New Computer Modern Math page subsets:

- **CFF tables.** allsorts keeps the full String INDEX and local Subrs INDEX entry counts. It blanks the unused entries, but their offset arrays remain (8,374 strings, 2,661 subrs). It also does not desubroutinize. hb-subset compacts both. Without `--desubroutinize`, hb's WOFF2 is 50,160 instead of 49,004.
- **`MATH`.** Our `MATH` is larger (3,362 vs 2,404 bytes) because it keeps per-glyph data that hb drops (see [below](#differences-from-hb-subset)).
- **CID conversion.** Converting to CID-keyed CFF closes most of the gap, but it changed glyph rasterization (see [Rendering](#rendering)), so the recommended default keeps name-keyed CFF.

### Rendering

Method:

- **Test page.** The 40-case page was rendered with each font as a WOFF2 `@font-face`: the full font, the hb subset, the Rust subset, and a **control**. The control is allsorts' output, which has no `MATH`, GSUB or GPOS.
- **Browsers.** Chromium 153.0.8010.12, Firefox 155.0 and WebKit 26.6 (all via Playwright 1.63.0), stable Firefox 157.0 (`--screenshot`), and Safari 27.0.1's WebKit on macOS 27.0.1 (`WKWebView` snapshot).
- **Comparison.** Screenshots were compared pixel by pixel with the full-font render.

Differing pixels against the full font (0 = pixel-identical):

| Browser | Font | hb subset | **Rust subset** | Rust vs hb | Control (no `MATH`) |
|---|---|---|---|---|---|
| Chromium 153 | NewCM / Libertinus / Noto | 0 / 0 / 0 | **0 / 0 / 0** | 0 / 0 / 0 | ≥244k, page height changes |
| Firefox 155 | NewCM / Libertinus / Noto | 0 / 0 / 0 | **0 / 0 / 0** | 0 / 0 / 0 | ≥224k, page height changes |
| Firefox 157 | NewCM / Libertinus / Noto | 0 / 0 / 0 | **0 / 0 / 0** | 0 / 0 / 0 | ≥243k |
| WebKit 26.6 | NewCM / Libertinus / Noto | 2 / 0 / 2 | **2 / 0 / 2** | 0 / 0 / 0 | ≥252k, page height changes |
| Safari 27.0.1 | NewCM / Libertinus / Noto | 0 / 0 / 2 | **0 / 0 / 2** | 0 / 0 / 0 | ≥871k, page height changes |

The 2-pixel residues are anti-aliasing noise: the largest channel difference was 2/255 in WebKit, and the residue sits at the image edge in Safari. They are identical for hb and Rust. The control shows that the test detects a lost `MATH` table.

Before we settled on the final setup, the same test caught three real problems:

1. **Naive character extraction.** Taking only the characters in the source text left out the radical and every `math-auto` italic letter. Chromium dropped the √ and drew upright letters.
2. **Missing U+0020 and U+00AF.** Firefox and WebKit then laid out NBSP text and `\boxed` differently, changing the page height.
3. **CID-keyed CFF from allsorts.** Glyphs shifted by sub-pixel amounts in Chromium, Firefox and WebKit (about 2,500 pixels, max channel difference 155/255). Layout did not change. Keeping name-keyed CFF made Rust and hb pixel-identical.

## Details

### What a subset must keep

- **A consistent `MATH` table.** OTS validates `MATH` and checks every variant and part glyph id against `numGlyphs` ([math.cc L381–L382, L430–L431](https://github.com/khaledhosny/ots/blob/62634335137d86473c84b20a8de9b9d97645bbd6/src/math.cc#L381)). On failure it calls `Drop("failed to parse MathVariants table")`, which logs "Table discarded" ([ots.cc `Table::Drop`](https://github.com/khaledhosny/ots/blob/62634335137d86473c84b20a8de9b9d97645bbd6/src/ots.cc#L1141)).
  - OTS "is integrated into Chromium and Firefox" ([README](https://github.com/khaledhosny/ots/blob/62634335137d86473c84b20a8de9b9d97645bbd6/README.md)). A stale `MATH` table therefore disappears **silently** in those two browsers, while Safari may still use it.
  - This is probably what math-core's README means by "I've tried other woff2 compressors, but they seemed to break the math table" ([README L199](https://github.com/tmke8/math-core/blob/fe8ce3e0c73acf03b4e01d174438a9b96c9abb97/README.md#font-subsetting)).
  - Both WOFF2 encoders we tested return `MATH` byte-identical after decoding, and OTS keeps it.
- **`ssty` and the glyphs it reaches.** math-core's README says `ssty` is needed for correct rendering in Firefox, which is "currently" the only browser that uses it. Dropping it roughly halves the all-symbols NewCM subset (450 KB to 200 KB) ([README § Font subsetting](https://github.com/tmke8/math-core/blob/fe8ce3e0c73acf03b4e01d174438a9b96c9abb97/README.md#font-subsetting)). math-core-fonts extends Libertinus's `ssty` to 73 glyphs ([README § Extending ssty](https://github.com/tmke8/math-core-fonts/blob/2af58a8fcfd00dd250bdf9b67caaa8bc4751ccb8/README.md#extending-ssty)), so these glyphs are real content.
- **CFF outlines.** All three patched fonts are CFF, and math-core-fonts builds `.otf` files ([README § Building](https://github.com/tmke8/math-core-fonts/blob/2af58a8fcfd00dd250bdf9b67caaa8bc4751ccb8/README.md#building)). math-core-fonts has **no releases**, only FontForge and Python build scripts ([releases](https://github.com/tmke8/math-core-fonts/releases)). The Engine therefore has to build or vendor the OTFs itself.

### The off-the-shelf crates

- **skera 0.8.0** ([fontations/skera](https://github.com/googlefonts/fontations/tree/skera-v0.8.0/skera)) is the Rust port of hb-subset. It was renamed from klippa on 2026-02-10 ([commit 98e4fe1](https://github.com/googlefonts/fontations/commit/98e4fe13f95f26b768b08c186dde27f5d0495918)) and has had 13 releases since then (0.1.0 to 0.8.0).
  - Its table dispatch has arms for glyf, gvar, GSUB, GPOS, GDEF, cmap, hmtx, COLR, CBLC and others. There is no `CFF `/`CFF2` arm and no `MATH` arm, so those fall through to `_ => Ok(())` and are dropped. With `SUBSET_FLAGS_PASSTHROUGH_UNRECOGNIZED` they are copied verbatim instead ([lib.rs L1283–L1426](https://github.com/googlefonts/fontations/blob/skera-v0.8.0/skera/src/lib.rs#L1283-L1426)).
  - `SUBSET_FLAGS_DESUBROUTINIZE` is documented as "UNIMPLEMENTED yet" ([L242](https://github.com/googlefonts/fontations/blob/skera-v0.8.0/skera/src/lib.rs#L242)).
  - Spike on NewCM: default flags gave 276 glyphs with no outline table, and OTS answered "no supported glyph data table(s) present". With passthrough, OTS rejected the CFF.
  - What it does well: it subsets GSUB, GPOS and GDEF exactly like HarfBuzz. On the NewCM page subset, its GSUB, GPOS and GDEF came out the same size as hb-subset's (1,080, 30 and 22 bytes on the corpus run).
- **fontcull 2.0.1** is "powered by klippa" and vendors forks (`fontcull-klippa 0.1.2`, `fontcull-read-fonts 0.38`) ([Cargo.toml](https://github.com/bearcove/fontcull/blob/b5d9b45c7b3e39d50dae76b8c221b7e516b71fe7/fontcull/Cargo.toml)). Its vendored klippa has the same "skip glyph closure for MATH" gap. In the spike its output had 269 glyphs but the full 8,598-glyph CFF and the original `MATH`, and OTS rejected it. The repository says it "has been absorbed into Dodeca" ([README](https://github.com/bearcove/fontcull/blob/b5d9b45c7b3e39d50dae76b8c221b7e516b71fe7/README.md)), and Dodeca's copy still vendors the same klippa.
- **allsorts 0.17.0** (YesLogic, used by Prince) subsets CFF properly: it rewrites the CharStrings and keeps only the subroutines in use ([cff/subset.rs](https://github.com/yeslogic/allsorts/blob/efab0fc769287f4ced68bbd723c328c9d9b89698/src/cff/subset.rs#L73-L160)). But its sfnt wrapper only ever writes cmap, hhea, hmtx, maxp, name, OS/2, post, `CFF ` and the hinting tables ([subset.rs `build_otf`](https://github.com/yeslogic/allsorts/blob/efab0fc769287f4ced68bbd723c328c9d9b89698/src/subset.rs#L440-L520)).
  - We therefore call `CFF::subset` directly and serialize with allsorts' `WriteBinary`.
  - It keeps the **whole** String INDEX: 8,374 glyph names, 110 KB in NewCM. Without pruning, the page subset's CFF was 157 KB instead of 64 KB.
  - The spike replaces unused entries with empty strings through the public `MaybeOwnedIndex::replace`. The public API has no way to compact the index, because `Dict` mutators are private.
- **subsetter 0.2.6** and **font-subset 0.1.0** say outright that they are out of scope (PDF embedding; TrueType-only data URLs).

### MATH subsetting (Engine-owned)

`write-fonts` 0.54.0 can own-convert a `read-fonts` `Math` (`to_owned_table()`), and every subtable is a plain struct with public fields. The spike's `subset_math` filters and renumbers these parts and `dump_table`s the result. It is about 120 lines:

- `MathItalicsCorrectionInfo`
- `MathTopAccentAttachment`
- `extendedShapeCoverage`
- `MathKernInfo`
- the vertical and horizontal `MathGlyphConstruction` lists, including every `MathGlyphVariantRecord` and `GlyphPartRecord`

The result passes OTS. A structural check also passed: for every kept code point, the outline, advance, italics correction, top accent, kerning, extended-shape flag, size variants and assembly parts (compared by outline hash), and the `ssty` closure match the original font, and `MathConstants` is byte-identical. That held for all three fonts and both character sets.

fontations itself plans `MATH` support. Chromium needs a HarfBuzz replacement for its MathML `MATH` parsing if it moves to HarfRust, and the maintainer wrote "We will support the MATH table" ([fontations#1894](https://github.com/googlefonts/fontations/issues/1894)). Read support shipped in read-fonts 0.45.0, but skera has no `MATH` subsetting yet.

### Differences from hb-subset

- **Closure order.** hb-subset runs cmap, then `MATH` closure, then GSUB closure, once ([hb-subset-plan.cc L452–L468](https://github.com/harfbuzz/harfbuzz/blob/da8cf25e5997e6cfebc50ffd5b1da7006e434885/src/hb-subset-plan.cc#L452-L468)). The spike runs cmap, then GSUB, then `MATH` until nothing changes. That keeps a few more glyphs in Noto (572 vs 554 on the page) and is a superset of hb's set.
- **`MATH` filtering.** hb keeps `MathGlyphInfo` records (italics correction, top accent, kerning, extended shape) only for `_glyphset_mathed`, which is cmap plus `MATH` closure. It keeps `MathVariants` constructions only for `_glyphset_cmaped` ([hb-ot-math-table.hh L206, L259, L487, L568, L986](https://github.com/harfbuzz/harfbuzz/blob/da8cf25e5997e6cfebc50ffd5b1da7006e434885/src/hb-ot-math-table.hh#L206)).
  - Glyphs reached only through GSUB, such as `ssty` script forms, lose their `MATH` data in hb's output. Our pipeline keeps it, which is why our `MATH` is larger.
  - Our 40-case page could not tell the two apart, because both rendered pixel-identical to the full font. Keeping the data is the safer choice for Firefox, which is the browser that applies `ssty`.

### WOFF2

- **What the format requires.** WOFF2 defines transforms only for `glyf`, `loca` and `hmtx`. For every other table, "transformation version 0 indicates the null transform where the original table data is passed directly to the Brotli compressor" ([W3C WOFF2 Recommendation](https://www.w3.org/TR/WOFF2/)). A CFF font therefore needs no transform code, only a directory and Brotli.
- **ttf2woff2.** It already handles absent `glyf`. Changing its `flavor != TTF_FLAVOR` check to also accept `0x4F54544F` was the only edit needed.
  - Patched output roundtripped (decoded with `wuff`) with every table byte-identical. Only `head.checkSumAdjustment` changed, which the decoder recomputes.
  - OTS accepted the output and kept `MATH`.
  - Sizes were within ±0.4% of Google's encoder (woofwoof and `woff2_compress`): 51,792 vs 51,840 (NewCM page), 31,644 vs 31,640 (Libertinus page) and 474,108 vs 473,996 (NewCM all symbols, CID).
- **Brotli quality**, for the NewCM page and NewCM all symbols:
  - q5: 56,928 B in 1.8 ms; 562,972 B in 19 ms
  - q9: 56,076 B in 4.2 ms; 544,548 B in 40 ms
  - q11: 51,792 B in 70 ms; 492,240 B in 819 ms
  - Quality 11 is worth it for production. The dev server can use q5 or serve the unsubset font.

### Which characters to keep

The browser draws characters that never appear in the MathML source, and it measures some glyphs for its own layout. Subsetting only the source text breaks rendering. The Engine's extractor must add the following:

1. **MathML Core `math-auto` italics.** A single-character `<mi>` is drawn from the Mathematical Italic block (for example `x` becomes U+1D465 and `h` becomes U+210E). math-core's script builds that table, which has 112 pairs ([extract_math_chars.py `_build_italic_map`](https://github.com/tmke8/math-core/blob/fe8ce3e0c73acf03b4e01d174438a9b96c9abb97/scripts/extract_math_chars.py#L60)).
2. **Implied characters.** `<msqrt>`/`<mroot>` draw U+221A ([`IMPLIED_CHARS`](https://github.com/tmke8/math-core/blob/fe8ce3e0c73acf03b4e01d174438a9b96c9abb97/scripts/extract_math_chars.py#L54)). The script also adds the NFD decomposition of every character.
3. **U+0020 when NBSP is present.** WebKit renders NBSP with the space glyph: `normalizeSpaces` maps `noBreakSpace` to `space` ([FontCascadeInlines.h L139–L142, L176–L179](https://github.com/WebKit/WebKit/blob/ded238657d1beb915e76f06475222ff3f2835005/Source/WebCore/platform/graphics/FontCascadeInlines.h#L139-L179)). math-core emits NBSP in `<mtext>`, and its script does not add U+0020. Firefox needed U+0020 too (spike bisection).
4. **U+00AF and `1` for Gecko.** `nsMathMLFrame::GetRuleThickness` measures U+00AF to get the `menclose` rule thickness ([nsMathMLFrame.cpp L95–L107](https://github.com/mozilla-firefox/firefox/blob/3f73c528a1ae5784ea5e1ee2c5ad3762507395f2/layout/mathml/nsMathMLFrame.cpp#L95-L107)). `menclose` and `mroot` measure `'1'` ([nsMathMLmencloseFrame.cpp L306](https://github.com/mozilla-firefox/firefox/blob/3f73c528a1ae5784ea5e1ee2c5ad3762507395f2/layout/mathml/nsMathMLmencloseFrame.cpp#L306), [nsMathMLmrootFrame.cpp L196](https://github.com/mozilla-firefox/firefox/blob/3f73c528a1ae5784ea5e1ee2c5ad3762507395f2/layout/mathml/nsMathMLmrootFrame.cpp#L196)). `<mo>-</mo>` is remapped to U+2212 ([nsMathMLmoFrame.cpp L125](https://github.com/mozilla-firefox/firefox/blob/3f73c528a1ae5784ea5e1ee2c5ad3762507395f2/layout/mathml/nsMathMLmoFrame.cpp#L125)). Without U+00AF, `\boxed{…}` grew by a pixel in Firefox and shifted everything below it.

Adding U+0020, U+00AF, `1`, U+2212 and U+221A every time costs about 10 bytes of WOFF2. With them, all five browser builds rendered the subsets pixel-identically to the full font.

Because the Engine produces the MathML itself, it can collect the characters while it renders, with no HTML re-parse. It is still worth testing against math-core's script on the same pages.

### Cost

- **Binary.** A stripped LTO binary with just this pipeline is 2,866,000 bytes, against 302,736 for hello-world. That is +2.56 MB.
  - Code by crate: skera 442 KB, brotli 333 KB, read-fonts 142 KB, write-fonts 118 KB, allsorts 64 KB, ttf2woff2 29 KB, plus core/std.
  - If the Engine already ships Brotli (for example for precompressed assets), part of that is shared.
- **Time.** On an Apple M4 Max, closure, skera, allsorts and `MATH` take 0.7–6 ms per font. WOFF2 dominates (see above).
- **Licenses.** All crates are MIT/Apache-2.0, except allsorts (Apache-2.0) and the `brotli` crate (BSD-3-Clause AND MIT).

## Recommendation

1. **Build the Engine's math-font pipeline from:**
   - read-fonts 0.45 for the closure
   - skera 0.8 for the non-outline tables, with explicit gids and `NO_LAYOUT_CLOSURE`
   - allsorts 0.17 for name-keyed CFF, with unused strings blanked
   - an Engine-owned `MATH` subsetter on write-fonts 0.54
   - ttf2woff2 0.13 patched to accept `OTTO`, or woofwoof if we accept a C++ build step for the Engine's own release builds (Authors never compile)
2. **Fail the build loudly** if skera's glyph count differs from the closure, if any `MATH` glyph id falls outside the map, or if a requested code point is missing from the subset cmap.
3. **Subset once per Site, not per page.** Take the union of characters from all pages and cache by (font hash, character-set hash, crate versions). Per-page subsets would cost more requests and lose cache hits across pages.
4. **Test the pipeline.** Snapshot tests should run OTS and the structural check (outline, advance, `MATH`, `ssty` equivalence per code point) on every font in the Starter.
5. **Upstream work** that would remove our custom code:
   - an `OTTO` PR for ttf2woff2
   - String and Subrs INDEX compaction plus desubroutinization for allsorts, which closes most of the size gap with hb
   - `MATH` subsetting in skera, which fontations has said it will support

## Open questions

1. **Font provenance.** math-core-fonts publishes no binaries and builds with FontForge and Python. Should the Engine repo build and vendor the three OTFs (pinned to a math-core-fonts commit) at Engine-release time? Or should it embed math-core's playground OTFs?
   - New Computer Modern Math is under the GUST Font License, which "requests" renaming derived works ([GUST-FONT-LICENSE.txt](https://github.com/tmke8/math-core-fonts/blob/2af58a8fcfd00dd250bdf9b67caaa8bc4751ccb8/NewComputerModernMath/GUST-FONT-LICENSE.txt)).
   - Libertinus is OFL with Reserved Font Names "Linux Libertine", "Biolinum" and "STIX Fonts" ([OFL.txt](https://github.com/tmke8/math-core-fonts/blob/2af58a8fcfd00dd250bdf9b67caaa8bc4751ccb8/LibertinusMath/OFL.txt)).
   - These need a licensing check for shipping subsets.
2. **Should `MATH` follow hb's tighter filtering** (drop `MathGlyphInfo` for glyphs reached only through GSUB, saving about 1 KB on small subsets), or keep the superset? Only a Firefox `ssty` test with italics correction and kerning on script glyphs can decide this.
3. **CID versus name-keyed CFF.** CID conversion saves about 5% but changed rasterization. Is a hinting-related shift acceptable if it is not visible at normal zoom? Our acceptance bar is pixel identity with the full font.
4. **Author-supplied math fonts.** These may be TrueType-flavoured (`glyf`), for which skera handles outlines natively and ttf2woff2 works unpatched. They may also lack `ssty` or be CFF2. Supporting them would need the same structural tests per font.
5. **Dev server.** Should it serve the full font (fastest, and correct by definition) and subset only in production builds?
6. **The build-time-math note's 21 KB figure** should be corrected to the numbers above, and math-core's `extract_math_chars.py` could be patched upstream to add U+0020 (for NBSP) and U+00AF.

## Appendix: spike

Throwaway code and outputs are in `/private/tmp/fontspike`:

- `spike/` (Rust) has these subcommands:
  - `subset`: the recommended pipeline
  - `probe`: runs every off-the-shelf crate
  - `woff2`: compares encoders with a `wuff` roundtrip
  - `check`: the structural equivalence check
  - `cffinfo`
- `spike/ttf2woff2-otto/` is ttf2woff2 0.13.3 with the `OTTO` patch.
- `sz/` and `szbase/` hold the binary-size measurement.
- `render/` holds the test pages, the Playwright and `WKWebView` screenshot scripts, `diff.py`, and every screenshot in `shots/`.
- `out/hb`, `out/rust`, `out/probe` and `out/w2` hold the fonts.

Tools used only for comparison and validation, never in the Engine path:

- HarfBuzz `hb-subset` 14.5.1
- Google `woff2_compress` 1.0.2 (Homebrew)
- OTS `ots-sanitize` 9.2.0 (PyPI `opentype-sanitizer`)
- Playwright 1.63.0
- Firefox 157.0

Firefox only launched with `CFFIXED_USER_HOME` set, because it could not create `~/Library/Application Support/Firefox` in this environment.
