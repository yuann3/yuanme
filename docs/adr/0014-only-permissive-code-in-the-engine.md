# Only permissively licensed code goes into the Engine

Everything compiled into the Engine binary must be under a permissive licence. The only exception is MPL-2.0, and only for files the Engine doesn't modify. The allowed licences are MIT, Apache-2.0 (with or without the LLVM exception), BSD-2-Clause, BSD-3-Clause, ISC, Zlib, 0BSD, CC0-1.0, BSL-1.0 and Unicode-3.0. A `cargo deny` check of the committed lockfile enforces this on every PR. Fonts and grammars embedded in the binary follow their own licences, are listed in the third-party notices, and are covered under Consequences.

yuanme is MIT OR Apache-2.0 so that anyone can ship, embed or fork it without a licence review. One EUPL or GPL crate inside the binary would undo that. Google bans EUPL outright as "very similar to the AGPL" ([licence policy](https://opensource.google/documentation/reference/thirdparty/licenses)). EUPL-1.2 leaves it to national copyright law whether static linking makes a derivative work. Its definition of distribution also covers "providing access to its essential functionalities" over a network. Decided in [What is yuanme's dependency and licence policy?](https://github.com/yuann3/yuanme/issues/27).

This reverses the highlighter choice in [Which Rust syntax highlighter gives Catppuccin-quality output as CSS classes?](https://github.com/yuann3/yuanme/issues/5). The Engine uses **ferriki** (MIT OR Apache-2.0), not giallo (EUPL-1.2). When #5 was decided, ferriki 0.10 took about 3 s for the corpus that giallo highlighted in 14.5 ms. ferriki 1.1.1, re-measured on 2026-10-17:
- It matches Shiki 3.19 on every non-whitespace character in every language tested, from the same pinned grammar and theme JSON. giallo matched on 98.1 %.
- It is 1.4× slower than giallo warm (19 ms against 14 ms) and about 2× slower cold.
- It needs no C toolchain. giallo builds `onig_sys` and `zstd-sys` with `cc`.

## Considered Options

- **giallo under EUPL-1.2** (rejected): the fastest exact-parity highlighter, with Zola depending on it. Its licence is the problem above.
- **syntect** (rejected): MIT, but it matched only about 80 % of Shiki's colours, and it has made no release since 2025-09.
- **Allow weak copyleft generally** (rejected): MPL-2.0 is allowed because its copyleft stops at the file, and lightningcss has no permissive equivalent.

## Consequences

- **ferriki's `Highlighter` is `!Send`.** The Engine keeps one per rayon thread. On 16 threads that costs about 0.4 s of CPU on a cold build, for regexes compiled again in each thread. At steady state its speed equals giallo's.
- **Grammars are precompiled at Engine release time.** ferriki's bundled catalogue lacks Lean, Coq, LaTeX and OCaml, so the Engine compiles all 226 Shiki 3.19 grammars with `ferriki-asset-gen` and embeds them. This adds about 6.7 MB to the binary.
- **ferriki has one maintainer, AI-assisted.** Its 1.0 shipped on 2026-10-10, and it has 3 stars. It stays behind the Engine's highlight module and is pinned exactly. Its licence lets the Engine fork it if upstream stalls.
- **Fonts and grammars.** The math font (New Computer Modern Math, GUST Font License) is shipped patched and subset under a new name, with a notice of the changes, as that licence asks. The Engine never subsets New Computer Modern's GPL-3-with-exceptions files: removing glyphs voids their distribution exception. Each grammar's own licence goes into the third-party notices.
- **Repository-only files.** GPL and GFDL files in `corpus/` are test data. They are never compiled in, and every published crate packages an explicit `include` list.
