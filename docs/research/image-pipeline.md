# Responsive image pipeline: Rust encoders vs sharp

Ticket: [#8](https://github.com/yuann3/yuanme/issues/8). Researched 2026-10-04.

## Question

Which Rust decoders, resizers and encoders give the Engine the fastest build-time responsive-image pipeline (AVIF, lossy WebP, JPEG)? How do they compare with sharp/libvips on the Reference site's About-page workload in speed and quality? How should results be cached so unchanged images are never re-encoded? And what do the C dependencies mean for cross-compilation and licensing?

## Short answer

Use a mostly-Rust stack with exactly one C library, libwebp:

| Stage | Crate (version, released) | Settings |
|---|---|---|
| Decode + orientation | `image` 0.25.10 (2026-03-10), which uses `zune-jpeg` for JPEG | `ImageDecoder::orientation()` + `DynamicImage::apply_orientation()` |
| Resize | `fast_image_resize` 6.1.0 (2026-07-21) | Lanczos3, sRGB (non-linear), like sharp |
| AVIF | `ravif` 0.13.0 (2026-01-19) on `rav1e` 0.8.1 | speed 6, quality 72, 4:4:4, 10-bit, **fixed tile/thread count** |
| WebP | `libwebp-sys` 0.14.4 (2026-04-29, bundles libwebp 1.6.0), called directly | quality 80, method 4 (sharp's defaults) |
| JPEG | `mozjpeg-rs` 0.9.2 (2026-04-17, pure Rust) | baseline, Annex K tables, optimised Huffman, no trellis, q80, 4:2:0 (libvips' defaults) |
| Cache key | `blake3` 1.8.7 | hash of source bytes, transform, encoder ID and version |

Measured on the About workload (4 photos × 5 widths × 3 formats = 60 outputs, Apple M4 Max), this stack does the whole job (decode, resize, encode, write) in **2.5 s on one core, 0.71 s on 4 threads and 0.24 s on 16**. sharp takes **4.4 s, 1.45 s and 0.64 s** for the same 60 outputs. At equal SSIMULACRA2 quality the WebP output is byte-identical to sharp's and the JPEG output is pixel-identical. AVIF is the one trade-off: rav1e files are about 12% larger than libaom's at sharp's default effort, but cost 0.58× the CPU.

The biggest CI saving does not come from changing encoders. **70% of today's CI AVIF time (17.8 s of 25.6 s of summed transform time) goes to the four full-size AVIFs that no page references.** Not generating them, plus the faster stack, should bring the cold image step from 9.34 s to about 1.5–2 s on the 4-vCPU runner (an estimate, derived below). With a content-hash cache, a warm build re-encodes nothing.

Do not use the AGPL `zen*` crates, the unreleased `image-webp` lossy encoder (it produced corrupt output in this test) or the `webp` crate (it pins libwebp 1.3.1). Keep libaom in reserve: at equal size and quality it is about 4× faster than rav1e, but it brings cmake, nasm and C++ into release CI through stale binding crates.

## Comparison tables

Method in brief (details under [Method](#method)): every encoder got the **same pixels**, the 20 Lanczos3-resized images from `fast_image_resize`. Quality is the mean SSIMULACRA2 of the decoded output against those pixels. Time is single-core encode time for all 20 images, min of 2–3 interleaved runs, on Apple M4 Max (macOS, rustc 1.99.0, sharp 0.34.5 with libvips 8.17.3, aom 3.13.1, libwebp 1.6.0 and mozjpeg 0826579). SSIMULACRA2 reads 90 = visually lossless, 80 = very high, 70 = high, 50 = medium ([cloudinary/ssimulacra2 README](https://github.com/cloudinary/ssimulacra2/blob/main/README.md)).

### AVIF (20 images, total bytes)

| Encoder, settings | Bytes | SSIMULACRA2 mean (min) | 1-core encode | vs sharp default |
|---|---:|---:|---:|---|
| sharp (libheif + aom) q50 effort 4 (**Astro default**) | 318,188 | 73.56 (67.63) | 3,663 ms | baseline |
| sharp (aom) q50 effort 6 | 314,080 | 73.77 (68.11) | 10,049 ms | −1% bytes, 2.7× CPU |
| sharp (aom) q54 effort 2 | 358,251 | 73.78 (67.72) | 511 ms | +13% bytes, 0.14× CPU |
| libaom via `libavif` 0.14 / `libavif-sys` (libavif 1.0.4, libaom 3.11.0) speed 5 q50 | 319,136 | 73.62 (67.67) | 3,350 ms | same as sharp |
| **ravif speed 6 q72** | 357,554 | 73.80 (68.22) | 2,116 ms | **+12% bytes, 0.58× CPU** |
| ravif speed 4 q72 | 351,085 | 74.04 (68.60) | 2,701 ms | +10% bytes, 0.74× CPU |
| ravif speed 8 q72 | 365,541 | 73.75 (68.39) | 1,789 ms | +15% bytes, 0.49× CPU |
| ravif speed 10 q80 | 497,568 | 72.60 (63.44) | 473 ms | +56% bytes, 0.13× CPU |

ravif's quality scale is not sharp's: ravif q72 ≈ sharp q50 here. ravif speed 6 q72 at ~73.8 is the setting that matches the Astro output's quality.

### WebP (lossy)

| Encoder | Licence | Bytes | SSIMULACRA2 | 1-core encode |
|---|---|---:|---:|---:|
| sharp q80 effort 4 (Astro default) | — | 490,058 | 76.87 | 194–208 ms |
| **libwebp 1.6.0 via `libwebp-sys` 0.14.4**, q80 m4 | BSD-3 + patent grant | **490,058 (byte-identical to sharp)** | 76.87 | 179 ms |
| `zenwebp` 0.4.4 q80 m4 (pure Rust) | **AGPL-3.0 or commercial** | 503,622 | 76.99 | 242 ms |
| `image-webp` main @ f4d80bd, lossy q80 (pure Rust, unreleased) | MIT/Apache | 514,550 | **29.25 (min −35)** | 67 ms |

### JPEG (q80, 4:2:0)

| Encoder, mode | Bytes | SSIMULACRA2 | 1-core encode | Notes |
|---|---:|---:|---:|---|
| sharp (libvips + mozjpeg, `JCP_FASTEST`) | 640,818 | 76.91 | 24–29 ms | Astro default |
| `mozjpeg` 0.10.13 (C) fastest + optimise coding | 641,178 | 76.91 | 17 ms | pixel-identical to sharp |
| **`mozjpeg-rs` 0.9.2** fastest + optimise Huffman + Annex K | 642,629 | 76.91 | 21 ms | **pixel-identical to sharp**, pure Rust |
| `jpeg-encoder` 0.7.1, optimised Huffman | 641,674 | 75.93 | 23 ms | about 1 point worse at the same size |
| `mozjpeg` (C) default (progressive, trellis, Robidoux tables) | 523,967 | 75.75 | 212 ms | 18% smaller, 1.2 points lower |
| `mozjpeg-rs` `max_compression()` | 525,575 | 75.70 | 337 ms | |

### Decode and resize (20 resizes from 4 sources, 1 core)

| Step | Time |
|---|---:|
| Decode 4 JPEGs (6.3 MP) with `image` / `zune-jpeg` | 80–100 ms |
| Resize ×20, `fast_image_resize` Lanczos3 | 146–185 ms |
| Resize ×20, `image::imageops::resize` Lanczos3 | 1,298–1,368 ms (≈8× slower) |

The fir resize and sharp's resize (libvips with JPEG shrink-on-load) differ slightly: SSIMULACRA2 between them is 90.36 (min 85.64), i.e. visually lossless.

### End-to-end, 60 outputs (decode + resize + encode + write)

| Pipeline | 1 thread | 4 threads | 16 threads |
|---|---:|---:|---:|
| Rust: ravif s6 + libwebp + mozjpeg-fast, default tiling | 2.47 s | 0.71 s | 0.22–0.24 s |
| Rust: same, ravif fixed at 4 threads (deterministic bytes) | n/a (ravif makes its own 4-thread pool) | 0.40–0.50 s | 0.33 s |
| Rust: same, ravif single tile (deterministic, across-image parallelism only) | 2.51 s | 0.76–0.96 s | 0.74–1.06 s |
| sharp, all 60 queued (`UV_THREADPOOL_SIZE`=1/4, `sharp.concurrency(1)`) | 4.37–4.71 s | 1.45–2.07 s | — |
| sharp, Astro's scheduling (4 per-photo chains, libvips default threads) | — | — | 0.64–1.0 s |

The machine was shared with other jobs during the run (load average 6–18). Single-core numbers stayed within ±3% across interleaved runs. The multi-thread numbers are noisier, so the ranges above are min–max.

## Details

### 1. Where today's time goes

- Astro's sharp service applies `.rotate()` and then `toFormat(format, { quality })` with no other options, so sharp's defaults apply (eyuan.me `node_modules/astro/dist/assets/services/sharp.js`, Astro 5.16.5): JPEG q80, WebP q80 effort 4, AVIF q50 effort 4, chroma 4:4:4, 8-bit ([sharp 0.34.5 lib/output.js](https://github.com/lovell/sharp/blob/v0.34.5/lib/output.js), `avif()`/`heif()` JSDoc). By default sharp converts to sRGB and strips all metadata, ICC included (same file, `keepMetadata` JSDoc).
- libvips sets mozjpeg to `JCP_FASTEST` before `jpeg_set_defaults`. That means libjpeg-turbo behaviour: baseline, Annex K tables, no trellis ([libvips 8.17.3 vips2jpeg.c](https://github.com/libvips/libvips/blob/v8.17.3/libvips/foreign/vips2jpeg.c), lines 578–591). This is why `mozjpeg-rs`'s fastest mode with Annex K tables reproduces sharp's pixels exactly.
- Astro runs one queue task per source image, with the transforms inside it in sequence. Each transform re-decodes the source (`generateImagesForPath` in `astro/dist/assets/build/generate.js`, and `PQueue({ concurrency: os.cpus().length })` in `astro/dist/core/build/generate.js`, both in eyuan.me's `node_modules`, Astro 5.16.5).
- CI run [37196745804](https://github.com/yuann3/eyuan.me/actions/runs/37196745804) (ubuntu-24.04, which has 4 vCPU for a public repo per [GitHub's runner specs](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)) took 9.34 s for 72 transforms. Summing the per-transform times in its log gives AVIF 25.56 s, WebP 1.15 s and JPEG 0.34 s. **The four largest AVIF transforms, which are the full-size AVIFs, take 17.78 s.** The parity inventory notes that no page references the full-size AVIF/WebP ([astro-parity-inventory.md](astro-parity-inventory.md), "About images"). Locally the same 72 transforms take sharp 11.4 s on one core, and the 60 referenced ones take 4.4 s.
- Pixel arithmetic agrees: the 20 width variants total about 4.3 MP, while the four full-size originals total 7.2 MP.

**CI estimate for the Engine (derived, not measured on a runner).** Drop the full-size AVIF/WebP. That leaves about 7.8 s (AVIF widths) + about 0.9 s (WebP) + 0.3 s (JPEG) ≈ 9 s of summed sharp transform time. Astro's run had a summed-to-wall ratio of 27.05 / 9.34 ≈ 2.9, so that is roughly 3 s of wall time. ravif s6 needs 0.58× aom's AVIF CPU, which gives about 5.5 s summed and **about 1.5–2 s wall**. x86_64 SIMD paths differ from the NEON paths measured here, so a one-off run of the spike on `ubuntu-latest` should confirm the figure (see open questions).

### 2. AVIF

- **ravif/rav1e.** `ravif` 0.13.0 (2026-01-19, BSD-3-Clause) wraps `rav1e` 0.8.1 (2025-06-16, BSD-2-Clause). It always encodes 4:4:4 and defaults to 10-bit ([ravif src/av1encoder.rs](https://github.com/kornelski/cavif-rs/blob/main/ravif/src/av1encoder.rs): `ColorModel::YCbCr` doc, `BitDepth::Auto` = Ten). Both repositories are active (rav1e pushed 2026-10-01, cavif-rs 2026-09-05, per the GitHub API). Forcing 8-bit at s6 q72 gave 359,919 B at 73.18 against 10-bit's 361,533 B at 73.80, so keep 10-bit.
- **Determinism trap.** ravif picks `tiles = min(threads, area / min_tile_size²)` and takes `threads` from the global rayon pool when unset (av1encoder.rs, around lines 650–655). The bytes of the same build therefore change with the machine's core count. Measured totals were 1,475,335 / 1,479,467 / 1,480,489 bytes at 1 / 4 / 16 threads, each stable across runs. `with_num_threads(Some(k))` with a constant `k` made the output identical at every pool size, and 4 tiles cost only +0.3% in bytes. sharp/aom does the same: 314,484 bytes with one libvips thread and 321,378 with 16.
- **libaom is faster for the same result.** aom at effort 2 matches ravif s6 on size and quality (358 KB at 73.8) with a quarter of the CPU (0.51 s vs 2.12 s). At effort 4 it is 12% smaller for 1.7× the CPU. The Rust route to libaom is `libavif` 0.14.0 / `libavif-sys` 0.17.0, which bundles libavif 1.0.4 and was released 2024-07-05, plus `libaom-sys` 0.17.2, which bundles libaom 3.11.0 and was released 2025-03-16 ([crates.io](https://crates.io/crates/libavif-sys)). It builds through the `cmake` crate. It built and matched sharp here (speed 5 q50: 319,136 B, 73.62, 3.35 s), but the bundled C is a year or more behind upstream (sharp ships aom 3.13.1).
- **Not considered further.** `zenavif` 0.1.6 is AGPL-3.0-only or commercial. `rav1d` 1.1.0 is a decoder only.

### 3. WebP

- **libwebp through `libwebp-sys` 0.14.4.** It bundles libwebp 1.6.0 (`vendor/README.md` banner; `WEBP_ENCODER_ABI_VERSION 0x0210`), the same version as sharp, and builds with the `cc` crate, with no cmake or nasm ([build.rs](https://github.com/NoXF/libwebp-sys/blob/master/build.rs)). An optional `system-dylib` feature links the system libwebp through pkg-config. The output was byte-identical to sharp's on all 20 images. The safe `webp` 0.3.1 wrapper (2025-08-29) depends on `libwebp-sys ^0.9.3`, which bundles libwebp 1.3.1 (`vendor/NEWS`). That version does carry the CVE-2023-4863 Huffman fix (`VP8LHuffmanTablesAllocate` is present), but it is four libwebp releases old. Zola uses `webp` 0.3 ([zola Cargo.toml](https://github.com/getzola/zola/blob/master/Cargo.toml)). The Engine should instead call `libwebp-sys` 0.14 through a roughly 40-line safe wrapper, as the spike did.
- **`image-webp`.** The lossy VP8 encoder was merged to `main` on 2025-10-19 ([PR #161](https://github.com/image-rs/image-webp/pull/161)) but has not been released: crates.io's latest is 0.2.4 (2025-08-27), whose README says "This crate only supports lossless encoding". [Issue #191](https://github.com/image-rs/image-webp/issues/191) (opened 2026-09-20, still open) reports a silently corrupt bitstream at q85, and the follow-up fixes are not on `main`: [#187](https://github.com/image-rs/image-webp/pull/187) and [#188](https://github.com/image-rs/image-webp/pull/188) were closed unmerged, and [#192](https://github.com/image-rs/image-webp/pull/192) is open. At commit f4d80bd and q80, 10 of the 20 About images scored below 40, with visible colour errors and a green line at the bottom edge. It is the fastest encoder in the test (67 ms) and is permissively licensed, so it is the one to re-test once a release lands.
- **`zenwebp`.** Pure Rust, with libwebp-level quality and speed in this test. Licensed `AGPL-3.0-only OR LicenseRef-Imazen-Commercial` ([crates.io](https://crates.io/crates/zenwebp)), and 0.4.5 is yanked.

### 4. JPEG

- **`mozjpeg-rs`** 0.9.2 (imazen, BSD-3-Clause, `#![forbid(unsafe_code)]`) states "byte-identical output to C mozjpeg in baseline and progressive modes" ([README](https://github.com/imazen/mozjpeg-rs)). Here its pixels matched both C mozjpeg and sharp; the bytes differ only in headers. The risk is maintenance: the repository was pushed 2026-09-29 but has 3 stars and about 93k recent downloads. Its LICENSE carries the IJG clause.
- **`jpeg-encoder`** 0.7.1 (2026-07-27, `(MIT OR Apache-2.0) AND IJG`) is the widely used fallback, with 1.4M recent downloads. It scored about 1 point lower at the same size.
- **The C `mozjpeg` crate** 0.10.13 (2025-02-18) builds through `mozjpeg-sys`. On x86, if nasm is missing it prints a cargo warning and builds **without SIMD** ([mozjpeg-sys src/build.rs](https://github.com/kornelski/mozjpeg-sys/blob/main/src/build.rs), `nasm_supported`). Upstream [mozilla/mozjpeg](https://github.com/mozilla/mozjpeg) was last pushed 2025-06-23. It has no advantage over `mozjpeg-rs`.
- JPEG is only the `<img>` fallback for browsers without AVIF/WebP, so trellis/progressive (18% smaller, 10–15× slower) is not worth it by default.

### 5. Decode, orientation, colour

- `image` 0.25 decodes JPEG with `zune-jpeg` (0.5.15, active). `ImageDecoder::orientation()` and `DynamicImage::apply_orientation()` were added in 0.25.4 (`CHANGES.md`). That replaces sharp's `.rotate()`; the About photos have no orientation tag.
- Since 0.25.7, `image` depends on `moxcms` for CICP-aware colour (`CHANGES.md`: "The support for transforming is limited for now"). sharp converts ICC-tagged input (for example iPhone Display P3) to sRGB by default. The Engine must decide whether to do the same, using `moxcms` directly. All four About photos are already sRGB.
- **Do the work once.** Decode each source once and resize once per width, then fan the three encodes out on a rayon pool. Astro instead re-decodes per transform. Decode and resize together cost about 0.25 s of one core for the whole About workload.

### 6. Caching so unchanged images are never re-encoded

Design:

1. **Content-addressed key.** `blake3(source bytes ‖ canonical transform {width, fit, format, quality, speed, tiles, bit depth, subsampling} ‖ encoder crate+version ‖ Engine pipeline version)`. Hash the bytes, not mtimes. Git does not preserve modification times, so every CI checkout looks "newer" ([Git FAQ](https://git.wiki.kernel.org/index.php/Git_FAQ#Why_isn.27t_Git_preserving_modification_time_on_files.3F)). Do not use `std::hash::DefaultHasher` either; its docs say "The internal algorithm is not specified, and so it and its hashes should not be relied upon over releases" ([std docs](https://doc.rust-lang.org/std/hash/struct.DefaultHasher.html)). Zola's image cache does both: it hashes the path string plus options with `DefaultHasher` and checks staleness by mtime ([components/imageproc/src/helpers.rs](https://github.com/getzola/zola/blob/master/components/imageproc/src/helpers.rs) `get_processed_filename`; [processor.rs](https://github.com/getzola/zola/blob/master/components/imageproc/src/processor.rs) `file_stale`). Hashing the About sources (about 0.87 MB) costs nothing next to one encode.
2. **URLs derive from the key, not the output bytes.** The HTML then needs only the key and the intrinsic size, which comes from a header-only read (`image::image_dimensions`). Pages can render while the encodes run in the background. An encoder upgrade changes the key and so busts the URL, which CDN `immutable` caching needs.
3. **Store.** `<cache dir>/images/<key>.<ext>` plus a small manifest (key → width, height, bytes). On a hit, copy or reflink the file into the output, as Astro does with `COPYFILE_FICLONE`. After each build, delete entries this build did not reference (mark and sweep), so the cache doesn't grow without bound. Hugo by contrast never expires by default (`maxAge: -1`, [Hugo caches](https://gohugo.io/configuration/caches/)).
4. **Deterministic encodes.** Fix ravif's thread/tile count (section 2), so that a cache filled on one machine is byte-for-byte what any machine would produce. Byte-identity across CPU architectures (x86 asm vs NEON) is unverified.
5. **CI.** Persist the cache dir with [actions/cache](https://github.com/actions/cache), keyed on the Engine version and `hashFiles` of the image sources, with a `restore-keys` prefix fallback. Because a cold run is about 2 s, this is an optimisation, not a requirement. Committing derivatives (parity open question 26) becomes unnecessary.

### 7. Cross-compilation

ADR 0002 has Authors install a prebuilt binary, so the C toolchain cost falls on the Engine's release CI and on `cargo install` users, not on Authors.

| Dependency | Needs at build time | Cross-compile notes |
|---|---|---|
| `image`, `zune-jpeg`, `fast_image_resize`, `mozjpeg-rs`, `jpeg-encoder`, `blake3` | Rust only | none (blake3 compiles C/asm SIMD by default via `cc`, and has a pure-Rust fallback) |
| `rav1e` (via ravif `asm` feature, on by default) | **x86_64: nasm ≥ 2.14.02**, and the build **panics** without it ([rav1e README](https://github.com/xiph/rav1e#dependency-nasm); build.rs "NASM build failed… disable the \"asm\" feature"). aarch64: `.S` files through `cc` | nasm is a host tool that emits target objects, so cross builds need only nasm on the host. Without `asm`, rav1e is pure Rust but slower (not measured) |
| `libwebp-sys` 0.14 | a C compiler for the target (`cc` crate) | works with `cargo zigbuild`, which provides `zig cc` and can target a chosen glibc ([cargo-zigbuild README](https://github.com/rust-cross/cargo-zigbuild)). Static musl is fine |
| (reserve) `libavif-sys` + `libaom-sys` | cmake, nasm (x86), C and C++ | heaviest: cmake toolchain files per target. Avoid unless AVIF speed becomes the bottleneck |
| (rejected) `mozjpeg-sys` | C, nasm on x86 for SIMD | silently loses SIMD without nasm |

A clean release build of the full spike (every candidate crate) took 37 s on the M4 Max. libwebp's C build is about 6 s and runs in parallel with Rust compilation.

### 8. Licensing

The Engine is `MIT OR Apache-2.0` (repo README).

| Crate | Licence | Obligation |
|---|---|---|
| image, fast_image_resize, zune-jpeg, rayon | MIT/Apache (zune also Zlib) | notices |
| ravif / rav1e | BSD-3 / BSD-2 **+ AOMedia Patent License 1.0** (`rav1e/PATENTS`) | notices; the patent licence is royalty-free with defensive termination |
| libwebp (via libwebp-sys, MIT) | BSD-3 + Google patent grant (`vendor/COPYING`, `vendor/PATENTS`) | notices |
| mozjpeg-rs | BSD-3-Clause + IJG clause | **must state "This software is based in part on the work of the Independent JPEG Group"** in the docs |
| jpeg-encoder | (MIT OR Apache-2.0) AND IJG | same IJG acknowledgement (its `LICENSE-IJG`) |
| blake3 | CC0 / Apache-2.0 | none |
| **Excluded** | `zenwebp`, `zenjpeg`, `zenavif`, `jpegli-rs` (AGPL-3.0 or commercial); `dssim-core` (AGPL-3.0); `imagequant` (GPL-3.0+) | would force the Engine binary under the AGPL/GPL |

Enforce this with a `cargo-deny` licence allowlist and ship a generated third-party notices file that includes the IJG sentence.

## Method

- Spike: `/private/tmp/yuanme-imgspike` (Rust, release profile with thin LTO), `/private/tmp/yuanme-aomspike` (libaom), and Node scripts that load eyuan.me's own `sharp` 0.34.5. These are throwaway and are not committed.
- Workload: the four `src/assets/about/*.jpg` (sRGB, no orientation tag) at widths 160/240/320/480/640, height by aspect ratio.
- Encoder-only comparisons fed every encoder the same `fast_image_resize` Lanczos3 output, saved losslessly. sharp got the same pixels as raw input, with `sharp.concurrency(1)` and `UV_THREADPOOL_SIZE=1` for single-core times.
- Decoding for scoring: AVIF through macOS ImageIO (`sips`), because sharp's prebuilt libheif rejects 10-bit AV1 ("Bitstream not supported by this decoder"); WebP and JPEG through sharp. On sharp's own 8-bit AVIFs, ImageIO and sharp decoding agreed to within 0.08 points. Scoring used the `ssimulacra2` crate 0.5.1.
- Caveats: one metric, 20 images from 4 photos, Apple Silicon NEON paths only.

## Open questions

1. **Validate on x86_64.** Run the spike once on `ubuntu-latest` to replace the 1.5–2 s CI estimate with a measurement. rav1e and aom both have AVX2 paths, and their relative speed may differ from NEON.
2. **Full-size `<img src>`.** Today the fallback `src` is a full-size re-encoded JPEG (1200–1600 px). Keep it for parity, or point `src` at the largest srcset width as a Parity exception? This ties to parity questions 6 and 26.
3. **Default AVIF quality knobs.** Expose `quality`/`speed` per Site with the defaults above (q72, s6), or a single named preset? Should the Engine translate Astro-style `quality: 50` onto ravif's scale?
4. **When to bring in libaom.** Is +12% AVIF bytes acceptable for a C-free AVIF build? A possible trigger: switch when a Site's cold image step exceeds some budget (for example 10 s in CI), behind the same encoder trait.
5. **Colour management.** Convert ICC/CICP-tagged sources such as Display P3 to sRGB like sharp does, or preserve wide gamut with an embedded profile?
6. **Reproducibility scope.** Must image bytes be identical across machines and architectures, or only per machine? This decides whether cache entries can be shared, for example from CI to a laptop.
7. **`cargo install` without nasm.** Offer a `pure-rust` feature that disables rav1e asm, or document nasm as a build requirement?
8. **Third-party notices.** How does the Engine ship the IJG acknowledgement and other notices (a `yuanme licenses` command, or a file in releases)?
9. **Re-test `image-webp`** once a lossy-encoder release ships with #191 fixed. A permissively licensed pure-Rust WebP would remove the last C dependency.
