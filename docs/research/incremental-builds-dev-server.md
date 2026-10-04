# Incremental builds and the dev server

Ticket: [#11](https://github.com/yuann3/yuanme/issues/11) (wayfinder research). Researched 2026-10-04. Vocabulary follows [`CONTEXT.md`](../../CONTEXT.md).

## Question

How should the Engine structure incremental rebuilds and `yuanme dev` so that editing one Post shows up in the browser in under 100 ms? This covers salsa-style memoized queries compared with content-hash caching, file watching (notify, debouncing, atomic saves from editors such as Emacs), live reload (SSE compared with WebSocket, CSS hot-swap without a full reload), and parallelism (rayon). It also asks how Zola, Hugo and others structure rebuilds, and where they are slow.

## Short answer

**Don't adopt salsa.** Memoize only the expensive pure leaf transforms (code highlighting, math, image variants), keyed by a content hash. Re-render every page in memory with rayon on each change, then diff the outputs to learn which URLs changed. Measured on this machine, the highlighter is where a Post edit spends its time. A code-block cache cuts the largest Reference-site Post from 10.8–50 ms to 0.04–0.08 ms. With the leaf cache warm, re-rendering **all 1,000** synthetic Posts takes 14–22 ms, so a dependency graph is not needed at personal-site scale. Add page-level dependency recording later, behind a seam, only if a Site grows past roughly 2,000 large pages.

**Measured, the latency in existing tools comes from debounce settings, not from rendering.** Zola waits 1,000 ms by default and Hugo batches every 500 ms. Hugo's rebuild of the 100-Post bench site took 10–16 ms, but save-to-reload took about 235 ms. Zola with `--fast --debounce 1` reached 21 ms but served stale HTML. The plan:

1. **Watch.** Use `notify` 8.2 with no library debouncer. Wait for 15 ms with no new event (cap 50 ms). Treat every event as "this path may have changed": re-read the file, hash it, and drop it if nothing changed. Never rely on event kinds.
2. **Serve.** Serve from an in-memory output snapshot that is swapped atomically, over HTTP plus a WebSocket.
3. **Update the browser in place.** Send the new HTML for the page being viewed. The client morphs the DOM, keeping `<canvas>` and anything marked preserve, and swaps CSS `<link>`s without a reload. It falls back to a full reload only when a script changed.

Estimated server-side total for a single Post edit is about 25–45 ms, which leaves at least 55 ms for the browser. The browser part has not been measured yet; see Open questions.

## The 100 ms budget, measured

Machine: Apple M4 Max (12P+4E cores, 16 threads), macOS 27.0.1, rustc 1.99.0, release builds. The machine was under heavy parallel load (load average 14–16) during the runs, so treat the figures as upper bounds. Each range covers repeated runs. Method: see [Appendix: spikes](#appendix-spikes).

| Stage (one Post edit) | Measured | Notes |
|---|---|---|
| FSEvents delivers the first event after the save completes | median 4.6–10.3 ms, max 12 ms (Vim-style sequence: max 24 ms) | notify 8.2.0, `recommended_watcher` (FSEvents) |
| Quiet window to collect the whole save | 15 ms proposed | Every event of one save arrived within about 12 ms of the save finishing (max 40 ms under load) |
| Hash the changed file | blake3 3–10 µs, xxh3-128 0.1–0.6 µs | 1.4–12.8 KB Posts |
| Parse Markdown to HTML, no highlighting | 27–52 µs | largest Post, `extending_c_stdlib.md`, 12.8 KB |
| Highlight 29 code blocks (syntect 5.3, warm) | 10.8–50 ms | the hot spot; the first call in a process costs 26–66 ms (lazy regex compile) |
| Same Post after a prose edit, with a code-block cache | 40–80 µs | 29/29 blocks hit |
| Template render (minijinja 2.24) | 16–50 µs | per page |
| Re-render **all** pages, leaf cache warm, rayon, output hash | 30 Posts: 0.5–2 ms · 300: 3.5–6.7 ms · 1,000: 14–22 ms | synthetic copies of the largest Post |
| Write one output file | 0.08–0.13 ms | |
| Server pipeline, save to WebSocket message (Zola `--fast --debounce 1`, as a proxy) | median 21 ms (11–41 ms) | FSEvents, 1 ms debounce, single-page render, WebSocket send |
| Browser: morph or reload, then paint | **not measured** | see Open questions |

## Comparison tables

### Incremental strategies

| Strategy | Single-Post edit cost | Correctness risk | Complexity / dependency risk | Verdict |
|---|---|---|---|---|
| Full rebuild from scratch each change (Zola default, mdBook) | 1.4 s for 100 highlighted Posts (Zola, measured). Cold highlighting of 1,000 Posts is 3–5.8 s even with rayon | none | none | Too slow once highlighting is in the loop |
| Single-page "fast" rebuild with no dependency tracking (Zola `--fast`) | 14–18 ms rebuild | **High**: listings go stale by design. In our run even the edited page was served stale | low | Rejected |
| Experimental timestamp metadata (Jekyll `--incremental`) | n/a | Jekyll says "will not work correctly in every scenario" and does not track `site.posts` iteration ([Jekyll docs](https://jekyllrb.com/docs/configuration/incremental-regeneration/)) | low | Rejected |
| Hand-built dependency tracker (Hugo ≥0.123 "identity" tracker) | 10–16 ms rebuild, measured, correct | medium: Hugo needed a dedicated dependency system ([v0.123.0 notes](https://github.com/gohugoio/hugo/releases/tag/v0.123.0)) | high | Not needed at our scale |
| **salsa** 0.28.5 (memoized query graph, red-green, backdating) | low after warm-up | low if every read goes through queries | high: macro-heavy API, 4 breaking 0.x releases in 7 months | Rejected for v1 |
| **comemo** 0.5.1 (Typst's constrained memoization with tracked access) | low | low | medium: small API, 0.x | Candidate for tier 2 |
| **Recommended: leaf memo by content hash + full in-memory re-render + output diff** | 0.5–2 ms render stage for 30 Posts; 14–22 ms for 1,000 | low: only the leaf keys must be complete, and a test can check it | low | **Adopt** |

### How existing dev servers rebuild and reload

| Tool (version) | Watcher, default settle | Rebuild on content edit | Transport | CSS without reload | HTML without reload | Measured save → reload message, 100-Post bench site |
|---|---|---|---|---|---|---|
| Zola 0.23.6 | notify-debouncer-full, **1,000 ms** | whole site (`recreate_site`). `--fast` renders one page only. Any template change rebuilds everything | WebSocket, LiveReload protocol, livereload.js 3.2.4 | via livereload.js (Zola always sends the path `/x.js` or a file path, so in practice a full reload) | no | default **2.48 s** median; `--debounce 1` 0.99 s; `--debounce 1 --fast` 21 ms (stale output) |
| Hugo 0.167.0 | fsnotify plus a **500 ms ticker** batcher | dependency-tracked partial rebuild. "Fast render" re-renders the 20 most recently visited pages and renders others on navigation | WebSocket, LiveReload protocol | yes: `RefreshPath` for each changed `.css` | no (full reload, or navigate to the changed page with `--navigateToChanged`) | **236 ms** median (rebuild itself 10–16 ms); fast render on or off made no difference |
| mdBook 0.5.4 | notify-debouncer-mini, **1 s** | whole book | WebSocket, `"reload"` text | no | no | not measured |
| Eleventy Dev Server 3.0.0-alpha.12 | chokidar | `--incremental`: changed templates plus layout and collection dependents | WebSocket | yes: cache-bust each matching `<link>` | **yes**: morphdom of `document.documentElement` with the server-sent HTML (`domDiff: true`) | not measured |
| tower-livereload 0.10.3 (Rust middleware) | none (BYO) | n/a | **SSE** | no | no (full reload) | n/a |

### Live-reload transport

| | SSE (`EventSource`) | WebSocket |
|---|---|---|
| Browser connection limit over HTTP/1.1 | **6 per browser per origin, shared by all tabs**. Chrome and Firefox marked it "Won't fix" ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/EventSource); [WHATWG spec](https://html.spec.whatwg.org/multipage/server-sent-events.html) warns about per-server connection limits) | not subject to the SSE warning |
| Direction | server → client only | both ways (the client can report which URL it is viewing) |
| Reconnect | built in; the `retry:` field sets the delay | hand-written (about 10 lines) |
| Rust side | `axum::response::sse` | axum 0.8.9 `ws` feature (tokio-tungstenite 0.30) |
| Precedent among SSGs | tower-livereload | Zola, Hugo, mdBook, Eleventy, Vite |
| Agent friendliness | `curl -N` readable | needs a client; better to give agents NDJSON on stdout (see below) |

## Details

### 1. Where Zola and Hugo actually spend the time

**Zola.** The default `--debounce` is 1,000 ms ([cli.rs L110–112](https://github.com/getzola/zola/blob/42c89b67214477358a9a4de14f181c474d31a937/src/cli.rs#L110-L112)). notify-debouncer-full is a trailing debouncer: it emits only after `timeout` of quiet, with a tick of 1/4 of the timeout ([debouncer-full lib.rs L738–764](https://github.com/notify-rs/notify/blob/5ebf4c0ba4e51ffae0a1d73476f9de9f505a82d8/notify-debouncer-full/src/lib.rs#L738-L764)). So at least 1 s passes before Zola does any work.

- **Rebuild scope.** Without `--fast`, a content change calls `recreate_site()`, which reloads and re-renders everything. With `--fast` it calls `add_and_render_page`, which queues only that page ([serve.rs L806–851](https://github.com/getzola/zola/blob/42c89b67214477358a9a4de14f181c474d31a937/src/cmd/serve.rs#L806-L851); [site lib.rs L559–564](https://github.com/getzola/zola/blob/42c89b67214477358a9a4de14f181c474d31a937/components/site/src/lib.rs#L559-L564)). Since 0.23.5, any template change rebuilds the whole site as a "temporary fix" for issue #3246 ([serve.rs L853–880](https://github.com/getzola/zola/blob/42c89b67214477358a9a4de14f181c474d31a937/src/cmd/serve.rs#L853-L880); CHANGELOG 0.23.5).
- **Measured.** On a 100-Post site using the Reference site's largest Post as the body, a full rebuild took 1.3–1.4 s. Save-to-reload took 2.48 s by default, 0.99 s with `--debounce 1`, and 21 ms with `--debounce 1 --fast`.
- **Staleness under `--fast`.** After a title edit, both `/blog/` and the page itself still showed the old title. Without `--fast` both updated. This was reproduced twice with an in-place write. We did not find the root cause and found no matching issue.
- **Output and editor filtering.** Zola serves HTML from memory by default (`--store-html` writes to disk; [cli.rs L94–96](https://github.com/getzola/zola/blob/42c89b67214477358a9a4de14f181c474d31a937/src/cli.rs#L94-L96)). It filters editor temp files with a hard-coded denylist of extensions and prefixes (`swp`, `~`, `#…`, `.#…`, JetBrains `___jb_*___`, Helix `bck`, Kate `kate-swp`) ([utils fs.rs L185–212](https://github.com/getzola/zola/blob/42c89b67214477358a9a4de14f181c474d31a937/components/utils/src/fs.rs#L185-L212)). Its changelog shows that list being patched editor by editor ("Handle more editors with change detection", "Ignore `.bck` files").

**Hugo.** File events are batched by a fixed 500 ms ticker ([hugobuilder.go L332](https://github.com/gohugoio/hugo/blob/6b3ba3a7e22ae2809605b8118aeed08b244f1955/commands/hugobuilder.go#L332); [watcher/batcher.go](https://github.com/gohugoio/hugo/blob/6b3ba3a7e22ae2809605b8118aeed08b244f1955/watcher/batcher.go)). That adds 0–500 ms (about 250 ms on average) before the rebuild starts, which matches the measured 236 ms median against a 10–16 ms rebuild.

- **Correct partial rebuilds.** Hugo has a dependency tracker that "quickly calculates the delta given a changed resource … and supports transitive relations" ([v0.123.0 release](https://github.com/gohugoio/hugo/releases/tag/v0.123.0)). Our title edit updated the page, the section list and the home list.
- **Fast render.** Fast render mode keeps an evicting queue of the 20 most recently visited URLs ([server.go L95–98](https://github.com/gohugoio/hugo/blob/6b3ba3a7e22ae2809605b8118aeed08b244f1955/commands/server.go#L95-L98)). It re-renders other pages lazily when the browser navigates to them ([server.go L372–393](https://github.com/gohugoio/hugo/blob/6b3ba3a7e22ae2809605b8118aeed08b244f1955/commands/server.go#L372-L393)).
- **Live reload.** After a build, Hugo sends `RefreshPath` for each changed `.css` file and force-refreshes otherwise. With `--navigateToChanged` it navigates the browser to the edited page ([hugobuilder.go L1003–1075](https://github.com/gohugoio/hugo/blob/6b3ba3a7e22ae2809605b8118aeed08b244f1955/commands/hugobuilder.go#L1003-L1075)). `hugo server` writes to disk by default; `--renderToMemory` is opt-in ([server.go L526–536](https://github.com/gohugoio/hugo/blob/6b3ba3a7e22ae2809605b8118aeed08b244f1955/commands/server.go#L526-L536)).

**mdBook.** It uses `notify_debouncer_mini::new_debouncer(Duration::from_secs(1), …)` and runs a full `build()` on every change ([native.rs L26, L94](https://github.com/rust-lang/mdBook/blob/22cce19e14eecd3a4695a84d4173f994d739c3d2/src/cmd/watch/native.rs#L26)). It then sends a bare `"reload"` over a WebSocket ([serve.rs L135](https://github.com/rust-lang/mdBook/blob/22cce19e14eecd3a4695a84d4173f994d739c3d2/src/cmd/serve.rs#L135)).

**Eleventy.**

- **Incremental builds.** `--incremental` rebuilds changed templates. A layout change rebuilds the templates that use it, and a tag change rebuilds templates that consume that collection. A config change or an unknown include forces a full build ([docs](https://www.11ty.dev/docs/usage/incremental/)).
- **In-place DOM updates.** Its dev server is the only surveyed tool that updates HTML without a reload. The server sends the rendered HTML of changed templates, and the client runs `morphdom(document.documentElement, content, …)` when the URL matches the current page ([reload-client.js L187–345](https://github.com/11ty/eleventy-dev-server/blob/5dc5d7a979f4e58b6a544067a0174e9848de78d5/client/reload-client.js#L187-L345)).
  - The morph skips `<link>` nodes and elements carrying a preserve attribute, and doesn't touch a focused input.
  - It falls back to a full reload when a `<script>` is added or changed.
  - When a permalink moves, it redirects to the new URL.
  - Afterwards it dispatches a DOM event so page scripts can rebind ([L471–489](https://github.com/11ty/eleventy-dev-server/blob/5dc5d7a979f4e58b6a544067a0174e9848de78d5/client/reload-client.js#L471-L489)).
- **CSS.** Updates cache-bust the matching `<link>` hrefs ([L216–244](https://github.com/11ty/eleventy-dev-server/blob/5dc5d7a979f4e58b6a544067a0174e9848de78d5/client/reload-client.js#L216-L244)).

**Takeaway.** In every tool measured, most of the time goes to fixed debounce or batching windows sized for slow full builds, not to rendering. Tools that rebuild only part of the site are either incorrect (Zola `--fast`, Jekyll) or need a real dependency tracker (Hugo).

### 2. Incremental computation: salsa, comemo or content hashes

**salsa** (0.28.5, released 2026-09-24, actively maintained) memoizes "tracked functions" over "inputs". On a new revision it re-validates memos by walking dependencies, and "backdates" a result that recomputes to an equal value, so dependents are not re-run (early cutoff) ([book: the red-green algorithm](https://github.com/salsa-rs/salsa/blob/30b614d826d697c47bc0f21591fab218f2004032/book/src/reference/algorithm.md)). Durability lets it skip checking inputs that rarely change ([book: durability](https://github.com/salsa-rs/salsa/blob/30b614d826d697c47bc0f21591fab218f2004032/book/src/reference/durability.md)). This is the right tool for deep, fine-grained query graphs such as a compiler front end. It does not fit the Engine for four reasons:

- **Unstable API.** Semver-breaking 0.x releases came out on 2025-12-16 (0.25), 2026-02-02 (0.26), 2026-06-04 (0.27) and 2026-07-12 (0.28). 0.28 changed core traits ("replace `Update` with `SalsaValue` and `PartialEq`", "return references by default") ([CHANGELOG](https://github.com/salsa-rs/salsa/blob/30b614d826d697c47bc0f21591fab218f2004032/CHANGELOG.md)).
- **The SSG graph is shallow.** It runs file → parsed entry → leaf transforms → page HTML → output, with fan-in only at Collection listings and feeds. The expensive nodes are pure leaves with small, hashable inputs.
- **Most of the Engine sits outside salsa's model anyway.** Writing outputs, encoding images and the HTTP server are side-effecting.
- **It costs agents and contributors.** Proc-macro-heavy query code is harder for a coding agent to change correctly, and agent ergonomics are a first-class requirement (map #1).

**comemo** (0.5.1, 2026-01-29; Typst's incremental engine) memoizes a function together with a record of which methods of a `Tracked<T>` argument it called. It reuses the result while those calls would still return the same values, "even if other files change" ([README](https://github.com/typst/comemo/blob/5944487f5e9d11949038662df4b307ec56f153b3/README.md)). That access-tracking model is exactly what page-level reuse needs: a page that reads only `posts[*].title` should not re-render when a body changes. Keep it as the tier-2 candidate.

**Content-hash leaf memo**, the "verifying traces" or "constructive traces" family in Build Systems à la Carte ([Mokhov, Mitchell, Peyton Jones](https://www.microsoft.com/en-us/research/publication/build-systems-a-la-carte/)), with early cutoff. Each expensive pure transform is a function `key → output`, where the key hashes everything the function reads. The measurements show this captures nearly all of the available win:

- **Highlighting dominates.** It costs 10.8–50 ms warm for one 29-block Post, against 27–52 µs to parse and 16–50 µs to template.
- **A cache hit removes it.** The same Post after a prose edit renders in 40–80 µs with a code-block cache.
- **Full re-render stays cheap.** Re-rendering 1,000 such Posts with a warm cache, then hashing every output, takes 14–22 ms on 16 threads.

### 3. Recommended build model

Tier 1 ships in v1:

1. **Inputs.** Map each path to `(len, mtime, xxh3-128 of bytes)`. A watcher hint re-reads only the hinted paths, and an unchanged hash ends the build as a no-op. That covers touches, Emacs lock files and editors that rewrite identical bytes.
2. **Entries.** Parse each Source file to `{ meta, body IR }`, cached by input hash plus a salt for the parser config. Markdown and Org share the metadata model. A meta-only hash makes it cheap to see whether listings could change, but in tier 1 that is only an optimisation.
3. **Leaf memo.** Code blocks are keyed by `(lang, code, highlighter version, theme)`, math by `(TeX, display, macro-set hash, renderer version)`, and image variants by `(source bytes hash, params)` with the cache on disk.
   - These are the only caches, and they are pure, so they never invalidate wrongly. A wrong key can only cause a miss or a stale fragment if the key is incomplete.
   - Prewarm the highlighter in the background at `yuanme dev` startup, because the first call in a process costs 26–66 ms.
4. **Collections** are rebuilt from the metas on every build. This is microseconds of work.
5. **Pages.** Render every page with rayon, xxh3 each output, and compare against the previous output map. The set of changed outputs gives the changed URLs, which drive live reload, with no dependency graph needed. Swap the new snapshot in atomically.
6. **Equivalence test.** In CI, an edit sequence replayed through the dev pipeline must produce byte-identical output to a clean `yuanme build`. This turns "leaf keys are complete" into a test, and it fits the snapshot-test and TDD stance in map #1.

Tier 2, added only when a budget test fails:

- Record page-level dependencies during rendering. Wrap template context values as tracked objects: minijinja's `Object::get_value` is called on every attribute access ([object.rs L172–191](https://github.com/mitsuhiko/minijinja/blob/8f5b3af730674cf6eb0da22c095aaf067f27d26a/minijinja/src/value/object.rs#L172-L191)). Wrap the template loader so it records which templates each page pulled in ([environment.rs `set_loader`](https://github.com/mitsuhiko/minijinja/blob/8f5b3af730674cf6eb0da22c095aaf067f27d26a/minijinja/src/environment.rs#L223)). Then skip pages whose recorded reads still hash the same, either hand-rolled or with comemo.
- The threshold follows from the numbers: a 100 ms budget with about 40 ms left for stage 5 allows roughly 2,000–3,000 pages of this size. That is far beyond a personal site. The Reference site has 28 pages.
- The template engine is still open (#6). Whichever engine wins must expose these two hooks.

### 4. File watching

- **Crate.** Use `notify` 8.2.0, the current stable release; 9.0.0-rc.5 is in RC (2026-08-30). It is maintained, with releases through 2026 ([CHANGELOG](https://github.com/notify-rs/notify/blob/5ebf4c0ba4e51ffae0a1d73476f9de9f505a82d8/notify/CHANGELOG.md)).
- **Backends.** `recommended_watcher` uses FSEvents on macOS, inotify on Linux and ReadDirectoryChangesW on Windows. On macOS, 8.2.0 creates the stream with `latency: 0.0` and `kFSEventStreamCreateFlagNoDefer` ([fsevent.rs L300–301 @ 8.2.0](https://github.com/notify-rs/notify/blob/notify-8.2.0/notify/src/fsevent.rs#L300-L301)), so there is no OS-side coalescing delay. 9.0 adds `Config::with_fsevent_latency`.
- **Don't use a library debouncer with a large timeout.** Write a 30-line coalescer instead:
  - On the first raw event, open a window that closes after 15 ms with no new events, capped at 50 ms after the first event.
  - Events that arrive during a build mark paths dirty. When the build ends, run again with the newest state.
  - The browser only ever gets the latest build ID.
- **Treat events as hints, never as facts.** notify warns that events "differ a lot between file editors. Some truncate the file on save, some create a new one and replace the old one" ([lib.rs L67–71](https://github.com/notify-rs/notify/blob/5ebf4c0ba4e51ffae0a1d73476f9de9f505a82d8/notify/src/lib.rs#L67-L71)). The spike confirmed this on FSEvents:

  | Save style | Events seen for the target `post.md` |
  |---|---|
  | in-place truncate + write | `Modify(Metadata)`, `Modify(Data(Content))` |
  | Emacs default first save (rename original to `post.md~`, write a new file) | `Create(File)`, `Modify(Name(Any))`, `Modify(Data)`, … plus `post.md~` |
  | write a temp file, then rename it over (Emacs `file-precious-flag`, most "atomic save" editors) | **only `Modify(Name(Any))`**, no data event |
  | Vim `backupcopy=auto` | a `4913` create/remove probe, rename, create, then remove of `post.md~` |
  | Emacs lock file | `Create(Other)` on `.#post.md` (a dangling symlink) |

  A watcher that waits for a data-modify event misses atomic saves entirely. The fix is to re-stat and re-hash every hinted path, whatever the event kind.
- **Emacs specifics, from the GNU manuals.**
  - With `backup-by-copying` nil (the default), the first save of a session backs up by renaming the old file and writing a new one ([Backup Copying](https://www.gnu.org/software/emacs/manual/html_node/emacs/Backup-Copying.html); [`save-buffer`](https://www.gnu.org/software/emacs/manual/html_node/elisp/Saving-Buffers.html)).
  - `file-precious-flag` writes "the new file to a temporary name … and then renam[es] it" ([Saving Buffers](https://www.gnu.org/software/emacs/manual/html_node/elisp/Saving-Buffers.html)).
  - The lock file is "a symbolic link with a special name … constructed by prepending `.#`", created on the first modification, before any save ([File Locks](https://www.gnu.org/software/emacs/manual/html_node/elisp/File-Locks.html); [Interlocking](https://www.gnu.org/software/emacs/manual/html_node/emacs/Interlocking.html)).
  - Auto-save files are `#name#` ([Auto Save Files](https://www.gnu.org/software/emacs/manual/html_node/emacs/Auto-Save-Files.html)).
- **Vim.** `backupcopy=auto` renames when it can, and Vim's help notes that rename-based saves confuse "file-watcher daemons like inotify" ([options.txt `'backupcopy'`](https://github.com/vim/vim/blob/master/runtime/doc/options.txt)). It also probes writability with a file named `4913` ([bufwrite.c](https://github.com/vim/vim/blob/master/src/bufwrite.c)).
- **Watch directories recursively, not individual files.** Rename-replace swaps the inode, so a watch on the file is lost.
- **Filter by allowlist, not denylist.** Accept only paths the Engine would load: Source format extensions under Collection directories, templates, styles, config and assets. Also honour `.gitignore`/`.ignore` through the `ignore` crate (0.4.33), and always exclude the output directory and `.git`. Zola's denylist needed per-editor patches. An allowlist plus the hash check makes `.#x`, `#x#`, `x~` and `4913` no-ops for free.
- **Linux limits.** Recursive inotify watches count toward `fs.inotify.max_user_watches` (notify crate docs, "Known problems": [lib.rs](https://github.com/notify-rs/notify/blob/5ebf4c0ba4e51ffae0a1d73476f9de9f505a82d8/notify/src/lib.rs)). Offer a `--poll` fallback, as Hugo does.

### 5. Dev server and live reload

- **Process shape.** One `yuanme dev` process runs three parts:
  - a tokio runtime for HTTP and WebSocket (axum 0.8.9);
  - a dedicated build thread that owns the caches;
  - the rayon pool.

  Builds never run on tokio worker threads. ADR 0001 (no runtime server) is about what is deployed, and this server is local build-time tooling. It does not conflict with ADR 0001, and that boundary is worth stating in the dev docs.
- **Output.** Serve from an in-memory `Arc` snapshot of `url → bytes`, swapped atomically after each build. This is Zola's approach, and it means no torn half-written files are ever served. Never write dev output into `dist/`, because dev includes drafts (draft visibility differs between dev and prod, per the parity inventory §visibility) and the injected client. Offer `--write <dir>` for agents that want files on disk.
- **Transport.** Use a WebSocket at `/__yuanme/ws`.
  - SSE over plain-HTTP localhost is capped at 6 connections per browser per origin, shared with page loads, and browsers won't fix it ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/EventSource)). A seventh tab of the dev Site would stall.
  - The WebSocket is also bidirectional. The client reports the URL it is showing, so the server can render that page's HTML first and send it inline. This is Hugo's "visited URLs" idea and Eleventy's inline content.
- **Message.** Use one JSON message per build: `{ build, ms, changed: [{ url, kind: "html"|"css"|"asset", html? }], moved: [{ from, to }], diagnostics: [...] }`. `html` is included only for the URLs that connected clients are viewing.
- **Client behaviour.** The client is a hand-written dev-only script injected before `</body>` (target ≤ 3 KB):
  - **CSS only.** For each changed stylesheet, clone its `<link>` with a cache-busting query, wait for `load`, then remove the old one. This is livereload-js's no-flash pattern ([reloader.js L462–512](https://github.com/livereload/livereload-js/blob/e23903f7b5827ce31906e985a88fb80303270a90/src/reloader.js#L462-L512)). There is no reload, and scroll and canvas state survive.
  - **The current page's HTML changed.** Morph the DOM from the inline HTML, following Eleventy's rules. Never replace `<canvas>` or `[data-yuanme-preserve]`, skip the focused input, and reload if a `<script>` was added or changed. Then dispatch `yuanme:patched` so page scripts can rebind.
    - For the Reference site this keeps the Film's WebGL context and the panel's scroll position. A full reload would restart the shader and replay the idle/expanded intro.
    - A vendored morph library (morphdom or idiomorph, both plain JS) ships inside the binary, so nothing needs Node (ADR 0003).
  - **The current page moved** (slug change). Navigate to the new URL, as Eleventy's `redirects` and Hugo's `--navigateToChanged` do.
  - **Another page changed.** Do nothing.
  - **Build error.** Show an overlay with the structured diagnostics and keep the last good DOM. Clear it on the next good build.
  - **Fallback.** Use `location.reload()` for anything else, after saving the panel's `scrollTop` in `sessionStorage`. In the Reference site the panel scrolls, not the window.
- **Agent surface.** `yuanme dev --json` prints one NDJSON event per build to stdout: start, done with stage timings, changed URLs and diagnostics. An agent can then wait for "build N done" instead of sleeping. Add a `GET /__yuanme/status` JSON endpoint as well. This feeds the v1 agent-native surface (#21).

### 6. Parallelism (rayon)

- **Version.** rayon 1.12.0 (2026-04-14).
- **Measured.** Cold highlighting of 30 copies of the largest Post took 323 ms sequentially and 46 ms with rayon. 1,000 copies took 17.9 s sequentially and 3.0 s with rayon, a 5–7× speedup on 16 threads. Dispatching a trivial 4-item `par_iter` costs 12–19 µs, so using rayon even on the single-edit path costs nothing.
- **Use it for:**
  - the initial parse of all Source files;
  - leaf misses (cold highlight, math);
  - stage 5, rendering every page.
- **Image encoding.** Run it on a separate low-priority pool, or as a background job whose completion triggers another build. An image miss must never block the HTML path. Image work is the slowest cold item today: 72 sharp transforms take 1.54 s in the Astro baseline (parity inventory §1).

## Appendix: spikes

The throwaway code lived in `/private/tmp/yuanme-spike-incr` and is not committed. It pinned notify 8.2.0, pulldown-cmark 0.13.4, syntect 5.3.0 (`default-fancy`), minijinja 2.24.0, blake3 1.8.7, xxhash-rust 0.8.19, rayon 1.12.0 and tungstenite 0.30.

- **`render`.**
  - **Input.** `eyuan.me/src/content/blog/extending_c_stdlib.md` (12,778 B, 29 fenced code blocks).
  - **Per-Post measurements.** Splits frontmatter, renders with pulldown-cmark and highlights fenced blocks with syntect `ClassedHTMLGenerator`, then renders a minijinja layout and writes the file. Each stage takes the median of 200 runs.
  - **Prose-edit cache test.** Re-renders a prose-edited copy with a code-block cache keyed by xxh3 of `(lang, code)`.
  - **Scale tests.** Renders N = 30/300/1,000 distinct copies, both cold (highlight everything) and warm (cache hits): template, output xxh3, compared with the previous hashes. Sequential and rayon timings come from the same corpus.
- **`watch`.** A `notify::recommended_watcher` on a temp directory. Each of five save styles ran 15 times, recording time from the end of the save to the first event, the event list, and the time to the last event.
- **`wsprobe`.** A LiveReload-protocol WebSocket client that edits a Post in place, then measures the time until a `reload` command arrives. It ran against `zola serve` 0.23.6 and `hugo server` 0.167.0 (official darwin release binaries) on the same generated 100-Post site, with highlighting on in both, and waited 2.5 s between edits.
- **Staleness check.** Edited a Post's title, then used `curl` to fetch the page and its listings 1–3 s later.

## Open questions

- **Browser half of the budget (unmeasured).** How long do a morph and a full reload take on the Reference site, including Film WebGL re-init, font revalidation and the panel intro? This needs a real browser, so it belongs with the Shell router prototype (#26). It decides whether morphing is required or merely nice.
- **Does the Shell router expose a dev "soft swap" hook?** The alternative is a generic morph. Reusing the router's swap keeps behaviour identical to navigation (#26).
- **CSS rebuild cost.** When an edit introduces new utility classes, how incremental can Tailwind-without-Node be (#7)? The budget above assumes CSS is untouched by a prose edit.
- **Math and highlighter leaf costs.** The math renderer (#4) and highlighter (#5) choices change the cold cost and the shape of the memo key. syntect `default-fancy` cost 0.4–1.7 ms per block here, and a faster highlighter shrinks cold starts.
- **Template engine hooks.** The template engine (#6) must offer access-tracked context objects and a loader hook if tier 2 is ever needed.
- **Persisting leaf caches.** Should leaf caches persist on disk (`.yuanme/cache`, content-addressed, salted by Engine version) so cold `yuanme dev`, `yuanme build` and CI start warm? What about eviction?
- **Linux and Windows watcher latency.** Only macOS FSEvents was measured.
- **Budget numbers.** The exact budget numbers (for example ≤ 50 ms server-side p95 at 30 pages and ≤ 100 ms at 1,000) belong to #15, along with how CI measures them.
