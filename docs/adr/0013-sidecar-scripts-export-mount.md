# Sidecar scripts export `mount`, and the Engine calls it on every Page load

A Template's Sidecar script (`row.ts` beside `row.html`) may export `mount(ctx)`. The Engine calls it on every Page load that rendered that Template: once after a full load, and again after each Soft navigation. `mount` may return a cleanup function, which runs at `yuanme:before-swap` before the Page is swapped out. `ctx` is `{ url, signal, data }`: `signal` is an `AbortSignal` that aborts at cleanup, so listeners added with `{ signal }` go away with the Page, and `data` is one object per module that lasts across swaps. A module's top level still runs once per document. Site-level Scripts stay plain modules that listen for `yuanme:*` events.

In Astro's model a page's scripts run once per document. Each script then has to listen for the page-load event and check whether its own elements are on the new page, which is how the Reference site's PhotoPile and copy button work today. That code behaves differently with the Router on and off, and agents get it wrong because nothing names the contract. An exported `mount` is one contract that runs the same way in every Router mode. `check` and the type declarations can verify it, and the dev server can re-import a changed Sidecar script and mount it again instead of reloading the Page. Decided in [What is the front-end pipeline for Authors?](https://github.com/yuann3/yuanme/issues/20).

## Considered Options

- **Run once per document and listen for `yuanme:page-load`** (rejected as the model for Sidecars): there is no API to learn, but every Sidecar re-implements binding, guarding and teardown, and swapped-out Pages leak listeners unless the Author removes them.
- **Mount once per element the Template writes** (rejected): it needs a marker on every instance and multiplies calls for list rows. A Sidecar that wants per-instance work queries its elements inside `mount`.

## Consequences

- Entry signatures must survive bundling. rolldown's `preserveEntrySignatures: false` would drop `mount`, so the Engine fixes it at `exports-only`.
- `mount`, `ctx`, the cleanup and their timing are public API from v1. The Engine ships their types in `.yuanme/types/yuanme.d.ts`.
- With the Router off, `mount` runs once and cleanup never runs.
