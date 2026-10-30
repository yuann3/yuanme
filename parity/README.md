# The parity test

Checks a Site against the Parity baseline, under the rules and Parity exceptions in [`docs/parity-exceptions.md`](../docs/parity-exceptions.md) (#23). It is test tooling for the Reference site; ADR 0003 covers the Engine, which still never runs Node.

| Check | Where | What |
|---|---|---|
| URLs | `cargo run -p parity` | The set of output paths and of internal link targets, and every target the baseline resolves still resolving |
| Normalized DOM | `cargo run -p parity` | Every Page both sides have, parsed with html5ever and normalized as the rules say |
| Pixels | `browser/pixels.ts` | Every Page × 390, 900, 1280 px × light, dark, with reduced motion; the full page and each screen of the panel |
| Behaviour | `browser/feel.ts` | The prototype's `feel.ts` checks (#26), compared side by side |

## The baseline

`baseline.lock` pins the release `parity-baseline-<rev>`: a `.tar.zst` holding Astro's `dist/` as `site/`, `remote/` with everything its Pages load from other origins (the Google Fonts stylesheets and font files, and the images Project pages embed from GitHub), and `BASELINE.json`. The harness answers every other origin from `remote/` and refuses anything else, so no check reaches the network.

- `baseline/build.sh REV OUT` builds it on linux/amd64 with Node 22, checks it against the live site, and packs it deterministically. Its header has the Docker command for other machines.
- `fetch.sh DIR` downloads the pinned release, checks its sha256 and unpacks it. `PARITY_BASELINE=FILE` unpacks a local tarball instead.

## Running it

```sh
parity/fetch.sh target/baseline
cargo run -p parity -- build ../eyuan.me target/site --yuanme target/debug/yuanme
cargo run -p parity -- check target/baseline target/site
(cd parity/browser && npm ci && npx playwright install chromium)
node parity/browser/parity.ts check target/baseline target/site --engines chromium --out target/parity-out
```

`build` runs `yuanme build --json --out DIR` and reads its `build-finished` event; the build report is in the project's `.yuanme/report.json`. Failing screenshots are written to `--out` as the baseline's, the candidate's and a diff. `--only pixels` or `--only feel` runs one of the two browser checks.

`self-test BASELINE`, in both tools, compares the baseline with itself (two renders, in the browser) and then seeds changes into a copy: those it must catch, and those that decode to the same DOM or that an exception covers, which it must let through. CI runs it (`.github/workflows/parity.yml`): Chromium on every push, all three engines when called with `engines`.

## Tolerating a difference

Each rule that lets a difference through names its exception id, and a rule whose id is missing from `docs/parity-exceptions.md` fails the test before it compares anything. A rule normalizes both sides alike, so it never requires a difference and the baseline compared with itself still passes. The rules so far:

- URLs and DOM (`src/lib.rs`): `apex-origin`, `trailing-slash-hrefs`, `assets-dir`, `robots-txt`, `unreferenced-files`, `head-metas`, `scoped-styles`, `single-initial-script`, `image-formats`, `self-hosted-fonts`, `immutable-assets`, `class-highlighting`, `og-image`, `reading-time`, `collection-order`, `not-found-canonical`, `dead-links`.
- Pixels (`browser/pixels.ts`): the 0.25 threshold inside raster images, `about-image-bytes` and `hero-images`; `collection-order`, which sorts the tags index's rows on both sides.
- Behaviour (`browser/feel.ts`): `route-announcer`, `panel-scroll`, `single-initial-script`, and `router-events` for the probe's event names.

The other exceptions get their rules with the port that makes the difference (#75, #76).
