# What must the Engine emit to be portable and fast on any static host?

Ticket: [#10](https://github.com/yuann3/yuanme/issues/10). Researched 2026-10-04. Feeds [#25](https://github.com/yuann3/yuanme/issues/25) (deploy portability) and [#15](https://github.com/yuann3/yuanme/issues/15) (performance budgets).

Hosts compared: Netlify, Cloudflare Pages, Cloudflare Workers static assets, Vercel, GitHub Pages, Bunny CDN (Storage zone + Pull zone). Sources are official docs, host source code (Cloudflare's asset servers are open source in `cloudflare/workers-sdk`), staff answers on official forums, and live probes run from Singapore with `curl` and from 10 regions with the [Globalping](https://globalping.io) API on 2026-10-04.

## Short answer

1. **Emit a directory layout and link to it with trailing slashes.** Write every page as `x/index.html` and make every internal href the trailing-slash form `/x/`. That is the only URL shape that all six targets serve with a 200 and no redirect and no config (table below). The Reference site's slashless nav hrefs (`/about`, `/projects`, ...) currently cost one 301 per click, measured at 280 to 540 ms from Singapore, because the redirect itself goes uncached through Cloudflare to Netlify's origin. Fix it as a Parity exception.
2. **Write one 404 page at `/404.html`.** Netlify, Pages, Vercel and GitHub Pages pick it up on their own. Workers needs `not_found_handling = "404-page"`, and Bunny needs the storage zone's "404 File path" set.
3. **Put content-hashed assets under one prefix with no leading underscore** (for example `/assets/`), and give that prefix `Cache-Control: public, max-age=31536000, immutable`. None of the hosts does this by default for a generic static site. Leave HTML, feeds and non-hashed public files on the host default (`public, max-age=0, must-revalidate` on Netlify, Cloudflare and Vercel).
4. **Don't emit precompressed `.br`/`.gz` files by default.** None of the five hosts serves them: each one compresses on the fly (Cloudflare and Bunny also use zstd). Host brotli comes out 9 to 15% larger than `brotli -q 11` on our files, which is 0.4 to 0.9 KB per file. Offer it only as an opt-in for self-hosting behind Caddy or nginx.
5. **Keep one host-neutral routing model inside the Engine** (header rules, redirects, the 404 page, the immutable prefix) **and render it through small per-host adapters**: `_headers` + `_redirects` (one renderer covers both Netlify and Cloudflare), `wrangler.jsonc`, `vercel.json`, `.nojekyll` plus meta-refresh stubs for GitHub Pages, and API calls for Bunny. Use meta-refresh stub pages as the universal fallback for redirects.
6. **Percent-encode generated path segments, or use ASCII slugs.** Workers static assets answers 307 to any path whose `encodeURIComponent` form differs from the request, so `/tags/c++/` redirects to `/tags/c%2B%2B/`.
7. **Speed:** Cloudflare Workers static assets had the best TTFB on a cold request (median 74 ms across 10 regions) and the second-best when warm (median 50 ms; GitHub Pages was 3 ms). It also has HTTP/3, zstd, and free, unlimited static requests. The Reference site's current setup (Cloudflare proxy in front of Netlify) was the slowest of everything measured (warm median 371 ms, worst 769 ms), because Cloudflare doesn't cache the HTML and Netlify's edge kept missing. Make Workers the first-class adapter, keep Netlify as a supported adapter, and recommend that the Reference site move to Workers, or at least stop proxying Netlify through Cloudflare.

## Comparison

| | Netlify | Cloudflare Pages | Cloudflare Workers assets | Vercel | GitHub Pages | Bunny (Storage + Pull zone) |
|---|---|---|---|---|---|---|
| Header config | `_headers` file or `netlify.toml` `[[headers]]` | `_headers` (100 rules, 2,000 chars/line) | `_headers` (same format and limits) | `vercel.json` `headers` | **none** | Edge Rules (dashboard/API), no file |
| Redirect config | `_redirects` or `netlify.toml` `[[redirects]]`, default 301 | `_redirects` (2,000 static + 100 dynamic), default 302 | `_redirects` (same), default 302 | `vercel.json` `redirects` (2,048 max), 307/308 | **none** (meta refresh only) | Edge Rules (dashboard/API), no file |
| `/x/` → `x/index.html` | 200 | 200 | 200 | 200 | 200 | 200 (observed) |
| `/x` with `x/index.html` | **301** → `/x/` | **308** → `/x/` | **307** → `/x/` (default `auto-trailing-slash`) | 200 by default (duplicate URL); 308 with `trailingSlash: true` | **301** → `/x/` | 200 (observed; duplicate URL) |
| `/x` with flat `x.html` | Pretty URLs feature (default on) | 200; `/x.html` → 308 `/x` | 200; `/x.html` → 307 `/x` | **404** unless `cleanUrls: true` | 200; `/x.html` also 200 | **404** (no extension mapping) |
| 404 page | `/404.html` automatically | nearest `404.html` automatically | only with `not_found_handling: "404-page"` (default `none`) | `/404.html` automatically | `/404.html` automatically | set "404 File path" in storage zone |
| Precompressed `.br` served | No | No | No | No | No | No |
| On-the-fly encodings | br, gzip | br, gzip (served br to a zstd-capable request) | zstd, br, gzip | br, gzip | **gzip only** | zstd, br, gzip |
| HTTP/3 | **No** | Yes | Yes | **No** (one Vercel-owned domain advertises it) | **No** | **No** (not observed) |
| Default browser cache | `max-age=0, must-revalidate` | `max-age=0, must-revalidate` + ETag | `max-age=0, must-revalidate` + ETag | `max-age=0, must-revalidate` | `max-age=600`, cannot change | whatever the zone/Edge Rules set |
| Immutable hashed assets | via `_headers` | via `_headers` | via `_headers` | via `headers`, or the Build Output API `/_vercel/immutable/` | **impossible** | via Edge Rule |
| Edge cache invalidated on deploy | Yes (atomic) | Yes | Yes | Yes | Yes (Fastly, 600 s TTL) | **No**: purge via API |
| TTFB median, cold (1st) request | 220 ms | 142 ms | **74 ms** | 299 ms | 124 ms | 64–139 ms (depends on zone) |
| TTFB median, warm (2nd) request | 238 ms | 38 ms | 50 ms | 196 ms | **3 ms** | 113 ms (one zone) |

Sources for each cell are in the details below. The TTFB rows come from [Measurements](#ttfb-worldwide); read the caveats there before you compare hosts.

## Details

### URL shape: why directory layout plus trailing-slash hrefs

- **Netlify.** Pretty URLs is on by default. It "forward[s] paths like `/about` to `/about/`", and you "cannot use a redirect rule to add or remove a trailing slash" ([Netlify redirect options](https://docs.netlify.com/manage/routing/redirects/redirect-options/)). Live: `eyuan.netlify.app/about` → `301 location: /about/`, and `/about/` → 200 (curl, 2026-10-04).
- **Cloudflare Pages.** In the asset-server source, a slashless path is served from `${pathname}.html` when that file exists. Otherwise, if `${pathname}/index.html` exists, the server redirects to `${pathname}/` ([`pages-shared/asset-server/handler.ts`](https://github.com/cloudflare/workers-sdk/blob/main/packages/pages-shared/asset-server/handler.ts)). The docs say `/contact.html` redirects to `/contact` and `/about/index.html` to `/about/` ([Serving Pages](https://developers.cloudflare.com/pages/configuration/serving-pages/)). Live: `what-the-loop-spec.pages.dev/index.html` → `308 location: /`.
- **Cloudflare Workers static assets.** `html_handling` defaults to `auto-trailing-slash` ([`asset-worker/src/configuration.ts`](https://github.com/cloudflare/workers-sdk/blob/main/packages/workers-shared/asset-worker/src/configuration.ts)). In that mode `/about` gets a 307 to `/about/` for `about/index.html`, and `/about` is served directly for `about.html`. `force-trailing-slash`, `drop-trailing-slash` and `none` are the other modes ([HTML handling](https://developers.cloudflare.com/workers/static-assets/routing/advanced/html-handling/)). Live: `developers.cloudflare.com/workers` → `307 location: /workers/`.
- **Workers path canonicalisation.** The asset worker decodes the path, re-encodes each segment with `encodeURIComponent`, and answers 307 when the result differs from the request ("/[boop] -> /%5Bboop%5D 307"; [`asset-worker/src/handler.ts`](https://github.com/cloudflare/workers-sdk/blob/main/packages/workers-shared/asset-worker/src/handler.ts), `encodePath` and the redirect block above it). The parity inventory keeps raw tag directories such as `/tags/c++/`. Browsers send `+` as is, so on Workers every such link would cost a 307.
- **Vercel.** `cleanUrls` defaults to `false`. When it is `true`, `about.html` is served at `/about` and `/about.html` gets a 308. `trailingSlash` defaults to `undefined`, which serves `/about` and `/about/` both "without redirecting". `true` gives a 308 to the slash form for paths with no extension ([vercel.json reference](https://vercel.com/docs/project-configuration/vercel-json)).
- **GitHub Pages.** Live probe of `rust-lang.github.io/rfcs`: `/rfcs` → `301` to `/rfcs/`. `/rfcs/0001-private-fields` and `/rfcs/0001-private-fields.html` both return 200 (a duplicate URL). `/rfcs/0001-private-fields/` returns 404.
- **Bunny.** The docs only cover SPA fallback: "Rewrite 404 to 200" plus a "404 File path" ([storage settings](https://bunny.net/docs/storage/settings.md), [static site hosting](https://bunny.net/docs/storage/static-site-hosting)). They don't document directory indexes. Live probes of three independent Bunny-served static sites (`kenan.fyi/about/`, `benjcal.space/about/`, `nzg.utf9k.net/sites/`) returned 200 for both `/x` and `/x/`, with no redirect.

So `x/index.html` linked as `/x/` is the one shape that every target serves with no redirect and no config. A flat `x.html` linked as `/x` breaks on Bunny and on Vercel without `cleanUrls`. Linking slashless to a directory layout, which is what the Reference site does today, redirects on Netlify, Pages, Workers and GitHub Pages.

**The measured cost of today's 301.** From Singapore, `eyuan.me/about` took 278 ms, 487 ms and 541 ms to return its 301, against 59 to 72 ms for a warm `/about/` (one 302 ms outlier). `curl -L` on `/about` totalled 506 ms. The redirect comes back with `cf-cache-status: DYNAMIC` and an `x-nf-request-id`, so it goes to Netlify on every click. The parity inventory already notes that the router then pushes `/x/` and sets `aria-current="true"` instead of `page`.

**A related redirect.** Canonical, RSS and sitemap URLs use `https://www.eyuan.me`, which 301s to the apex (`www.eyuan.me/` → `location: https://eyuan.me/`, curl). Making the configured Site origin the served origin removes that redirect too. This is open question 3 in the parity inventory.

### Precompressed files and compression

- **Netlify** doesn't serve precompressed files. Staff (2022): "our CDN already automatically compresses your text assets using brotli or gzip (not, I understand, quite as thoroughly as you can yourself), it's very likely not going to become a high priority" ([forum](https://answers.netlify.com/t/serving-pre-compressed-brotli-files/53515)). `Content-Encoding` is on the list of headers that `_headers` cannot set ([Custom headers](https://docs.netlify.com/manage/routing/headers/)), so the files can't be faked through headers either.
- **Cloudflare.** Pages "will also serve Gzip and Brotli responses whenever possible" ([Serving Pages](https://developers.cloudflare.com/pages/configuration/serving-pages/)). For the zone, "Free Plan: Content is compressed by default using Zstandard" ([Compression](https://developers.cloudflare.com/speed/optimization/content/compression/)). The Workers assets docs don't mention precompressed variants ([Headers](https://developers.cloudflare.com/workers/static-assets/headers/)). Live: a Workers-served blog returns `content-encoding: zstd`.
- **Vercel** compresses with gzip and brotli, for an allowlist of MIME types ([Compression](https://vercel.com/docs/how-vercel-cdn-works/compression)). A 2024 community report that precompressed `.br` files are not served got no fix from staff ([thread](https://community.vercel.com/t/vercel-not-serving-pre-compressed-assets-brotli/1639)).
- **GitHub Pages** returned `content-encoding: gzip` even when the request offered `br` and `zstd` (live probe).
- **Bunny** "automatically compresses content using gzip, Brotli (br), or Zstandard (zstd)" ([MIME compression](https://bunny.net/docs/cdn/frequently-asked-questions/mime-compression.md)). The docs don't mention precompressed files.
- **What it would gain.** Netlify served `/` as 3,135 bytes of br; `brotli -q 11` on the same body gives 2,729. For `Shell…C3Ek_E9W.js` it served 10,682 bytes against 9,796 at q11 (parity inventory). That is 9 to 15% (0.4 to 0.9 KB per file), and no host would serve the precompressed files anyway.

### HTTP/3

- **Netlify:** "No, HTTP2 is as far as we go for now" (staff, 2024-09-28, [forum](https://answers.netlify.com/t/how-to-set-the-http3/126332)). On 2026-10-04, `eyuan.netlify.app`, `www.netlify.com` and `docs.netlify.com` sent no `alt-svc` header.
- **Cloudflare (Pages and Workers):** `alt-svc: h3=":443"; ma=86400` on `*.pages.dev`, on Workers-served sites, and on `eyuan.me` today. On `eyuan.me` it comes from the Cloudflare proxy, not from Netlify.
- **Vercel:** No `alt-svc` on `vercel.com` or on three `*.vercel.app` static sites. `nextjs.org` alone sends `h3=":443"; ma=300`. The community thread has no staff statement ([thread](https://community.vercel.com/t/http-3-support/41935)). Treat it as not generally available.
- **GitHub Pages:** No `alt-svc` (live probe).
- **Bunny:** No `alt-svc` on `bunny.net`, `kenan.fyi` or `benjcal.space`. The [CDN features page](https://bunny.net/cdn/features/) and the [SSL docs](https://bunny.net/docs/cdn/security/ssl.md) mention TLS 1.2/1.3 but not HTTP/3.

### Caching and immutable assets

- **Netlify** defaults static files to `Cache-Control: public, max-age=0, must-revalidate` plus `Netlify-CDN-Cache-Control: public, s-maxage=31536000, must-revalidate`, and "all new deploys invalidate the cache" ([Caching overview](https://docs.netlify.com/build/caching/caching-overview/)). Live: hashed `_astro/*.js` comes back from the Netlify origin with `max-age=0`.
- **The Reference site today.** On `eyuan.me` the same file arrives as `max-age=14400`. That is Cloudflare's default Browser Cache TTL of 4 hours, which overrides origin values that are lower ([Browser Cache TTL](https://developers.cloudflare.com/cache/how-to/edge-browser-cache-ttl/)). It is not an immutable policy: non-hashed public files get the same 4-hour TTL, so they can be up to 4 hours stale after a deploy.
- **Cloudflare Pages and Workers** both default to `public, max-age=0, must-revalidate` plus a strong ETag ([Serving Pages](https://developers.cloudflare.com/pages/configuration/serving-pages/); [Workers headers](https://developers.cloudflare.com/workers/static-assets/headers/); the `CACHE_CONTROL_BROWSER` constant in [`utils/headers.ts`](https://github.com/cloudflare/workers-sdk/blob/main/packages/workers-shared/asset-worker/src/utils/headers.ts)). Pages documents content-hashed CSS/JS as the case where custom caching "does make sense". Workers assets are cached at the nearest location on first request, behind tiered cache ([Workers static assets](https://developers.cloudflare.com/workers/static-assets/)), and static asset requests are "free and unlimited" ([billing](https://developers.cloudflare.com/workers/static-assets/billing-and-limitations/)).
- **Vercel** caches static files at the edge "for the lifetime of the deployment". The browser gets `max-age=0, must-revalidate` (live). Files emitted under `/_vercel/immutable/` with an `immutable.json` manifest are served as `public, max-age=31536000, immutable` and shared across deployments ([CDN cache](https://vercel.com/docs/caching/cdn-cache); [Build Output API primitives](https://vercel.com/docs/build-output-api/primitives)). A `headers` rule in `vercel.json` is the simpler route.
- **GitHub Pages** sends `cache-control: max-age=600` on everything, and nothing can change it (live probe; the [limits page](https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits) offers no header config).
- **Bunny** "does not monitor your origin for file changes ... purge the cache" ([Purge Cache](https://bunny.net/docs/cdn/purge-cache.md)). Headers and cache times are set through Edge Rules such as "Set Response Header" and "Override Cache Time" ([Edge Rules](https://bunny.net/docs/cdn/edge-rules)). Its static-hosting guide recommends a long TTL for hashed JS/CSS and a short one for HTML. Live Bunny sites varied from `max-age=180` to `max-age=2592000` on HTML, so the defaults depend on each Author's configuration.

### Config formats and limits

- **Netlify:** `_headers` and `_redirects` in the publish directory, or `[[headers]]`/`[[redirects]]` in `netlify.toml`. The two formats are equivalent ([headers](https://docs.netlify.com/manage/routing/headers/), [redirects](https://docs.netlify.com/manage/routing/redirects/redirect-options/)). `netlify deploy --dir dist`, which the Reference site's CI uses, picks up files in `dist`.
- **Cloudflare:** The `_headers` format is "the URL or URL pattern" followed by indented `Name: value` lines, with up to 100 rules and 2,000 characters per line ([Pages headers](https://developers.cloudflare.com/pages/configuration/headers/)). `_redirects` lines are `[source] [destination] [code?]`, with codes 301/302/303/307/308, a default of 302, and limits of 2,000 static plus 100 dynamic rules ([Pages redirects](https://developers.cloudflare.com/pages/configuration/redirects/), [Workers redirects](https://developers.cloudflare.com/workers/static-assets/redirects/)). Workers supports both files natively ([migration guide](https://developers.cloudflare.com/workers/static-assets/migration-guides/migrate-from-pages/)). Workers limits are 20,000 files (free) or 100,000 (paid) and 25 MiB per file ([limits](https://developers.cloudflare.com/workers/platform/limits/)). Cloudflare now says: "Start new projects with Workers" ([Pages docs](https://developers.cloudflare.com/pages/)).
- **Netlify and Cloudflare share a syntax.** Both `_headers` files are a path line followed by indented headers, and both `_redirects` files are `from to [code]` with splats and `:placeholders`. One renderer can target both if it keeps to the common subset (Cloudflare's 100-rule and 2,100-redirect caps; no Netlify-only conditions).
- **Vercel:** `vercel.json` takes `headers: [{source, headers:[{key,value}]}]`, `redirects` (at most 2,048), `cleanUrls` and `trailingSlash` ([vercel.json](https://vercel.com/docs/project-configuration/vercel-json), [configuration redirects](https://vercel.com/docs/routing/redirects/configuration-redirects)). A prebuilt deploy can use the Build Output API: `.vercel/output/static` plus `config.json` ([configuration](https://vercel.com/docs/build-output-api/configuration)).
- **GitHub Pages:** no header or redirect config. Deploy a prebuilt artifact with `actions/upload-pages-artifact@v4` and `actions/deploy-pages@v4`; the tar must be under 10 GB and contain no symlinks or hard links ([custom workflows](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages)). The custom 404 is `404.html` ([docs](https://docs.github.com/en/pages/getting-started-with-github-pages/creating-a-custom-404-page-for-your-github-pages-site)). Sites are limited to 1 GB with a 100 GB/month soft bandwidth limit, and commercial use is not allowed ([limits](https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits)). A branch deploy runs Jekyll, which drops `_`-prefixed folders unless `.nojekyll` is present. That is why the asset prefix should not start with `_`, and why the adapter writes `.nojekyll` anyway.
- **Bunny:** no file-based config. You upload through the Storage HTTP API (`PUT https://storage.bunnycdn.com/<zone>/<path>`, [Vite guide](https://bunny.net/docs/storage/static-site-hosting/vite.md)), set Edge Rules in the dashboard or API, and purge through the API. Storage doesn't support renames ([limits](https://bunny.net/docs/storage/limits.md)). The minimum charge is $1/month ([FAQ](https://bunny.net/faq/)).

### TTFB worldwide

**Method.** Globalping HTTP GET over HTTPS with `Accept-Encoding: br, gzip`. Each host got one probe in each of Frankfurt, London, Virginia Beach, Los Angeles, São Paulo, Johannesburg, Mumbai, Singapore, Tokyo and Sydney, then a second request from the same probes about 2 s later. The numbers are Globalping's `firstByte` timing in ms (time from request sent to first byte, after TLS). Measured 2026-10-04 around 12:10 UTC. Globalping measurement IDs (1st/2nd request): Netlify `2nAAxeqDcWQajECei00021Fqk`/`2BvCTzWH1lefwhPJp00021Fqk`; Netlify behind Cloudflare `2ARhnuj53VyH7eS1c00021Fqk`/`2WWH2nzacgPubNdZI00021Fqk`; Pages `2U1ERDBpqrR2OWgV500021Fqk`/`2rcRMF3KqIGYmmbay00021Fqk`; Workers `2Rpu0PgM6QFR14RKt00021Fqk`/`2RF6TORvmm6Dz1Y5q00021Fql`; Vercel `27wgLq2xpyAKeeasV00021Fql`/`2DFZjcLoczEDpJ12h00021Fql`; GitHub Pages `2ir9dT79PTMjoVU0Z00021Fql`/`2bvKO2xiaPYfAXTHQ00021Fql`; Bunny `29p3sHo0HmSps9irH00021Fql`/`2UVCkbv07IhODk57600021Fql`; `kenan.fyi` `2BoJnF8yi7Mu73BaA00021Fql`.

**Hosts measured.** These are different real sites, chosen to be low traffic like a personal site:
- Netlify: `eyuan.netlify.app/about/`
- Netlify behind the Cloudflare proxy: `eyuan.me/about/`
- Cloudflare Pages: `what-the-loop-spec.pages.dev/`
- Workers: `www.brycewray.com/posts/`, a personal Hugo blog
- Vercel: `garden-ten-snowy.vercel.app/`
- GitHub Pages: `rust-lang.github.io/rfcs/`
- Bunny: `benjcal.space/about/`, plus `kenan.fyi/about/` for the first request only (a rate limit stopped the second run)

| Host | 1st: median / max | 2nd: median / max | Notes |
|---|---|---|---|
| Netlify | 220 / 416 | 238 / 432 | 9 of 10 second requests were still `fwd=miss` |
| Netlify behind Cloudflare (Reference site today) | 290 / 769 | 371 / 766 | HTML not cached by Cloudflare; Netlify edge mostly missing |
| Cloudflare Pages | 142 / 765 | 38 / 373 | |
| Cloudflare Workers assets | **74** / 325 | 50 / 191 | `cf-cache-status: HIT` throughout |
| Vercel | 299 / 573 | 196 / 358 | every second request was a HIT; the cache is per region ([CDN cache](https://vercel.com/docs/caching/cdn-cache)) |
| GitHub Pages | 124 / 231 | **3** / 72 | Fastly; a 600 s TTL makes it re-fetch often |
| Bunny `benjcal.space` | 139 / 317 | 113 / 277 | TCP RTT of 70 to 270 ms outside the US means this zone serves from few PoPs |
| Bunny `kenan.fyi` (1st only) | 64 / 10,077 | n/a | mostly local PoPs; one 10 s storage miss in Tokyo |

**Caveats.** The payloads differ, there is one sample per probe and run, and the Globalping probes sit in data centres. Bunny's numbers depend on the Author's pull-zone tier, routing filters and storage replication, which are per-zone settings ([changelog: EEA routing filter](https://bunny.net/docs/changelog), [replication](https://bunny.net/docs/storage/replication.md)). The robust conclusions:
- On a low-traffic site, Cloudflare (Workers or Pages) answers in about 30 to 60 ms in most regions once a single request has warmed the location.
- Netlify kept going back to its origin on repeat requests.
- Putting Cloudflare in front of Netlify made things slower, not faster.

## Recommended output layout

```
dist/
  index.html
  about/index.html            # every page is <path>/index.html; hrefs are "/about/"
  blog/<slug>/index.html
  blog/2/index.html
  tags/<encoded-or-slug>/index.html
  404.html                    # exactly one 404 page at the root
  assets/<name>.<hash>.<ext>  # all content-hashed CSS/JS/images; nothing else lives here
  <public passthrough files>  # unhashed, original paths (PDF, PNGs, favicon)
  rss.xml  sitemap-index.xml  sitemap-0.xml  llms.txt
  # adapter outputs (only the one selected):
  _headers  _redirects        # netlify, cloudflare
  .nojekyll                   # github-pages
```

Engine rules:
- **Canonical URL form.** Every generated internal href, canonical and sitemap URL is either `/` or ends in `/` (except files with extensions). Every path segment is ASCII-slugged or `encodeURIComponent`-encoded. The build fails on output path collisions, which the parity inventory already requires.
- **One routing model.** The Engine holds a host-neutral `Routes` model: header rules (glob → headers), redirects (from → to, 301/302/307/308), the 404 page, and the immutable prefix. Adapters render it, and the Engine checks it against the strictest target's limits (100 header rules, 2,000 static redirects).
- **Default header rules.** `/assets/*` gets `Cache-Control: public, max-age=31536000, immutable`. Nothing else is set, so HTML and feeds stay on the hosts' revalidate-always defaults.
- **Redirect fallback.** Every redirect is also written as a meta-refresh stub page (`<meta http-equiv="refresh" content="0; url=…">` plus `<link rel="canonical">`) on hosts that have no redirect config.
- **No precompressed files by default.** `--precompress` writes `.br`/`.gz` siblings for self-hosting (Caddy `file_server precompressed`, nginx `brotli_static`).

## Per-host adapters

| Adapter | Emits | Deploy step (scaffolded by `init`) |
|---|---|---|
| `netlify` | `_headers`, `_redirects` in `dist/` | `netlify deploy --dir dist --prod` (unchanged from today) |
| `cloudflare` (Workers, the default) | `_headers`, `_redirects`; `wrangler.jsonc` with `assets.directory = "dist"`, `html_handling = "auto-trailing-slash"`, `not_found_handling = "404-page"` | `wrangler deploy` |
| `cloudflare-pages` (legacy) | `_headers`, `_redirects` | `wrangler pages deploy dist` |
| `vercel` | `vercel.json` with `trailingSlash: true`, `cleanUrls: false`, `headers`, `redirects` (or `.vercel/output/config.json` for `--prebuilt`) | `vercel deploy --prebuilt` or a Git integration |
| `github-pages` | `.nojekyll`, meta-refresh stubs for redirects, optional `CNAME`; warns that immutable caching, custom headers, br and HTTP/3 are unavailable | `actions/upload-pages-artifact@v4` + `actions/deploy-pages@v4` |
| `bunny` | meta-refresh stubs; no config files | sync to Storage (upload changed, delete removed) → ensure Edge Rules (immutable `/assets/*`, short TTL for HTML) and the zone's 404 path through the API → full pull-zone purge |
| `static` (generic or self-host) | nothing extra, plus optional `--precompress` | none |

## Open questions

1. **Host for the Reference site.** Should it move from Netlify (behind the Cloudflare proxy) to Cloudflare Workers static assets, given that DNS is already on Cloudflare? Or should it stay on Netlify and at least switch the Cloudflare proxy off? This is #25's call. It also changes the deploy step in the parity inventory, section 2.5.
2. **Parity exceptions to confirm.** Trailing-slash nav and project-row hrefs (parity inventory Q2), the Site origin moving to the apex (Q3), adding `_headers` for immutable assets (Q5), and renaming `_astro/` to `/assets/`.
3. **Tag and series path policy.** Should the Engine percent-encode the raw names (`/tags/c%2B%2B/`) or slug them (`/tags/cpp/`)? Netlify, Vercel and Bunny serving `%2B`-encoded paths from a directory literally named `c++` was not verified. A throwaway deploy should check it before #25 decides.
4. **Bunny directory indexes** were observed on three live sites but are undocumented. They need a confirmation from Bunny or a test zone before Bunny is a supported adapter.
5. **Repeat measurement.** Should TTFB be re-measured with the same payload deployed to each host (a matrix spike), so #15's budgets don't rest on different third-party sites? Globalping's anonymous rate limit stopped the third and fourth repeats here.
6. **Vercel immutable assets.** Is using `/_vercel/immutable/` plus `immutable.json`, for cross-deploy asset reuse, worth a Vercel-specific asset path, or is a `headers` rule enough?
7. **Workers 307s.** Workers uses 307 (temporary) for its trailing-slash and encoding redirects. That only matters if a slashless or unencoded URL is ever linked from outside the site. Should the Engine also emit explicit 308s in `_redirects` for known legacy URLs?
