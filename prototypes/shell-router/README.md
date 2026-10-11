# Prototype #26: the Shell swap router

**Throwaway.** This answers [#26](https://github.com/yuann3/yuanme/issues/26): is a small Navigation API router good enough to become the Engine's built-in router? It replaces Astro's `<ClientRouter />` on the Reference site (eyuan.me at `6567e01`) and compares the two side by side.

**Verdict: yes.** In Firefox 157 (Playwright's build, on Linux), Chromium 156 and WebKit 27.2 the prototype matches Astro on every feel check. It keeps one WebGL context for the Film across a 12-step tour and paints soft navigations sooner. The router is 1.6 KB br, against 3.6 KB br for Astro's router alone.

## What is here

| Path | What |
|---|---|
| `shell-router.patch` | The whole change to eyuan.me (`git apply` on `6567e01`) |
| `src/router.ts` | The router: Navigation API driver, swap core, script runner, prefetch, announcer, dev hook |
| `src/router-history.ts` | An optional History API driver, built only into the `hist` variant to measure its cost |
| `build.sh` | Clones eyuan.me at `6567e01` and builds `astro-dist`, `proto-dist` and `proto-hist-dist` with Bun |
| `serve.ts` | Serves the builds like Netlify: `:4321` Astro, `:4322` prototype, `:4323` prototype with the Navigation API hidden (full page loads), `:4324` prototype plus History API driver with the Navigation API hidden |
| `tools/bench/` | Playwright benches: `nav.ts` (timing), `feel.ts` (behaviour), `devswap.ts` and `devsplit.ts` (dev swap), `sizes.ts`, Firefox probes |
| `results/` | Raw JSON and logs for every number below |

Run: `./build.sh && bun serve.ts`, then from `tools/` (after `bun install`): `bun bench/feel.ts chromium`. For Firefox on macOS 27, which can't launch Firefox here, run the bench in `mcr.microsoft.com/playwright:v1.64.0-noble` under `xvfb-run`, with `HOST=host.docker.internal`.

## Feel: identical to Astro in all three engines

`results/feel-{firefox,chromium,webkit}.json`. Variants: `astro`, `proto`, `full` (prototype with the Navigation API hidden), `hist`.

| Check | astro | proto | hist | full |
|---|---|---|---|---|
| Film: same canvas, WebGL contexts over a 12-step tour (push, back, forward, Esc, same section) | yes, 1 | yes, 1 | yes, 1 | no, 13 |
| `aria-current`, titles, copy buttons (29/29), PhotoPile init on every step | ok | ok | ok | ok |
| Mobile window scroll: push → 0, back → restored, forward → 0 | ok | ok | ok | ok |
| Desktop panel scroll restored on back | no | no | no | yes in Firefox |
| Reduced motion: no morph animation, reveal base 0 ms | ok | ok | ok | n/a |
| Focus after a keyboard-activated rail link | link | link | link | body |
| Two clicks 5 ms apart land on the second page, one panel | ok | ok | ok | ok |
| `.rss.xml` link is a full load | ok | ok | ok | ok |
| Redirect `/about` → 301 `/about/`: ends on `/about/`, panel swapped, one new entry, back works (`results/redirect.log`) | ok | ok, but `navigateerror` fires | n/a | n/a |
| Route announcers in the DOM after the tour | **12** (leak) | 1 | 1 | 0 |

**Redirect adoption aborts the navigation.** As [#12](https://github.com/yuann3/yuanme/issues/12) predicted, adopting the final URL with `replaceState` inside the handler aborts the intercepted navigation, so `navigateerror` fires in all three engines and `navigation.transition.finished` rejects. The swap, scripts and `page-load` still run, so a visitor sees nothing wrong. The Engine's router should redirect with `precommitHandler` + `redirect()` where that option exists (Chromium 141, Firefox 147; only when `e.cancelable && e.navigationType !== 'traverse'`) and keep `replaceState` for Safari. The Engine links to `/x/` directly, so redirects only come from hand-written links.

**Bug found and fixed in Firefox 157.** After the body swap, `NavigateEvent.scroll()` on a push left the window where it was (47 → 47 on mobile), while Chromium and WebKit scroll to the top (`results/ffsite.log`). On a plain page all three engines reset a push, and they differ on traverse: WebKit restores inside `scroll()`, Firefox has restored before the handler runs, and headless Chromium went to 0 (`results/ffscroll.log`). The router now scrolls push and replace itself (to the fragment, or to the top) and calls `scroll()` only on traverse. With that, Firefox matches Astro.

## Timing: soft navigation against a full page load

Click to first paint of the new panel, in ms, median / p95. "full" is the same Site doing full page loads, so its paint includes the Film's WebGL restart.

**Chromium 156 headed, real GPU (Apple M4 Max, ANGLE Metal)**, n = 7–8 (`results/nav-chromium-headed.json`):

| Step | astro | proto | hist | full | full: Film GL init |
|---|---|---|---|---|---|
| idle → expanded | 41 / 49 | 29 / 36 | 28 / 34 | 51 / 69 | 8 / 11 |
| expanded → expanded | 31 / 56 | 21 / 31 | 21 / 24 | 33 / 105 | 8 / 68 |
| expanded → idle | 30 / 65 | 17 / 156 | 17 / 18 | 30 / (stall) | 10 / 193 |
| back | 31 / 40 | 14 / 17 | 17 / 21 | 34 / 39 | 10 / 11 |
| forward | 27 / 39 | 17 / 19 | 14 / 27 | 35 / 43 | 11 / 13 |

One proto and one hist run errored (n = 7). The full-load "expanded → idle" p95 includes a 222 s stall, an outlier from the headed window. WebKit 27.2 and headless Chromium (SwiftShader) show the same ordering (`results/results-nav-*-headless.log`): proto ≤ astro, and in Chromium proto is about half of full.

**Hybrid cross-document View Transition with a Film handoff:** not needed. On a real GPU, WebGL init is only 8–11 ms, but a full load still loses the Film's state, paints later than a soft swap, and does nothing in Firefox, which has no cross-document VT. Keep `@view-transition` as the separate zero-JS mode for Sites without a persistent canvas.

## Dev soft swap: the browser half of save-to-paint

The dev server's push, simulated: hand the page's edited HTML to the router's dev hook (`yuanme:patch` → parse → quiet Shell swap that keeps the Film and panel scroll), against `location.reload()`. Edit to paint, ms, median / p95, n = 30 (`results/devswap-*.json`):

| Engine | Page | soft swap | reload |
|---|---|---|---|
| Chromium 156 headed | /blog/ | 17 / 19 | 33 / 41 |
| Chromium 156 headed | long post (29 code blocks) | 26 / 31 | 56 / 62 |
| WebKit 27.2 headless | /blog/ | 20 / 31 | 64 / 149 |
| WebKit 27.2 headless | long post | 63 / 73 | 162 / 188 |

The soft swap always keeps the Film's canvas and the panel scroll position. A reload restarts the Film and loses the scroll.

Parsing isn't where the time goes (`results/devsplit.log`). Both engines parse the whole 139 KB page in 1–3 ms and replace the panel in under 2 ms. Style, layout and paint of the 113 KB panel take 38–44 ms, so sending only a fragment would not help. If the long post breaks the 100 ms budget, the fix is a keyed, block-level morph in dev only, so that unchanged code blocks keep their layout.

## Bytes

Brotli q11, minified with esbuild (`results/sizes.md`).

| Item | br (B) |
|---|---|
| `router.ts` (Navigation API driver + swap core + scripts + prefetch + announcer + dev hook) | 1,608 |
| `router.ts` without the dev hook | 1,562 |
| `router.ts` + History API driver | 1,879 (+271) |
| Shell glue (morph, nav sync, panel swap, `before-swap`) | 1,541 |
| Astro's router chunk alone | 3,644 |

First-party JS per Page, as served:

| Page | astro | proto |
|---|---|---|
| `/` and `/blog/` | 15,050 | 11,314 |
| a Post | 15,462 | 11,726 |
| `/about/` (PhotoPile) | 17,801 | **14,065** |

`/about/` is still over the 13 KB br JS budget from [#15](https://github.com/yuann3/yuanme/issues/15). The router isn't the cause: the Film, palettes and reveal account for 8.8 KB br, and PhotoPile for 2.75 KB br.

## Fallback for browsers without the Navigation API

Every current engine has the Navigation API (Chrome 105, Firefox 147, Safari 26.2). Hiding it gives full page loads: correct on every check except the Film, which restarts on each load. The History API driver keeps the Film too, but costs 271 B br and a second code path to test. **Recommendation: full page loads**, with no History API driver.

## Not covered

- Firefox 157 stable on a real GPU. Playwright's Firefox (a Nightly-derived 157.0 build) crashes at startup on macOS 27.0.1 ("Could not find profile folder", even outside Playwright). The Firefox runs here are that build on Linux with llvmpipe WebGL, in Docker: they cover behaviour, not timing or stable-release quirks.
- Bun-shaped lifecycle events from [#19](https://github.com/yuann3/yuanme/issues/19) (`dispose` plus carried `data`). They weren't built. `before-swap` is the natural `dispose` point, and a `data` object on the swap event would carry state across.
- Panel scroll on back. #12 recommended restoring the panel's scroll from entry state. Astro doesn't, so the prototype doesn't either (0 after back in every engine). Restoring it would be an improvement over Astro, so it belongs with the Parity exceptions ([#23](https://github.com/yuann3/yuanme/issues/23)).
- The event prefix. The prototype still fires `astro:before-swap` / `astro:after-swap` / `astro:page-load`; whether the Engine's events are `yuanme:*` is still open.
- The `precommitHandler` traverse bug from #12 doesn't arise, because this router never passes `precommitHandler`. The Engine's router must apply #12's guard when it adds `redirect()`.
- A failed load falls back to a full load: when the response isn't HTML, has no `[data-shell]` or the fetch fails, the handler calls `location.reload()`, so the panel never shows a stale Page.
