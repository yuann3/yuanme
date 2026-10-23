# yuanme-org

The Engine's Org parser: a fork of [organic](https://code.fizz.buzz/talexander/organic) (0BSD) by Tom Alexander, taken at `e44be41` (2026-10-21). Upstream's parser has not changed since 2024-04-11. Forking it rather than writing our own was decided in [#30](https://github.com/yuann3/yuanme/issues/30), and vendoring it in [#27](https://github.com/yuann3/yuanme/issues/27). This crate is now the upstream; patches may be offered back, but nothing waits on them.

## What differs from upstream

- It builds on stable Rust. The compare, wasm and parse binaries, the tracing and event-count instrumentation, and the dependencies they pulled in are gone. Only nom 7.1.3 remains.
- `AffiliatedKeywords::keywords` is public, because the Engine reads `#+CAPTION`, `#+NAME` and `#+ATTR_HTML` from it.

`org_mode_samples/` is upstream's test corpus, kept as test data. It is not published.
