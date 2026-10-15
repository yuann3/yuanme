# The built-in Router runs on the Navigation API; other browsers get full page loads

The Engine ships one small client Router. It intercepts same-origin navigations with the Navigation API, fetches the next Page, and swaps it into the current document, so elements a Site keeps between Pages (the Reference site's Film canvas and its WebGL context) survive. Browsers without the Navigation API get ordinary full page loads; there is no History API fallback. The Router fires `yuanme:before-swap` (whose `swap` a Site may replace), `yuanme:after-swap` and `yuanme:page-load`, with no `astro:*` aliases. Its budget sits inside the 13 KB br JS Budget from [#15](https://github.com/yuann3/yuanme/issues/15): at most 2 KB br for the Router itself, and 3.5 KB br for the Router plus the Site's own swap code.

Native cross-document View Transitions can't keep a canvas alive, because the old document is unloaded and only a snapshot of it is animated. Firefox also doesn't have them ([#12](https://github.com/yuann3/yuanme/issues/12)). The Navigation API is in Chrome 105, Firefox 147 and Safari 26.2. A prototype replaced Astro's `<ClientRouter />` on the Reference site and matched it on every behaviour check in Firefox 157, Chromium 156 and WebKit 27.2. It kept one WebGL context over a 12-step tour, painted soft navigations sooner than Astro, and measured 1.6 KB br against Astro's 3.6 KB. Decided in [How small can the Shell swap router be, and does it feel identical?](https://github.com/yuann3/yuanme/issues/26). The prototype and its measurements are on branch `prototype/shell-router`.

## Considered Options

- **History API fallback driver** (rejected): it keeps persistent elements alive in browsers without the Navigation API, but it costs 271 B br and a second navigation path to test, for browsers that are already out of date.
- **A hybrid of cross-document View Transitions with handoff of persistent state** (rejected): on a real GPU the Film's WebGL setup is only 8–11 ms, but a full load still loses its state, paints later than a soft swap, and does nothing in Firefox.
- **Keeping Astro's event names, or aliasing them** (rejected): the Reference site is being ported anyway, and an alias would be permanent API surface.

## Consequences

- `@view-transition` stays a separate zero-JS mode for Sites with nothing to keep alive between Pages.
- In browsers without the Navigation API, anything a Site keeps alive restarts on every navigation. That is correct, just less smooth.
- Push and replace navigations scroll to the fragment, or to the top, themselves, and only traversals call `NavigateEvent.scroll()`. Firefox 157's `scroll()` left a push where it was after the body swap.
- Redirects are adopted with `precommitHandler` + `redirect()` where that option exists, passed only when `e.cancelable` and not on traversals, and with `replaceState` in Safari, where adopting the URL aborts the navigation and fires `navigateerror`.
- A response that isn't an HTML Page the Router can swap, or a failed fetch, falls back to a full load, so a visitor never sees a stale Page.
- The dev server reuses the same swap core through a dev-only hook that swaps a Page in place without motion. Its bytes are never shipped in a built Site.
