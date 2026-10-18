# Parity exceptions

This file lists every deliberate difference between the Reference site (eyuan.me built with yuanme) and its Astro predecessor. It also sets the rules of the parity test that checks everything else. Each exception has an id. Every rule in the parity harness that tolerates a difference names one of these ids, and a rule whose id is missing from this file fails the test.

Decided in [What are the Reference site's Parity exceptions?](https://github.com/yuann3/yuanme/issues/23), which worked through section 3 of the [parity inventory](research/astro-parity-inventory.md).

## The parity test

### Parity baseline
- Astro's `dist/`, built from a pinned eyuan.me commit. Never the live site: Cloudflare rewrites its HTML ([#13](https://github.com/yuann3/yuanme/issues/13)).
- The baseline is frozen when the port starts. A content edit made during the port lands in both repositories, or waits until after cutover.

### What is compared
1. **URLs.** The set of output paths and every internal href, mapped through the URL exceptions below, must be equal.
2. **Normalized DOM.** Every Page.
3. **Pixels.** Every Page, at each viewport and colour scheme.
4. **Behaviour.** The prototype's `feel.ts` checks from [#26](https://github.com/yuann3/yuanme/issues/26), in Chromium, Firefox and WebKit.

### Normalization
Both sides are parsed with html5ever, then:
- **Removed:** comments, text nodes that are only whitespace, and the scoping attributes (`data-astro-cid-*`, `data-y-*`).
- **Whitespace** in text is collapsed, except inside `pre`, `textarea`, `script` and `style`.
- **Attributes and `class` tokens** are sorted. `="true"` and a bare boolean attribute are equal.
- **Asset URLs:** a hashed URL is replaced by the source file it was built from.
- **The head** is compared as the ordered list of stylesheets, plus the unordered set of everything else, keyed by tag, `name`/`property`/`rel` and value.
- **Highlighted code:** `<pre>` blocks are compared by text only. Their look is covered by the pixel diff (`class-highlighting`).

Escaping style, kept comments, attribute order and whitespace all decode to the same DOM, so they need no exception.

### Pixels
- **Matrix:** every Page × widths 390, 900 and 1280 px (the ≤ 767, 768–1099 and ≥ 1100 bands) × light and dark.
- **Engines:** Chromium on every PR; Firefox and WebKit at releases. Both sides run in the same pinned Playwright browser.
- **Motion:** screenshots are taken with `prefers-reduced-motion: reduce`, once `document.getAnimations()` has settled. With that setting the Film doesn't start, so its deterministic fallback CSS is what's compared. The Film itself is covered by the behaviour checks.
- **Fonts:** the harness serves the baseline's Google Fonts requests from the same woff2 files the Reference site self-hosts.
- **Threshold:** pixelmatch with a per-pixel threshold of 0.1. A screenshot fails if more than 0.05 % of its pixels differ. Re-encoded photos use a per-pixel threshold of 0.25: the four About photos (`about-image-bytes`) and the hero images (`hero-images`).

## Exceptions

### URLs and outputs

| Id | Astro | yuanme | Decided in |
|---|---|---|---|
| `assets-dir` | Hashed files under `/_astro/` | Under `/assets/`, named by blake3 content hash | [#10](https://github.com/yuann3/yuanme/issues/10) |
| `trailing-slash-hrefs` | Some nav and project links have no trailing slash and cost a 301 | Every internal href is `/x/` | #10 |
| `apex-origin` | `https://www.eyuan.me` in canonical, sitemap and RSS | `https://eyuan.me` | #10 |
| `immutable-assets` | `max-age=14400` from Cloudflare's default | `/assets/` gets `public, max-age=31536000, immutable` through the host adapter | #10 |
| `robots-txt` | No `robots.txt` | `robots.txt` with the sitemap URL | #23 |
| `unreferenced-files` | Atkinson fonts, full-size About AVIF/WebP and the copied originals are published | Not published. `ica.pdf` is kept | #23 |
| `llms-txt` | "Archive by year" label over a flat list | An ordinary Route Template with the same content and a correct label | #23 |

No tag URL changes: the Slugs of `C`, `Programming`, `Life` and `Draft` equal today's directory names (ADR 0005).

### Content

| Id | Astro | yuanme | Decided in |
|---|---|---|---|
| `visibility-rules` | Drafts are listed and in RSS; hidden Posts keep their tags | Drafts are absent; Unlisted Entries are in no list, Taxonomy, feed or sitemap | [#18](https://github.com/yuann3/yuanme/issues/18) |
| `collection-order` | First-seen order, `/tags` ties as `life, draft, c, programming` | Collection `sort` rules; ties by Slug: `c, draft, life, programming` | #18, [#19](https://github.com/yuann3/yuanme/issues/19) |
| `rss` | Includes Drafts and hidden Posts, unsorted | Published, listed Entries, newest first | #19, #23 |
| `heading-ids` | Duplicate headings record `ruskey-` but emit `ruskey` | github-slugger numbering (`ruskey`, `ruskey-1`) | [#2](https://github.com/yuann3/yuanme/issues/2), #23 |
| `reading-time` | Counts glued blocks: 8 and 2 min | Every visible word at 200 wpm, rounded up, at least 1: 9 and 2 min | #23 |
| `class-highlighting` | Shiki inline-style dual-theme spans | Class-based spans from giallo with pinned Shiki grammars and themes. A pin bump that changes colours is accepted through the pixel diff and noted here | [#5](https://github.com/yuann3/yuanme/issues/5), #23 |

### Markup and styles

| Id | Astro | yuanme | Decided in |
|---|---|---|---|
| `scoped-styles` | Per-Page scoped `<style>` blocks, `/about/`-only CSS, CSS for Components that render nothing | One Site-wide Sidecar stylesheet with `data-y-*` scoping | ADR 0006, [#20](https://github.com/yuann3/yuanme/issues/20) |
| `tailwind-v4` | Tailwind 3.4.19 with duplicated preflight | Tailwind v4 through the Managed tool, one copy of preflight. Any pixel difference from v4's preflight is fixed in the port, not excepted | #23 |
| `css-targets` | `100vh` fallbacks before `dvh`/`svh`, `::-moz-selection` | Dropped by the default targets (Chrome 111, Firefox 128, Safari 16.4) | #20, #23 |
| `self-hosted-fonts` | Google Fonts hotlink with preconnects; Instrument Serif loaded unused | Self-hosted woff2: Geist 400/500/600, Geist Mono 400/500, IBM Plex Mono 400/500, `font-display: swap`, Geist 400 and 500 preloaded | #23 |
| `head-metas` | `generator` = `Astro v5.16.5`, `astro-view-transitions-*` metas | `generator` = `yuanme <version>`; no router metas; `<link rel="alternate">` for RSS | #23 |
| `og-image` | `og:image` and `twitter:image` point at missing files | Omitted until a Page has a real image; `twitter:card` is `summary` | #23 |
| `empty-idle-main` | The idle `<main>` holds two spaces, so `:empty` never matches | It is empty, so `.panel-content:empty` hides it as written | #23 |
| `about-img-src` | `<img src>` is a full-size re-encoded JPEG | The largest srcset JPEG | #23 |
| `about-image-bytes` | sharp's encodes | The Engine's encoders. Near-identical pixels, compared at the looser threshold | [#8](https://github.com/yuann3/yuanme/issues/8), #23 |
| `image-formats` | About photos have AVIF and WebP `<source>`s | One AVIF `<source>` plus the JPEG `<img srcset>`. WebP is not generated, because every target browser decodes AVIF | [#28](https://github.com/yuann3/yuanme/issues/28) |
| `hero-images` | Post and project hero images (`rustc.PNG` 1.6 MB, `plane.png`, `Ylib.png`) are plain `<img>` from `public/` | Processed by `image()`: AVIF and JPEG srcsets, `width`/`height`, `fetchpriority="high"`, and a preload. Compared at the looser per-pixel threshold | #28 |

### Client runtime

| Id | Astro | yuanme | Decided in |
|---|---|---|---|
| `router-events` | `astro:before-swap`, `astro:after-swap`, `astro:page-load` | `yuanme:*`, with no aliases | [#26](https://github.com/yuann3/yuanme/issues/26) |
| `router-no-view-transition` | The swap runs inside a neutralised `startViewTransition` | No wrapper | #26 |
| `route-announcer` | A new announcer node on every navigation | One node, reused | #26 |
| `mount-scripts` | PhotoPile and the copy button run once per document and bind on page-load | Sidecar scripts that export `mount(ctx)` | #20, ADR 0013 |
| `photopile-lazy` | PhotoPile loads with the Page | Its `mount` loads the rest with `import()` when the pile scrolls into view, which keeps `/about/` under the 13 KB br JS budget | #23 |
| `prefetch` | `<link rel=prefetch>` on hover | The Router's in-memory prefetch map, which also works in Safari | #20 |
| `single-initial-script` | Router, ClientRouter and Shell chunks with an import waterfall | The Router is bundled into the Page's Script, so a Page with one Script loads one JS file, within ≤ 6 render-path requests | #20, #23 |
| `panel-scroll` | The panel scroll resets to the top on back and forward | Restored from the history entry with `updateCurrentEntry` | [#12](https://github.com/yuann3/yuanme/issues/12), #23 |
| `film-motion-change` | If reduced motion is on at load, the Film never starts | It starts when reduced motion is turned off | #23 |

## Kept on purpose

These match Astro, but a reader might expect them to differ.

- **A 404 response is swapped into the panel**, as Astro does.
- **Back and forward record a CLS of 0.51–0.56** in Chromium, from the panel morph changing layout without recent input. Clicked links record none. The CLS Gate measures cold loads. Field CLS is checked once after cutover, and fixing it would mean redesigning the morph.

## Not an exception

**Org-only constructs** have no Astro counterpart: footnotes, description lists, captioned figures, example and verse blocks, and TODO keywords. They get new `.prose-panel` rules in the existing tokens, checked against snapshots of the Org corpus.
