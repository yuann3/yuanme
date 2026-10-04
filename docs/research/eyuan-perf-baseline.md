# eyuan.me real-world performance baseline

Ticket: [Measure eyuan.me's real-world performance baseline](https://github.com/yuann3/yuanme/issues/13). Measured on 2026-10-04 against the live Reference site at `https://eyuan.me` (Astro 5.16.5 build, deployed to Netlify, proxied by Cloudflare). The build-side numbers are in [`astro-parity-inventory.md`](astro-parity-inventory.md) §1 and are not repeated here.

This is the runtime baseline that [What are yuanme's performance budgets?](https://github.com/yuann3/yuanme/issues/15) sets numbers against.

## Method

| Source | What it gives | Where from |
|---|---|---|
| Lighthouse 13.5.0 (`bunx lighthouse`, headless Brave/Chromium 154), performance category only, 3 runs per cell, median reported | Lab FCP, LCP, CLS, TBT, Speed Index, TTFB, requests, bytes | Singapore (Cloudflare POP `SIN`), home fibre |
| Mobile preset: simulated throttling, 150 ms RTT, 1.6 Mbps down, 4× CPU slowdown | "Slow 4G, mid-range phone" | |
| Desktop preset: simulated, 40 ms RTT, 10 Mbps, no CPU slowdown | | |
| check-host.net HTTP check, 40 nodes, 1 sample each | Full request time (DNS + TCP + TLS + response) per region | 40 probe nodes on 5 continents |
| `curl` with `accept-encoding: br`, 5 samples | TTFB, compressed HTML size, headers, redirect chains | Singapore |

**Gaps, stated plainly:**
- **CrUX field data was not obtained.** The PageSpeed Insights API returned `429 Quota exceeded … Queries per day` for anonymous use, and the CrUX API needs a key. A low-traffic personal site may have no CrUX record anyway. To fill it, rerun `GET https://chromeuxreport.googleapis.com/v1/records:queryRecord?key=…` with `{"origin":"https://eyuan.me"}` once an API key exists.
- **INP is not measured.** It is a field-only metric. TBT, its lab proxy, is 0 ms on every page and every run, so the main thread is idle on load.
- **Lab Core Web Vitals come from one region.** Lighthouse's simulated throttling models the network from observed RTTs, so the region mostly moves TTFB. The 40-node probe covers the other regions for server and network time only. WebPageTest needs an account and API key now and was not used.

## Results: lab, Singapore (median of 3)

| Page | Form | Score | FCP ms | LCP ms | CLS | TBT ms | SI ms | TTFB ms | Requests | Transferred KB |
|---|---|---|---|---|---|---|---|---|---|---|
| `/` | mobile | 91 | 2,772 | 2,772 | 0 | 0 | 2,772 | 49 | 14 | 107.6 |
| `/` | desktop | 99 | 761 | 761 | 0 | 0 | 761 | 36 | 15 | 107.6 |
| `/blog/extending-c-stdlib/` | mobile | **70** | 2,947 | **10,940** | 0 | 0 | 2,947 | 269 | 16 | **1,716.6** |
| `/blog/extending-c-stdlib/` | desktop | 90 | 804 | 2,056 | 0 | 0 | 804 | 45 | 17 | 1,716.7 |
| `/projects/` | mobile | 91 | 2,774 | 2,789 | 0 | 0 | 2,774 | 258 | 14 | 109.1 |
| `/projects/` | desktop | 99 | 765 | 809 | 0 | 0 | 820 | 260 | 15 | 109.1 |
| `/about/` | mobile | 89 | 2,826 | 2,862 | 0 | 0 | 3,069 | 265 | 20 | 188.0 |
| `/about/` | desktop | 99 | 720 | 762 | 0 | 0 | 720 | 40 | 21 | 150.5 |

Run-to-run spread (min / max) is tight for paint metrics, at most ±10%: for example, Post mobile LCP was 10,753–11,250 ms and home mobile FCP 2,763–2,954 ms. TTFB is not tight; see "Edge caching" below.

### Transferred bytes by type (mobile, KB)

| Page | Image | Font | Script | CSS | HTML | Other |
|---|---|---|---|---|---|---|
| `/` | 0 | 61.1 | 28.7 | 11.9 | 4.4 | 1.5 |
| Post | 1,593.3 | 66.9 | 28.7 | 11.9 | 14.4 | 1.5 |
| `/projects/` | 0 | 61.1 | 28.7 | 11.9 | 5.9 | 1.5 |
| `/about/` | 72.7 | 61.1 | 32.2 | 13.9 | 6.4 | 1.5 |

The 28.7 KB of script splits into **18.0 KB first-party** (Shell 11.2, router 4.5, ClientRouter 2.2) and **11.9 KB injected by Cloudflare** (Web Analytics `beacon.min.js` 10.3, `email-decode.min.js` 1.1, `/cdn-cgi/rum` beacon 0.5). The fonts are all third-party from Google Fonts: Geist 29.3 KB, Geist Mono 23.1 KB, IBM Plex Mono 10.1 KB, plus a Geist Mono subset (5.9 KB) on the Post. The 1.5 KB Google Fonts stylesheet is counted under CSS.

## Results: request time by region (40 nodes, 1 sample)

Full HTTP request time for the HTML, including DNS, TCP and TLS. Quartiles are over the 39 nodes that answered.

| Page | p25 | median | p75 | Notable slow nodes |
|---|---|---|---|---|
| `/` | 82 ms | 170 ms | 307 ms | Sofia 1.6 s, Jakarta 3.7 s |
| Post | 105 ms | 167 ms | 290 ms | Hong Kong 1.0 s, Sofia 1.5 s |
| `/projects/` | 168 ms | 296 ms | 418 ms | Hong Kong 1.4 s, Jakarta 3.4 s |
| `/about/` | 153 ms | 216 ms | 308 ms | Hong Kong 0.9 s, Jakarta 3.4 s |

Bucharest timed out (6 s) on every page; this is likely the probe node rather than the site. Representative nodes for the budgets ticket: Zurich 37–174 ms, New York 130–169 ms, Los Angeles 195–261 ms, Sydney 26–265 ms, Mumbai 216–506 ms, Hong Kong 921–1,370 ms.

## What the numbers say

1. **The Post's LCP is a 1.6 MB passthrough PNG.** The LCP element is `article.prose-panel > img` = `/rustc.PNG` (1,631,529 B, served from `public/` with no derivatives, no `srcset`, no `fetchpriority`). It accounts for 93% of the page's bytes and takes mobile LCP from about 2.9 s to 10.9 s. The inventory lists three such PNGs (3.1 MB in total). The Engine's image pipeline has to cover images referenced from Post bodies, not only `<Picture>` call sites, or this regression gets ported at full parity.
2. **Mobile FCP of about 2.8 s on 4–15 KB pages is all render-blocking.** Lighthouse estimates 1.1–1.3 s of savings. The blockers, in order: the cross-origin Google Fonts stylesheet (762 ms on the Post), `_slug_.css` (300–632 ms), `about.css`, and Cloudflare's injected `email-decode.min.js`. On `/` the LCP element is text (`div.intro > p.sub`) with 1.6 s of element render delay, which is fonts plus CSS. Self-hosted fonts and inlined critical CSS are where the Engine can beat the baseline without changing a pixel.
3. **CLS is 0 and TBT is 0 everywhere.** The Film and the router cost nothing measurable on load. The budget can set these as hard zero or near-zero lines.
4. **Cloudflare changes the shipped HTML.** Email obfuscation rewrites `mailto:` links to `/cdn-cgi/l/email-protection#…` and injects a render-blocking script. Web Analytics injects a 10 KB beacon. None of this is in `dist/`. Parity tests must compare against the build output (or against a Cloudflare-off origin), and the budget's "JS bytes before the Film" must say whether it counts host-injected scripts.
5. **Two CDNs are stacked, and the HTML is not cached at the outer one.** DNS is on Cloudflare (proxied, `cf-cache-status: DYNAMIC` on HTML) in front of Netlify Edge (`cache-status: "Netlify Edge"; hit` or `fwd=miss`). Netlify misses show up as TTFB spikes of 245–536 ms against 24–62 ms on hits, from the same machine minutes apart. This is a hosting finding for [How does a Site deploy portably to any host?](https://github.com/yuann3/yuanme/issues/25), not an Engine one.
6. **Hashed assets are not cached as immutable.** `/_astro/*.css` returns `cache-control: public, max-age=14400, must-revalidate` (4 h). HTML returns `max-age=0, must-revalidate`. Per [What must the Engine emit to be portable and fast on any static host?](https://github.com/yuann3/yuanme/issues/10), hashed `/assets/` should be `immutable`.
7. **Nav links cost a redirect.** The header links are `/about`, `/projects` and `/blog`, without the trailing slash, and each one 301s to `/x/` on a full load. The RSS `<link>`s point at `https://www.eyuan.me/blog/…/`, which 301s from `www` to the apex. The Engine's link-as-`/x/` rule removes the first. The second is a Site config value (canonical origin).
8. **The host compresses at a lower quality than possible.** Live brotli is 3,270 B for `/` and 12,300 B for the Post, against 2,578 B and 9,458 B at brotli q11 locally, so the host's output is 27–30% larger. Pre-compressed `.br` output would close this on hosts that serve it.

## Raw data

The Lighthouse JSON (24 reports, about 10 MB) and the check-host responses were kept out of the repo. To reproduce, for each page × `{mobile, --preset=desktop}` × 3 runs:

```sh
CHROME_PATH=<chromium binary> bunx --bun lighthouse "https://eyuan.me<page>" [--preset=desktop] \
  --only-categories=performance --output=json --output-path=<page>-<form>-<n>.json --quiet \
  --chrome-flags="--headless=new --no-first-run --disable-extensions"
```

The multi-region check is `GET https://check-host.net/check-http?host=https://eyuan.me/<page>&max_nodes=40`, then `GET /check-result/<request_id>`.
