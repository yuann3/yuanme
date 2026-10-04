# Client router: native cross-document View Transitions or a JS swap router?

Ticket: [#12](https://github.com/yuann3/yuanme/issues/12). Researched 2026-10-04. Browser versions current on that date: Chrome 154, Firefox 157, Safari 27 ([BCD `browsers/*.json`][bcd]).

## Question

The Reference site's Shell keeps the rail, the Panel frame and the WebGL Film canvas alive across navigation. It swaps only the `<html>` attributes, the `<head>` and `[data-panel-content]` ([shell.ts:301-328][shell], [Shell.astro:62-108][shellastro]). Astro's `<ClientRouter />` does this today with 15,475 B of minified router JS, 5,118 B brotli ([inventory §1, JS table][inv]). Can native cross-document View Transitions (`@view-transition`, `pageswap`/`pagereveal`) replace that router while the Film canvas persists? If they can't, what is the smallest JS router, and what does it have to do?

## Short answer

**No.** A cross-document View Transition is a real navigation. The old Document is unloaded and the new one creates a new canvas and a new WebGL context. All the transition carries over is a static image of the old page ([css-view-transitions-2 §8.1.4][vt2]; confirmed by a spike in Chromium 153 and WebKit 26.6, see [Spike](#spike-results)). Firefox 157 also still ships no cross-document View Transitions ([BCD][bcd], [bug 1860854][bz1860854]). The Film, the morph and the state-preserving swap therefore need a same-document JS router.

**Recommendation: the Engine ships its own small router built on the Navigation API**, which became cross-browser in Firefox 147 (Jan 2026) and Safari 26.2 (Dec 2025) ([BCD][bcd]). In browsers without the Navigation API the router does nothing and links do ordinary full page loads. Each Site page is already complete HTML, so nothing breaks. A throwaway spike covering the Reference site's whole router subset (intercept, fetch, redirect adoption, head/root/content swap, swap events, script dedupe, panel-scroll restoration, announcer, hover prefetch) came to **3,636 B minified, 1,428 B brotli**. That is about a quarter of Astro's router (15,475 / 5,118 B) and a tenth of Turbo's (93,519 / 22,607 B). It kept the WebGL context through push, a redirect and back in Chromium and WebKit. The Engine router should:

- emit a `before-swap` event whose `swap` the Site can override, plus `after-swap` and `page-load`;
- default to `focusReset: 'manual'` and `scroll: 'manual'`, and restore the panel scroll from entry state;
- adopt redirects through `precommitHandler` + `controller.redirect()` where available (only on cancelable, non-traverse events; see [Verification](#verification)), otherwise through `history.replaceState` after the swap;
- prefetch with `<link rel=prefetch>` on hover, not Speculation Rules.

Cross-document `@view-transition` stays available as a separate, zero-JS option for Sites that have no persistent canvas.

## Comparison

Sizes are esbuild 0.28.2 `--bundle --minify --format=esm`, then gzip -9 and brotli -q 11, measured 2026-10-04 in `/private/tmp/routerbench` ([method](#sizes)). "Canvas persists" means the WebGL context survives a navigation.

| Option | Version (last publish) | Canvas persists | Firefox 157 | min / gz / br (B) | Notes |
|---|---|---|---|---|---|
| Native cross-doc VT (`@view-transition`) | Chrome 126+, Safari 18.2+ | **No**, new Document and context | No transition, plain load | 0 | Old page is a static image; 4 s timeout in Chrome; nothing to bundle |
| Cross-doc VT + state handoff (`pageswap` → `sessionStorage` → `pagereveal`) | same | **No**, faked by restarting the shader at the handed-off time | Plain load, film restarts | about 0.3 KB extra | Shader recompile and fallback flash on every nav; morph becomes a snapshot animation |
| Astro `<ClientRouter />` (status quo) | Astro 5.16.5 in use; Astro 7.3.5 current | Yes (custom `event.swap`) | Yes | 15,475 / 5,772 / 5,118 | Tied to Astro; History API; wraps `startViewTransition`, which the site neutralises |
| Turbo Drive | 8.0.23 (2026-01-29) | Yes, with `data-turbo-permanent` (node is transferred) | Yes | 93,519 / 25,721 / 22,607 | Replaces `<body>`; brings Frames and Streams too |
| swup core | 4.10.0 (2026-09-03) | Yes (only `containers` are replaced) | Yes | 22,245 / 7,721 / 6,955 | Head, scroll, preload and a11y are plugins |
| swup + head, scroll, preload, a11y plugins | 4.10.0 + plugins | Yes | Yes | 47,529 / 15,352 / 13,856 | The plugins are needed for parity |
| @barba/core | 2.10.3 (2024-08-12) | Yes (container swap) | Yes | 32,319 / 10,457 / 9,402 | No release in over two years |
| htmx `hx-boost` | 2.0.11 (2026-09-22) | Only with extra attributes | Yes | 61,704 / 18,521 / 16,549 | A general hypermedia library, much more than a router |
| **Hand-rolled, Navigation API (spike)** | n/a | **Yes** (verified) | Yes (Firefox 147+) | **3,636 / 1,649 / 1,428** | Browser owns history, traversal, abort and entry state |

## Details

### 1. Cross-document View Transitions cannot keep a WebGL context

- The spec lifecycle: "If the ViewTransition is not skipped, the state of the old document is captured. The navigation proceeds: the old Document is unloaded, and the new Document is now active" ([css-view-transitions-2 §8.1.4 Lifecycle, Editor's Draft 31 August 2026][vt2]).
- The transition paints "a static visual capture of the old state, and a live capture of the new state" ([css-view-transitions-1 §1][vt1]; the same wording is in [level 2][vt2]). So during a cross-document morph the old Film is a frozen bitmap. The new Film is a new canvas that shows nothing until its new context has compiled the shader and drawn a frame. Until then the CSS fallback shows ([inventory, film.ts][inv]).
- Only View-Transition metadata crosses documents: "A view transition params is a struct whose purpose is to serialize view transition information across documents" (named elements and the snapshot containing block size; [vt2 §12.1.4][vt2]). Script state, DOM nodes and GL contexts do not cross.
- The spike agrees ([below](#spike-results)). After a cross-document VT navigation in Chromium 153 and WebKit 26.6, `performance.timeOrigin` and a per-document random id had changed, and a `sessionStorage` counter of `getContext('webgl')` calls had gone from 1 to 2. `pageswap` and `pagereveal` both carried a `viewTransition` object, so the transition really ran.
- **State handoff can only fake persistence.** A Site could save the film's palette, tween phase and clock in `pageswap` and restart from them in `pagereveal`, which is the documented pattern for passing data ([Chrome cross-document guide][chromexdoc]). It still recompiles the shader, re-parses the 19.9 KB inline fallback CSS and re-evaluates the 26.7 KB Shell bundle on every navigation ([inventory §1][inv]). It also turns the WAAPI clip-path morph ([shell.ts:97-142][shell]) into a snapshot-group animation. That animation cannot be interrupted mid-flight the way the current morph can.
- Other constraints:
  - Cross-document VT only runs for same-origin `push`/`replace` navigations that the user started from page content, and for `traverse`. It does not run for reloads or for navigations from the URL bar ([MDN @view-transition][mdnvt]).
  - Chrome skips the transition when the navigation takes "more than four seconds" ([Chrome guide][chromexdoc]).
  - While rendering is suppressed, "all pointer hit testing must target its document element" ([vt1 §7.1.1][vt1]), and during the animation the captured elements do not respond to hit-testing ([vt1][vt1]). This would interrupt the rail's hover preview (`film:preview`) and the pointer swirl.
  - A new Document also replays the cold-boot reveal (`body[data-boot]`; [shell.ts:332-362][shell]) unless every page detects that it arrived by navigation.
- **Support (BCD, 2026-10-03):**
  - `@view-transition`: Chrome 126, Safari 18.2, Firefox none.
  - `pagereveal`: Chrome 123, Safari 18.2. `pageswap`: Chrome 124, Safari 18.2 (partial: not fired on cross-origin navigation).
  - `<link rel=expect>` with `blocking=render`: Chrome 124 / 105, Safari 18.2, Firefox none.
  - Mozilla's meta bug for View Transitions level 2 is still `NEW` (last changed 2026-09-24) ([bug 1860854][bz1860854]). The `pageswap` bug is `ASSIGNED` ([bug 1881438][bz1881438]).
  - Same-document View Transitions are in all three engines (Firefox 144) ([BCD `api/Document.json`][bcd]), but the Reference site doesn't use them ([inventory, View Transitions][inv]).
- Astro's own docs make the same split. Native cross-document transitions "don't alter the core functionality of a multi-page application", and `<ClientRouter />` "will increasingly become unnecessary" as browser APIs evolve ([Astro view transitions guide][astrovt]). A persistent live canvas is exactly the case those APIs don't cover yet.

### 2. The Navigation API is now cross-browser and does most of a router's work

BCD (2026-10-03) lists `Navigation`, `NavigateEvent.intercept`, `NavigateEvent.scroll`, `sourceElement`, `hasUAVisualTransition` and `navigation.activation` in Chrome 102-135, Firefox 147 and Safari 26.2 ([BCD `api/Navigation.json`, `api/NavigateEvent.json`][bcd]). Firefox 147 shipped 2026-01-13 and Safari 26.2 on 2025-12-12. Current ESR Firefox 153 is included. What the browser now does for the router:

- **Interception and history.** One `navigate` listener sees link clicks, `navigation.navigate()`, and back/forward to entries this Document created. `intercept()` turns them into same-document navigations, so there is no click listener and no hand-written `pushState` or index bookkeeping. The Astro router keeps that bookkeeping by hand today ([inventory, History][inv]). `canIntercept`, `hashChange`, `downloadRequest` and `formData` give the skip conditions that Astro computes by hand ([inventory, Click interception][inv]).
- **Abort.** `event.signal` aborts the fetch when a newer navigation starts, replacing Astro's AbortController.
- **Focus.** The default `focusReset: 'after-transition'` focuses the first `autofocus` element or `<body>` ([MDN intercept][mdnintercept]). The Reference site does **not** move focus: a focused rail link keeps it ([inventory, Focus][inv]). For parity the router passes `focusReset: 'manual'`.
- **Scroll.** `scroll: 'after-transition'` restores or resets the **document** scroll ([MDN intercept][mdnintercept]). The Panel is its own scroll container and the window never scrolls on desktop, so the router passes `scroll: 'manual'`. It keeps the panel's `scrollTop` in entry state (`navigation.updateCurrentEntry`) and restores it on traverse. This also answers inventory question 29 more cheaply than Astro could: Astro never restored the panel scroll.
- **Redirect adoption** (inventory: the Netlify `/x` → 301 → `/x/` case). There are two paths, both verified in the spike:
  - `precommitHandler` with `controller.redirect(res.url)` changes the URL before it commits. It is clean: no abort and one entry. Chrome 141 and Firefox 147 support it; Safari only in Technology Preview ([BCD `api/NavigationPrecommitController.json`][bcd]).
  - The fallback is `history.replaceState(state, '', res.url)` after the swap. It works in Chromium and WebKit with one history entry, but it **aborts the in-flight navigation** (`signal.aborted === true`, so `navigateerror` fires). The router has to finish its swap before it adopts the URL and must ignore its own replace event.
  - The better fix is upstream: the Engine controls every `href`, so it should emit canonical trailing-slash URLs, and the redirect never happens.
- **iOS swipe-back.** `hasUAVisualTransition` says the browser already animated a back gesture, so the router can skip the morph instead of animating twice.

### 3. What the router itself must still do

This list comes from the parity inventory's Client runtime section ([inv][inv]) plus the items above. Each line is one or a few statements in the spike.

1. **Opt-in and fallback.** Act only when the current page has `[data-shell]`. Fall back to `location.assign` when the response is not HTML, is a cross-origin redirect, or the new page has no shell (for example `/rss.xml`, which Astro fetched twice; the spike filters by extension first).
2. **Fetch** with `event.signal`. Use `DOMParser` and strip `<noscript>`. Status codes are not checked; whether a 404 swaps into the panel is an open question (inventory Q32).
3. **Events.** Fire `before-swap` with `{from, to, navigationType, newDocument, swap}` where `swap` can be replaced: shell.ts overrides it ([shell.ts:301][shell]), and film.ts reads the pre-swap state ([film.ts:597-598][film]). Then `after-swap`, by which point the new `<html>` attributes and `aria-current` are set ([film.ts:599-615][film]), then `page-load`, which also fires once on cold load.
4. **Default swap.** Copy the root attributes, swap the `<head>` except for scripts and identical stylesheets, and replace `[data-panel-content]`. The Site's override keeps the morph sequence synchronous in one task ([inventory, Custom shell swap][inv]).
5. **Scripts in swapped content.** Run each new `src`/inline script once per Document (Astro's `data-astro-exec` behaviour), or have the Engine bundle page scripts instead (inventory Q28).
6. **Route announcer.** One `aria-live=assertive` node, reused. Astro appends a new one per navigation (inventory Q33).
7. **Prefetch.** On hover or focus after 80 ms, and on `touchstart`, append `<link rel=prefetch>`. Skip on `saveData`. Speculation Rules are not a cross-browser replacement: prefetch is Chrome 110+ only, Safari 26.2 has it only behind a preference, and Firefox has none ([BCD `html/elements/script.json`][bcd]). Even `<link rel=prefetch>` sits behind a preference in Safari ([BCD `html/elements/link.json`][bcd]), so Safari gets no prefetch either way. The same holds today.
8. **No motion of its own.** Reduced-motion behaviour stays in the Site (inventory, Reduced motion). The router never calls `startViewTransition` by default (inventory Q31). The site neutralises it today with `:root{view-transition-name:none}`.

### 4. Why not adopt an existing library

- **Turbo 8.0.23** is the largest option at 22.6 KB brotli. Turbo Drive replaces the `<body>` and keeps state only through `data-turbo-permanent` elements, which it "transfers ... from the original page to the new page, preserving their data and event listeners" ([Turbo handbook][turbo]). The rail, Panel frame and canvas would all need to be permanent, and the clip-path morph would have to work around a body swap.
- **swup 4.10.0** fits the model best: `containers` are "the content containers to be replaced on page visits" ([swup options][swup]). But parity needs the head, scroll, preload and a11y plugins (13.9 KB brotli). Its history animations off by default, and with them on swup "has to disable native browser scroll restoration" ([swup options][swup]).
- **barba 2.10.3** has had no release since 2024-08-12 ([npm][npmbarba]). **htmx 2.0.11** is a general hypermedia library at 16.5 KB brotli.
- All of them are built on the History API and re-implement what the Navigation API now provides. All of them add an npm dependency the Engine would have to bundle (ADR-0003). ADR-0001 already says "we ship small hand-written JS".

### 5. Where cross-document VT still fits

For a Starter or Site **without** a persistent canvas, `@view-transition { navigation: auto; }` plus `view-transition-name` gives animated navigation with zero JS. Firefox degrades to plain loads ([MDN @view-transition][mdnvt]). The Engine can expose both:

- `router = "native"`: emit the at-rule and no JS;
- `router = "swap"`: the Navigation API router.

The two should not be combined on one Site. If the swap router is active, the at-rule only applies to the full-load fallbacks, where its 4 s timeout and its snapshot of a frozen Film would look like a glitch.

## Spike results

The spike lives in `/private/tmp/vtspike` (pages, a Python server that 301s `/x` → `/x/`, and Playwright scripts) and `/private/tmp/routerbench`, run with Playwright 1.63.0. Each page has a `<canvas>` with a rAF-driven WebGL clear loop. A per-Document random id and a `sessionStorage` counter of `getContext('webgl')` calls detect a new Document and a new context.

| Scenario | Chromium 153.0.8010.12 | WebKit 26.6 |
|---|---|---|
| Cross-doc VT `/vt-a/` → `/vt-b/` | New Document, context count 2, `pageswap`/`pagereveal` had a viewTransition | Same |
| Navigation API, redirect via `replaceState` after swap | Same Document and context; URL `/b/`; 2 entries; `signal.aborted = true` after replaceState; back swaps `/a/` | Same |
| Navigation API, redirect via `precommitHandler` + `redirect()` | Same Document and context; URL `/b/`; 2 entries; no abort | Same (this Playwright WebKit build exposes it; Safari 27 stable does not per BCD) |
| Hand-rolled router: push to a redirecting URL, then back | Same Document, context count 1, root attrs and `<meta>` swapped, events `page-load → before-swap → after-swap → page-load`, panel scroll 0 on push and restored to 500 on back, `/rss.xml` did a full load | Same |

Limits: Playwright's Firefox 155 build would not launch in this environment ("Could not find profile folder"), so Firefox behaviour comes from BCD only. Headless GPUs are software-rendered, so the spike measured no shader-compile or first-frame timing.

### Sizes

`npm i @hotwired/turbo@8.0.23 swup@4.10.0 @swup/{head,scroll,preload,a11y}-plugin @barba/core htmx.org@2.0.11`. The entries were a bare import (Turbo, htmx), `new Swup({containers:['[data-panel-content]']})` with or without the four plugins, and `barba.init({})`. Each was built with `esbuild --bundle --minify --format=esm` and then compressed with `gzip -9c` and `brotli -q 11`. The Astro figures come from the Reference site's built `dist/` ([inventory §1][inv]: ClientRouter chunk 3,893/1,686/1,474 plus router chunk 11,582/4,086/3,644).

## Verification

An independent re-check on 2026-10-04 held the recommendation. What was re-checked, and what changed:

- **Reproduced.** The BCD entries for `@view-transition`, `PageSwapEvent`/`PageRevealEvent`, `Window.pageswap_event` (Safari partial), `Navigation`, `NavigateEvent`, `NavigationPrecommitController`, `speculationrules` and `link rel=prefetch` match the text ([BCD main][bcdmain]). Browser release dates match: Firefox 147 on 2026-01-13, Safari 26.2 on 2025-12-12, Firefox 157 current, ESR 153. Mozilla bug 1860854 is `NEW` (last changed 2026-09-24) and bug 1881438 is `ASSIGNED` ([Bugzilla REST][bz1860854]). The npm versions and publish dates match (`npm view`). htmx also has a `4.0.0` release on its `next` tag, which changes nothing here. Rebuilding every entry in `/private/tmp/routerbench` with esbuild 0.28.2 gave the same minified and brotli sizes to the byte. Gzip came out within 4 B, because of the gzip header. The Astro figures add up from the inventory chunks (3,893 + 11,582 = 15,475; 1,474 + 3,644 = 5,118). The spike's `result.json` shows the logged evidence for each row of the spike table. The spec quotes (vt2 ED 31 August 2026) and the Chrome guide's four-second timeout and navigation-type list check out.
- **Correction: Chrome versions.** `NavigateEvent.intercept()` and `scroll` shipped under those names in Chrome 105. Chrome 102-107 had them as `transitionWhile`/`restoreScroll`. `hasUAVisualTransition` is Chrome 118, `activation` 123 and `sourceElement` 135. "Chrome 102+" is right only for the `Navigation` interface itself. All of these predate any browser the Engine targets.
- **Correction: `precommitHandler` in Safari.** BCD lists the `NavigationPrecommitController` interface as Safari Technology Preview, but lists the `intercept()` option `precommitHandler` as Safari `false`. Feature-detect the option, not just the interface. A check for `'NavigationPrecommitController' in window` can pass where the option is ignored.
- **Bug in the spike router: `precommitHandler` on traversals.** `handrolled.js` passes `precommitHandler` to every intercepted navigation whenever `NavigationPrecommitController` exists, back/forward included. The HTML spec makes a traverse `navigate` event non-cancelable when the user goes back through browser UI and the page has no history-action activation. `intercept()` then throws if given a `precommitHandler`: "trying to pass a precommitHandler to a non-cancelable NavigateEvent will throw" ([HTML §7.2.6 Navigation API][htmlnav]). The note calls it a `SecurityError`, the algorithm an `InvalidStateError`, and MDN says `SecurityError`. In Chrome and Firefox, the listener would then throw and the same-document traversal would not be intercepted, so the URL would change but the Panel would keep the old page. The re-check could not reproduce this in Playwright, whose headless pages always report `userActivation.hasBeenActive`, so the spike's "back" rows don't cover it. **Fix:** pass `precommitHandler` only when `e.cancelable && e.navigationType !== 'traverse'`. A traversal goes to a URL the router has already adopted, so it never needs a redirect. Add this to the Engine router's test list, along with a check that `intercept()` errors fall back to `location.reload()` instead of leaving a stale Panel.
- **No better option missed.** Moving the GL work into an `OffscreenCanvas` in a `SharedWorker` would keep the compiled program, but each new Document still needs its own on-screen canvas and a first frame, and `SharedWorker` is missing from Chrome for Android. It doesn't remove the reload. Iframe-based shells break URLs, history and indexing. Neither beats a same-document router.

## Open questions

1. **Engine router name and event names.** The Reference site's scripts listen for `astro:before-swap`, `astro:after-swap` and `astro:page-load`. The Engine needs its own prefix, which depends on the project name, now under discussion. Keep an `astro:*` alias for porting, or rename film.ts and shell.ts once (inventory Q27)?
2. **Browsers without the Navigation API** (Firefox < 147, Safari < 26.2): accept full page loads (recommended, zero bytes), or add a History-API fallback of about 1-2 KB?
3. **Redirect strategy.** Emit trailing-slash hrefs everywhere so there are no 301s (recommended, and also faster), and keep `precommitHandler`/`replaceState` only as a safety net? This changes the Reference site's link markup, so it would be a Parity exception.
4. **Panel scroll restoration on back/forward.** Adopt it (one `updateCurrentEntry` call) as a Parity exception, or keep Astro's always-top behaviour (inventory Q29)?
5. **Speculation Rules** when Safari ships them without a preference: add a `prefetch` document rule next to the hover `<link rel=prefetch>`?
6. **Hybrid VT mode.** Is a "native" router mode with film-state handoff worth offering for canvas Sites that don't need interruptible morphs? This needs a timing measurement on real GPUs: shader compile to first frame, against the snapshot morph.
7. **Real-browser Firefox check.** Re-run the spike on Firefox 157 stable on a desktop before the router lands.

[bcd]: https://github.com/mdn/browser-compat-data/tree/d26d7c58d03d2e3d673261e529cbdad7689db401
[bcdmain]: https://github.com/mdn/browser-compat-data
[htmlnav]: https://html.spec.whatwg.org/multipage/nav-history-apis.html#navigation-api
[vt1]: https://drafts.csswg.org/css-view-transitions-1/
[vt2]: https://drafts.csswg.org/css-view-transitions-2/#lifecycle
[bz1860854]: https://bugzilla.mozilla.org/show_bug.cgi?id=1860854
[bz1881438]: https://bugzilla.mozilla.org/show_bug.cgi?id=1881438
[chromexdoc]: https://developer.chrome.com/docs/web-platform/view-transitions/cross-document
[mdnvt]: https://developer.mozilla.org/en-US/docs/Web/CSS/@view-transition
[mdnintercept]: https://developer.mozilla.org/en-US/docs/Web/API/NavigateEvent/intercept
[astrovt]: https://docs.astro.build/en/guides/view-transitions/
[turbo]: https://turbo.hotwired.dev/handbook/building
[swup]: https://swup.js.org/options/
[npmbarba]: https://www.npmjs.com/package/@barba/core
[inv]: ./astro-parity-inventory.md
[shell]: https://github.com/yuann3/eyuan.me/blob/main/src/scripts/shell.ts
[film]: https://github.com/yuann3/eyuan.me/blob/main/src/scripts/film.ts
[shellastro]: https://github.com/yuann3/eyuan.me/blob/main/src/layouts/Shell.astro
