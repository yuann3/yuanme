# Front-end toolchain: rolldown/oxc, lightningcss, and Tailwind without Node

Research for [ADR 0003](../adr/0003-zero-node-toolchain.md), ticket [#7](https://github.com/yuann3/yuanme/issues/7). Facts are as of 2026-10-04. Claims about binary size, build time and behaviour come from throwaway spikes built for this ticket (see [Spike setup](#spike-setup)). Every other claim links its primary source.

## Question

Can the Engine embed rolldown and oxc (bundle, transpile TypeScript, minify) and lightningcss (bundle, minify, targets, nesting) as Rust libraries? For each one: is the API stable, how does it resolve a `node_modules` that Bun installed, what does it cost in binary size and compile time, and what is its licence? Then: how can Tailwind CSS v4 run without Node, and what would the Engine have to do to support it?

## Short answer

**Yes, all three embed, and they build on stable Rust.** None of them gives the Rust API a stability promise, so the Engine has to pin exact versions and keep each tool behind its own Engine module.

Recommended toolchain:

1. **JS/TS: the `rolldown` crate, pinned exactly (`=1.2.12` today), used as the only bundler, transpiler and minifier.** It pulls in oxc (parser, TypeScript stripping, minifier) and `oxc_resolver`. In the spike it bundled TypeScript plus npm packages from both of Bun's `node_modules` layouts (hoisted and isolated) and produced byte-identical output from each. The Engine should not add oxc as a separate dependency unless it is pinned to the version rolldown pins.
2. **CSS: `lightningcss` `=1.0.0-alpha.72`, using its bundler with an Engine-written `SourceProvider`.** The stock provider only resolves relative paths. The Engine's provider sends bare `@import` specifiers through `oxc_resolver`, which is already in the build through rolldown. The Engine must always set explicit browser targets. Its default should be Tailwind v4's baseline: Chrome 111, Safari 16.4, Firefox 128.
3. **Tailwind v4: an opt-in, Engine-managed sidecar running the official standalone CLI.** The Engine downloads a version-pinned, checksum-verified binary on first use and caches it. It runs the CLI unminified over a class list the Engine extracts from its own rendered output, then sends the result through the Engine's lightningcss pass. Sites that don't use Tailwind never download it.
   - A second option was proven in a spike: run Tailwind's own JS compiler in an embedded QuickJS. It costs about 1.4 MB and 35–60 ms in-process. Keep it behind the same seam as a later replacement, not as v1, because it brings back the embedded JS runtime that [ADR 0002](../adr/0002-runtime-templates.md) rejected.
   - Rust reimplementations are not ready to be the default.

Approximate cost of 1 and 2 together: **+15 MB** on the stripped release binary (fat LTO), about **281 crates**, about **75 s** for a cold `--release` build and **155–200 s** for a cold fat-LTO build (Apple M4 Max, 16 cores).

## Comparison

| Option | Embeddable from Rust | Rust API stability | Resolves Bun `node_modules` | Binary cost (stripped, fat LTO) | Cold build | Licence | Verdict |
|---|---|---|---|---|---|---|---|
| **rolldown 1.2.12** (includes oxc 0.152 + oxc_resolver 11) | Yes, on crates.io since 1.0.0 (2026-05-22) | None. "will not follow the semver contract", Rust-only issues "will be closed" | Yes: hoisted and isolated (symlinked `.bun` store) both verified | +10.4 MB (10.7 MB total) | 76–95 s release; 198 s fat LTO | MIT | **Use** |
| oxc 0.152 alone (parser, transformer, minifier, codegen) | Yes | 0.x. 59 minor (breaking) releases in 12 months | Not a bundler; needs `oxc_resolver` and a module graph you write yourself | +2.7 MB (3.0 MB total) | 16 s release; 27 s fat LTO | MIT | Only through rolldown |
| swc (`swc_core` 81, `swc_bundler` 56) | Yes | 34 major versions of `swc_core` in 12 months | Through its own resolver | not measured | not measured | Apache-2.0 | Not chosen: Vite's direction is Rolldown/oxc, and churn is worse |
| **lightningcss 1.0.0-alpha.72** (+ oxc_resolver) | Yes | Pre-release `1.0.0-alpha.N` since 2022 | Only through a custom `SourceProvider` (stock one joins relative paths) | +4.8 MB alone (5.1 MB total); +4.4 MB on top of rolldown | 19 s release; 61 s fat LTO | MPL-2.0 (file-level copyleft) | **Use** |
| Tailwind v4.3.3 standalone CLI (sidecar) | No, a separate process | Official CLI | Its own scanner and resolver | 0 in Engine; 80–112 MB download per platform | n/a | MIT | **Use, opt-in** |
| Tailwind compiler (npm `tailwindcss`, pure JS) in embedded QuickJS (`rquickjs` 0.14) | Yes | Tailwind's JS API plus rquickjs 0.x | Engine supplies the stylesheets | +1.1 MB plus a 0.29 MB JS bundle | 20 s release | MIT | Proven; keep as a future swap |
| encre-css 0.21.1 (Rust reimplementation) | Yes | 0.x | n/a | not measured | not measured | MIT | Not default: no CSS-first config (`@theme`, `@apply`), TOML config instead |
| tailwind-css / tailwind-rs, railwind, rswind | Yes | Abandoned (last releases 2022–2024) | n/a | n/a | n/a | MPL-2.0 / MIT | No |

## Spike setup

The spikes live in `/private/tmp/yuanme-spike` and are not committed:

- Each tool is its own Cargo project, built with rustc 1.99.0 stable on an Apple M4 Max with 16 cores.
- Each project has its own clean target directory and the builds ran one after another.
- Two profiles were measured:
  - `release` with `strip = true`.
  - `dist`, which is `release` plus `lto = "fat"` and `codegen-units = 1`.
- "Incremental" means `touch src/main.rs` followed by a `release` rebuild.

| Spike | What it does | Unique crates | Clean `release` | Size `release` | Incremental | Clean `dist` | Size `dist` |
|---|---|---|---|---|---|---|---|
| sp-base | `println!` | 1 | 0.2 s | 0.34 MB | 0.1 s | 1.1 s | 0.30 MB |
| sp-oxc | parse TS → `Transformer` (target es2020) → `Minifier` (mangle, `CompressOptions::smallest()`) → `Codegen` | 118 | 16.1 s | 3.56 MB | 1.0 s | 27.3 s | 3.03 MB |
| sp-lcss | lightningcss `Bundler` + custom `SourceProvider` on `oxc_resolver` → `minify` → `to_css` with targets | 108 | 19.1 s | 7.20 MB | 2.2 s | 60.8 s | 5.12 MB |
| sp-rolldown | `rolldown::Bundler` (browser, ESM, minify, hashed names) | 240 | 94.5 s / 75.8 s (two runs) | 14.36 MB | 9.2 s | 198.3 s | 10.73 MB |
| sp-all | sp-rolldown + sp-lcss in one binary | 281 | 73.2 s / 76.6 s | 20.27 MB | 11.7 s | 155.5 s | 15.08 MB |
| sp-qjs | `rquickjs` running Tailwind 4.3.3's `compile()` | small | 19.8 s | 1.41 MB | – | – | 1.35 MB |

For scale, the Bun 1.4.2 binary on the same machine is 61.9 MB, and the Tailwind v4.3.3 standalone CLI for macOS arm64 is 79.8 MB.

## 1. rolldown (and the oxc inside it)

### Status and API stability

- **Published.** `rolldown` is on crates.io. 1.0.0 was published 2026-05-22, and 24 versions have followed, reaching 1.2.12 on 2026-09-30 at about one release a week ([crates.io versions](https://crates.io/crates/rolldown/versions)). Its owners are Boshen and hyfdev, both core maintainers ([crates.io owners API](https://crates.io/api/v1/crates/rolldown/owners)).
- **1.0 is a JS promise.** Rolldown 1.0 was announced stable on 2026-05-07. The semver promise covers the JS API: "Option names, types, and plugin hook signatures stay backward-compatible". Vite 8 has used Rolldown since its stable release in March 2026 ([VoidZero, Announcing Rolldown 1.0](https://voidzero.dev/posts/announcing-rolldown-1-0)).
- **The Rust crates are explicitly unstable** ([rolldown.rs, Rust crates](https://rolldown.rs/apis/rust-crates)):
  - "The crates will not follow the semver contract. Breaking changes may be introduced freely in any version."
  - The crates get no documentation.
  - "Any issues that only affect for the Rust crates will not be worked on as a team and will be closed." PRs are still accepted.
- **The oxc underneath churns too.** rolldown 1.2.12 depends on `oxc ^0.152.0` ([crates.io deps](https://crates.io/crates/rolldown/1.2.12/dependencies)), and oxc publishes a new 0.x minor every week. Cargo treats each one as breaking: there were 59 in the 12 months to 2026-10-04 ([crates.io](https://crates.io/crates/oxc/versions)). The changelogs record real API breaks, for example:
  - 0.141 split `MetaProperty`.
  - 0.143 added `ArrowFunctionBody`.
  - 0.144 added `ClassHeritage`.
  - 0.149 renamed `ParserReturn::panicked` to `fatal_error`.

  Sources: [oxc CHANGELOG](https://github.com/oxc-project/oxc/blob/main/crates/oxc/CHANGELOG.md), [oxc_transformer CHANGELOG](https://github.com/oxc-project/oxc/blob/main/crates/oxc_transformer/CHANGELOG.md).
- **Still small to call.** The Engine-facing surface is `Bundler::new(BundlerOptions) -> BuildResult<Bundler>` plus `write()`, `generate()` and `close()`, which are async ([source](https://docs.rs/crate/rolldown/1.2.12/source/src/bundler/impl_bundler_build.rs)). `BundlerOptions` mirrors the JS options: `input`, `cwd`, `platform`, `format`, `minify`, `entry_filenames`, `resolve`, `tsconfig`, `transform`, `define`, `treeshake` and so on ([source](https://docs.rs/crate/rolldown_common/1.2.12/source/src/inner_bundler_options/mod.rs)). The spike compiled on the first try against the crate's own `examples/basic.rs`.

  ```rust
  let mut bundler = Bundler::new(BundlerOptions {
    input: Some(vec![InputItem { name: Some("app".into()), import: "./src/main.ts".into() }]),
    cwd: Some(site_root), dir: Some("dist".into()),
    platform: Some(Platform::Browser), format: Some(OutputFormat::Esm),
    minify: Some(RawMinifyOptions::Bool(true)),
    entry_filenames: Some("[name]-[hash].js".to_string().into()),
    ..Default::default()
  })?;
  let out = bundler.write().await?; // out.assets, out.warnings
  ```

### TypeScript, minification and what it does not do

- **Transforms.** oxc's transformer strips TypeScript, compiles JSX and lowers syntax from ES2026 down to ES2015 ([oxc transformer docs](https://oxc.rs/docs/guide/usage/transformer.html)).
- **Minifies.** The minifier removes dead code, mangles names and strips whitespace ([oxc minifier docs](https://oxc.rs/docs/guide/usage/minifier.html)). It "makes some assumptions about your code" ([same page](https://oxc.rs/docs/guide/usage/minifier.html)).
- **Reference site files.** In the spike these went through without errors:
  - `film.ts`: 24,221 → 12,180 bytes in 2.8 ms.
  - `shell.ts`: 12,695 → 6,444 bytes in 0.7 ms.
  - `motion-reveal.ts`: 5,753 → 2,716 bytes in 0.6 ms.
  - Bundling `film.ts` + `film-palettes.ts`, which mix `import type` with values, gave 17,081 bytes (7,545 gzip) in 4.5 ms.
- **No type checking.** Neither rolldown nor oxc type-checks. TypeScript 7.0 went GA on 2026-07-08 as a native Go port with no programmatic API yet ([TypeScript blog](https://devblogs.microsoft.com/typescript/?p=5246)). Its npm package ships `tsc` as a Node shim, but the platform package holds a standalone 24 MB Mach-O `tsc` that runs without Node. Checked locally: `@typescript/typescript-darwin-arm64@7.0.2/lib/tsc --version` prints `Version 7.0.2`. This is relevant to parity-inventory question 36.
- **CSS imported from JS is a hard error.** The spike printed: `UNSUPPORTED_FEATURE: Bundling CSS is no longer supported (experimental support has been removed). See https://github.com/rolldown/rolldown/issues/4271`. A CSS RFC ([#8403](https://github.com/rolldown/rolldown/issues/8403)) proposed using lightningcss and is closed without the work landing.
- **Unresolved imports are only warnings.** A missing package (`canvas-confetti`, not installed) produced an `UNRESOLVED_IMPORT` warning and was left as an external bare import, so the build "succeeded" with a broken bundle. The Engine must turn this into an error.

### Resolving `node_modules` installed by Bun

- **How rolldown resolves.** It resolves through `oxc_resolver`, a Rust port of webpack's enhanced-resolve. That resolver implements the ESM and CommonJS algorithms, `exports`/`imports`, tsconfig `paths` and Yarn PnP ([oxc-resolver README](https://github.com/oxc-project/oxc-resolver)). Rolldown's configuration ([resolver_config.rs](https://docs.rs/crate/rolldown_resolver/1.2.12/source/src/resolver_config.rs)):
  - Condition names are `default` plus `browser` or `node` depending on the platform, with `import`/`require` added per import kind.
  - `symlinks` defaults to `true`, so symlinks are resolved to real paths.
- **Bun's two layouts** ([Bun docs, isolated installs](https://bun.com/docs/pm/isolated-installs)):
  - From Bun 1.3.2 (`configVersion = 1`), new workspaces default to the **isolated** linker and single-package projects default to **hoisted**.
  - Isolated installs keep packages in `node_modules/.bun/<pkg>@<ver>/node_modules/<pkg>` and symlink only the direct dependencies into `node_modules/`.
- **Verified with Bun 1.4.2.** The fixture used `nanoid@5` (has a browser condition) and `@floating-ui/dom@1`, whose transitive `@floating-ui/core` and `@floating-ui/utils` are *not* top-level in isolated mode.
  - With `--linker hoisted` and with `--linker isolated`, both bundles came out as `app-DdTG_41z.js` (10,696 bytes), byte-identical.
  - The browser build of nanoid (`crypto.getRandomValues`) was selected.
  - Bundling took 5–16 ms.

### Cost

- Binary size, compile time and crate count are in [Spike setup](#spike-setup): rolldown alone is about +10.4 MB fat-LTO, about 240 crates, and about 76–95 s cold `release`.
- A 9–12 s incremental `release` rebuild means Engine developers should use the `dev` profile day to day. Release builds belong in CI with a target cache.
- MSRV: oxc 0.152 declares `rust-version = 1.96` ([crates.io](https://crates.io/crates/oxc/0.152.0)).

### Licence

- rolldown is MIT and includes MIT code from Rollup and esbuild ([README](https://github.com/rolldown/rolldown#licenses)). oxc and oxc_resolver are MIT ([crates.io](https://crates.io/crates/oxc)).
- The transitive tree for sp-all (from `cargo metadata`) is mostly MIT and/or Apache-2.0. The exceptions:
  - Unicode-3.0 (ICU, through `textwrap` in `oxc_diagnostics`).
  - BSD-3-Clause (`oxc_sourcemap`).
  - BSD-2-Clause (`pnp`).
  - BSL-1.0 (`xxhash-rust`).
  - Zlib and ISC.
  - The MPL-2.0 crates listed under lightningcss.
- The tree has no GPL-only dependency. The GPL appears only inside `OR` choices.

## 2. lightningcss

- **Status.** The Rust crate has been `1.0.0-alpha.N` since alpha.33 in September 2022. The latest is 1.0.0-alpha.72 (2026-07-20), released alongside npm v1.33.0 ([crates.io](https://crates.io/crates/lightningcss/versions), [GitHub releases](https://github.com/parcel-bundler/lightningcss/releases)). The repo is active, with commits on 2026-09-29 and 421 open issues ([GitHub](https://github.com/parcel-bundler/lightningcss)). Releases come every 2–4 months, so fixes on `main` lag. For example, "fix: preserve individual transform properties" ([#1340](https://github.com/parcel-bundler/lightningcss/pull/1340)) merged on 2026-09-27 and is not released yet.
- **Features.** The default features are `bundler`, `nodejs` and `sourcemap`. The Engine needs only `bundler` and `sourcemap`; `browserslist` is optional, for query strings ([Cargo.toml](https://docs.rs/crate/lightningcss/1.0.0-alpha.72/source/Cargo.toml.orig)).
- **Bundling.** `Bundler::new(&provider, source_map, ParserOptions)` and then `.bundle(entry)` inline the `@import` graph.
  - `@import … media`, `supports(…)` and `layer(…)` conditions are kept as wrapping at-rules ([bundling docs](https://lightningcss.dev/bundling.html)).
  - Known limits from the same page: the same file imported under different layer names, nested anonymous layers, and external imports placed after bundled ones.
  - The built-in `FileProvider::resolve` "Assume[s] the specifier is a relative file path" ([source](https://docs.rs/crate/lightningcss/1.0.0-alpha.72/source/src/bundler.rs)). To import CSS from npm packages (`@import "some-pkg/styles.css"`), the Engine implements `SourceProvider::resolve`. The spike does this in about 15 lines on `oxc_resolver` with conditions `style`, `import`, `default`, main fields `style` and `main`, and extension `.css`.
- **Targets and nesting.**
  - Targets are a `Browsers` struct whose versions are encoded as 24-bit `major<<16 | minor<<8 | patch` ([transpilation docs](https://lightningcss.dev/transpilation.html)).
  - Nesting is always parsed and is lowered only when the targets need it. `include` and `exclude` `Features` flags override what the targets imply ([same page](https://lightningcss.dev/transpilation.html)).
  - Also lowered by target: color-mix, relative colours, `lab`/`oklch`, media range syntax, logical properties and more ([README](https://github.com/parcel-bundler/lightningcss#features)).
- **The Reference site's built CSS** (`_slug_.ynAfo1z9.css`, 29,241 bytes) through the spike with targets Chrome 111 / Safari 16.4 / Firefox 128 gave 27,864 bytes in 3–5 ms.
  - **Kept unchanged:** `in oklab` (16), `color-mix` (8), relative `oklab(from …)` (4), `@property` (9), `:has(` (9), `text-wrap:pretty`, `svh`/`dvh`.
  - **Changed in ways the parity inventory says to avoid** (§2.4 "Media and modern CSS", "Tailwind wiring"):
    - The `height:100vh;height:100dvh` fallback pairs collapse to `100dvh`. With **no targets** they collapse too.
    - `::-moz-selection` is dropped.
    - Some rules are merged (`*,:before,:after` with `::backdrop`).
  - **Restored by older targets.** With Safari set to 15.0, the four `100vh` fallbacks and the `-webkit-` prefixes come back (29,114 bytes).
  - **So targets alone decide fallbacks.** lightningcss has no "keep authored fallbacks" switch. Parity needs targets at least as old as the browsers those fallbacks were written for, or a Parity exception.
- **Tailwind uses the same pass.** Tailwind v4 itself post-processes with lightningcss using exactly these targets (Safari 16.4, iOS 16.4, Firefox 128, Chrome 111), with `include: Nesting | MediaQueries` and `exclude: LogicalProperties | DirSelector | LightDark` ([`@tailwindcss/node` optimize.ts](https://github.com/tailwindlabs/tailwindcss/blob/v4.3.3/packages/%40tailwindcss-node/src/optimize.ts)). The Engine can reproduce Tailwind's optimise step natively.
- **Licence: MPL-2.0.** This also covers `cssparser`, `cssparser-color`, `cssparser-macros`, `dtoa-short` and `parcel_selectors`. MPL is file-level copyleft. Linking unmodified MPL crates into an MIT/Apache Engine binary is allowed. When distributing binaries, "You must inform the recipients where they can get the source for the MPLed code" ([MPL 2.0 FAQ, Q8](https://www.mozilla.org/en-US/MPL/2.0/FAQ/)). A third-party-licences file that points at crates.io satisfies this. Patching an MPL file means publishing that file's changes.

## 3. Tailwind CSS v4 without Node

**What Tailwind v4 is made of.** The latest release is v4.3.3 (2026-07-16). npm also keeps a `v3-lts` tag at 3.4.19 ([npm](https://registry.npmjs.org/tailwindcss), [GitHub releases](https://github.com/tailwindlabs/tailwindcss/releases/tag/v4.3.3)). It has three parts:

1. **Scanner ("Oxide"), in Rust.** The `tailwindcss-oxide` crate extracts class candidates ([crates/oxide/Cargo.toml](https://github.com/tailwindlabs/tailwindcss/blob/v4.3.3/crates/oxide/Cargo.toml)). It is exposed to JS only through a napi `cdylib` ([crates/node/Cargo.toml](https://github.com/tailwindlabs/tailwindcss/blob/v4.3.3/crates/node/Cargo.toml)) and is **not on crates.io**: it has path dependencies and the name returns 404 on crates.io. A maintainer said in [discussion #16191](https://github.com/tailwindlabs/tailwindcss/discussions/16191) (wongjn, 2025-02-03): "Rust is used only for some code paths that are slower in JavaScript. Otherwise, the project is still mainly JavaScript-based so it doesn't make sense as a Rust package."
2. **Compiler, in TypeScript.** The npm package `tailwindcss` has zero dependencies and no `node:` imports. `dist/lib.mjs` is 250 KB and exports `compile(css, { base, loadStylesheet, loadModule })`, which returns `{ build(candidates): string }` (checked in the installed 4.3.3 package). It is environment-agnostic; the repo also ships `@tailwindcss/browser` ([packages/](https://github.com/tailwindlabs/tailwindcss/tree/v4.3.3/packages)).
3. **Optimiser: lightningcss,** with the targets listed in §2.

The browser floor of v4 is Chrome 111, Safari 16.4 and Firefox 128 ([compatibility](https://tailwindcss.com/docs/compatibility)). Class detection is plain-text scanning. It skips `.gitignore`d files, `node_modules`, binaries, CSS and lockfiles, and is steered with `@source`, `@source not`, `source(none)` and `@source inline("…")` safelists ([detecting classes](https://tailwindcss.com/docs/detecting-classes-in-source-files)).

### Option A: the official standalone CLI (recommended, opt-in)

- **What it is.** Since v4 the standalone CLI is built with `Bun.build({ compile: … })` for seven targets, with `@tailwindcss/forms`, `typography` and `aspect-ratio` bundled in ([build.ts](https://github.com/tailwindlabs/tailwindcss/blob/v4.3.3/packages/%40tailwindcss-standalone/scripts/build.ts), [package.json](https://github.com/tailwindlabs/tailwindcss/blob/v4.3.3/packages/%40tailwindcss-standalone/package.json)). The v3 CLI was built with `pkg` and needs no Node either ([blog](https://tailwindcss.com/blog/standalone-cli)).
- **Size.** The v4.3.3 assets are 79.8 MB (macos-arm64) to 112.5 MB (windows-x64), and `sha256sums.txt` is published next to them ([release](https://github.com/tailwindlabs/tailwindcss/releases/tag/v4.3.3)). For v3.4.19 the assets are 30–54 MB ([release](https://github.com/tailwindlabs/tailwindcss/releases/tag/v3.4.19)).
- **Measured.** The checksum verified. On the Reference site's 151 class tokens plus a small `@theme`, a run takes **0.19–0.34 s wall time per process**, of which Tailwind reports 27–29 ms. Startup is about 160 ms per process, and `--watch` amortises it in dev. Output with `--minify` was 7,832 bytes.
- **Precedent.** Phoenix's `tailwind` package downloads a version-pinned standalone binary into `_build`, auto-detects the target and allows a `:path` override ([docs](https://tailwind.hexdocs.pm/Tailwind.html)).
- **Pros:** exact official output; supports CSS-first config, `@plugin`, `@config` and first-party plugins; Tailwind upgrades don't need an Engine release; no Engine binary growth.
- **Cons:** an 80–112 MB first-use download, which hurts hermetic or offline CI unless cached; a second process; a Bun runtime inside the binary.

### Option B: Tailwind's compiler in embedded QuickJS (proven, deferred)

- **What was built.** The spike bundled `tailwindcss@4.3.3` into a 287 KB IIFE and ran `compile()` and `build(candidates)` inside `rquickjs` 0.14 (MIT, [crates.io](https://crates.io/crates/rquickjs)). The Engine served `@import "tailwindcss"` through `loadStylesheet`.
- **Measured.** Load plus eval took 15–93 ms (first run slowest) and compile plus build took 19–50 ms. Total was **35–143 ms**, typically about 60 ms, in-process. The binary is 1.35 MB fat-LTO.
- **Fidelity.** The output had the same rules as the standalone CLI's output. The diffs, 29 lines, are quote style and line-wrapping inside preflight and font-family strings, because the CLI embeds a differently formatted `index.css` than the npm package.
- **Pros.** In-process, no download. The Author can `bun add tailwindcss` and the Engine can load *that* version from `node_modules`, which fits ADR 0003's "npm packages from node_modules" model.
- **Cons.**
  - It embeds a JS runtime, the exact cost ADR 0002 rejected for templates ("slower and heavier, and it reintroduces a JS runtime at build time").
  - JS plugins and `@config` files need module loading through `loadModule` and fail if they touch Node APIs.
  - The Engine must provide the candidate scanner itself, by vendoring MIT Oxide code or writing its own.
  - QuickJS speed on large candidate sets was not measured.

### Option C: a Rust reimplementation (not now)

- **encre-css 0.21.1** (MIT, 2026-09-09) is the only maintained one ([crates.io](https://crates.io/crates/encre-css), [repo](https://gitlab.com/encre-org/encre-css)). It says "Since v0.16.0, this library only supports TailwindCSS v4.0" and "This is **not** a project made by the TailwindCSS team". It is configured by TOML or Rust (`Config::from_file("encre-css.toml")`), and its source has no `@theme`, `@apply`, `@utility` or `@variant` handling.
  - On the Reference site's tokens it generated CSS for 71 of the 75 that Tailwind v4.3.3 recognises. The four misses were the custom theme colours, which in encre-css need TOML config.
  - Output byte-equivalence was not tested.
- **The others are abandoned:** `tailwind-css` (0.13.0, 2023-11), `tailwind-rs` (0.2.0, 2022), `railwind` (0.1.5, 2023-02) and `rswind` (0.0.1-alpha.1, 2024-06) ([crates.io](https://crates.io/crates/tailwind-css)).
- **Exact parity is a large project.** An independent OCaml port reached byte-for-byte parity with v4 by differential-testing against the reference binary and passing "905 upstream tests". The hard parts were rounding and OKLCH gamut mapping ([Gazagnaire, 2026-04-21](https://gazagnaire.org/blog/2026-04-21-tailwind-ocaml.html)).

### What the Engine has to do to support Tailwind (Option A)

1. **Opt-in.** Add a site config key such as `[css] tailwind = "4.3.3"`, or detect `@import "tailwindcss"` in the CSS entry. Building fails with a clear diagnostic when Tailwind is detected but no version is pinned.
2. **Acquire the binary.**
   - Map OS, architecture and libc to the asset name (`tailwindcss-{linux,macos}-{x64,arm64}[-musl]`, `-windows-x64.exe`).
   - Download from the GitHub release and verify SHA-256. Prefer checksums embedded in the Engine for known versions; otherwise use the release's `sha256sums.txt`.
   - Cache it per user under the version, and support an offline flag and a path override such as `YUANME_TAILWIND_BIN`.
   - CI caches that directory.
3. **Give it the classes the Engine generates.** Markdown and Org renderers, code-block wrappers and runtime-only JS classes (the parity inventory notes CopyBtn's classes appear only in JS) are not in source files Tailwind would scan. Order the build:
   1. Render HTML in memory.
   2. Extract class tokens from the HTML and the JS bundles.
   3. Write them to a temporary file referenced by an `@source` the Engine injects. Alternatively, use `source(none)` and list sources explicitly so `dist/` and caches are never scanned.
   4. Run Tailwind.
   5. Hash the CSS.
   6. Write the HTML with the hashed name.
4. **One CSS pass.** Run Tailwind without `--minify`/`--optimize` and send its output through the Engine's lightningcss (same targets, minification, hashing), so Tailwind and non-Tailwind Sites share one optimiser.
5. **Dev loop.** Keep a `--watch` process alive, or respawn per rebuild (about 0.2 s). Map Tailwind's stderr into Engine diagnostics.
6. **Plugins.** First-party plugins work out of the box. For third-party `@plugin` packages, Authors `bun add` them. Whether the standalone CLI resolves them from a Bun-isolated `node_modules` was **not tested**.

## Implications for ADR 0003

- **Supported:** "embed Rust-native front-end tooling … rolldown/oxc and lightningcss" works as written. Both build on stable Rust and resolve Bun's layouts.
- **Needs an amendment:** ADR 0003 rejects "Shelling out to Bun or esbuild … adds a second required runtime". The Tailwind standalone CLI *is* a Bun-compiled executable. Recommending it is defensible only because it is optional (only Tailwind Sites), managed by the Engine (the Author installs nothing), and pinned. ADR 0003 should say so explicitly. Option B avoids the second process but runs into ADR 0002's argument against an embedded JS engine.
- **Maintenance cost to record:** pin `=` versions, keep rolldown and lightningcss each behind one Engine module that never exposes their types, and expect to fix the Engine's own breakage on each bump. The Engine can't rely on upstream for Rust-only issues.

## Open questions

1. Accept a managed Bun-built sidecar for Tailwind (Option A) as an amendment to ADR 0003, or require in-process Tailwind (Option B), with an amendment to ADR 0002?
2. Default browser targets: Tailwind's (Chrome 111 / Safari 16.4 / Firefox 128), or older ones that keep the Reference site's authored fallbacks (`100vh` before `100dvh`, `::-moz-selection`)? Or record those as Parity exceptions? Should Authors set targets through a browserslist query (the optional `browserslist` feature) or explicit versions?
3. Reference site (parity inventory Q23): migrate from Tailwind v3 to v4 (its v3 classes mostly worked under v4), run the v3 standalone CLI, or drop Tailwind for about 90 hand-ported utilities, making the Reference site not need the sidecar at all?
4. JS that imports CSS: forbid it (current rolldown behaviour), or write a small rolldown Rust plugin (`Bundler::with_plugins`) that hands those imports to lightningcss?
5. How often to bump rolldown/oxc: every release, or quarterly? Who owns fixing breakage, given the rolldown team closes Rust-only issues?
6. Type gate for client TS: run the TypeScript 7 native `tsc` (standalone 24 MB Go binary, no Node) as an optional managed sidecar like Tailwind, or don't type-check?
7. Release-binary budget: is +15 MB acceptable, or should rolldown sit behind a Cargo feature for a "no-JS" Engine build?
8. Do the standalone CLI's `@plugin` imports resolve third-party plugins from a Bun-isolated `node_modules`? This is untested.
