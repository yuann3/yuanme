# Every build re-renders the whole Site in memory, from content-hash leaf caches

A build is one function from a snapshot of the Author's files to the bytes of every Page and asset, held in memory. It reads and writes no files itself, which leaves the command-line program free to write the result to the output directory or to serve it from `yuanme dev`. Each build re-parses and re-renders the whole Site, and `yuanme dev` does the same on every save. There is no dependency graph between inputs and Pages. Builds are fast because the expensive pure transforms are memoized by a content hash of everything they read: parsed Entries, highlighted code blocks, an Entry's math, image variants, bundles and font subsets. Those caches persist in `.yuanme/cache`, keyed by the Engine version as well, so a cold `yuanme dev`, `yuanme build` or CI run starts warm. `yuanme dev` finds the Pages that changed by comparing the hashes of the new output with the previous build's.

With warm caches, re-rendering 1,000 Posts and hashing the output took 11–22 ms, and a prose edit to the heaviest Post took 33–80 µs to render ([#11](https://github.com/yuann3/yuanme/issues/11)). The budgets are 50 ms incremental and 2 s warm for the Scale Site ([#15](https://github.com/yuann3/yuanme/issues/15)). Nothing can go stale, because nothing is skipped: a cache entry is either keyed by everything it read or it is a bug, and a test catches that bug. Decided in [What is the Engine's architecture?](https://github.com/yuann3/yuanme/issues/24).

## Considered Options

- **salsa or another incremental-computation framework** (rejected): five breaking releases in ten months, and a dependency graph built for a compiler's deep query chains, where a Site's is two levels deep.
- **A per-Page dependency graph, recorded from what each render read** (deferred): it pays off only above roughly 2,000 large Pages. #19's probe showed the Template engine can record what a render reads, so it can be added later without changing anything here.
- **Rendering only the edited Page in dev** (rejected): Zola's `--fast` and Hugo's fast render serve stale lists and neighbours, and both made that an opt-in.

## Consequences

- CI replays an edit sequence through `yuanme dev` and checks that its output is byte-identical to a clean `yuanme build` with the same visibility and the dev client stripped. That turns "every cache key is complete" into a test.
- Every cache key includes the Engine version, and anything from Site config that the transform reads: highlight grammars and themes, math macros, the Collection schema.
- A change to a Template or to Site config re-renders everything, as every other change does. That is acceptable because rendering everything is the fast path.
- Peak memory grows with the Site's HTML, since every Page is held at once. Image bytes are not held in memory; they go straight to and from the cache. The Scale Site's HTML is about 45 MB, inside its 1 GB budget.
