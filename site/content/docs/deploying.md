---
title: "Deploying"
description: "What yuanme build writes, the Host adapter for each static host, the cache policy, headers, redirects and the 404 Page."
---

`yuanme build` writes the Site as static files, plus the configuration one static host reads, such as `_headers` and `_redirects`. The Host adapter, chosen in Site config, decides which host's configuration it writes. You upload the output directory with the host's own tool; nothing of yuanme runs on the host.

## What `yuanme build` writes

The output directory is `dist/`, or `.vercel/output/` with the `vercel` adapter. `--out DIR` names another, from the project root.

- Each Page at its Route's file: `/blog/hello/` is `blog/hello/index.html`.
- Stylesheets, Scripts and images, hashed, in `assets/`, such as `assets/site-7d4ca780d77b89b7.js`.
- Each Static file at its path from `static/`, unchanged and unhashed.
- `sitemap-index.xml` and the files it lists, and each feed.
- The Host adapter's files.

A build writes only the files that changed, and deletes the files the previous build wrote that this one doesn't. Files yuanme didn't write stay where they are, and the build names them. `yuanme build --clean` empties the output directory first. A build with errors writes nothing and leaves the output directory as it was.

yuanme remembers what it wrote in the last output directory it built into. After you change the adapter or `--out`, build once with `--clean`, so files the old adapter wrote, such as `_headers`, don't stay behind.

`yuanme check` reports the same Diagnostics as `build`, the host's included, and writes nothing.

## Host adapters

`[host] adapter` names the host. It defaults to `cloudflare`:

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[host]
adapter = "cloudflare"
```

Every adapter renders the same rules: yuanme's cache policy, your `[[host.headers]]` and `[[host.redirects]]`, and the 404 Page.

### `cloudflare`

For Cloudflare Workers with static assets, the default. It writes `_headers`, and `_redirects` when Site config has redirects.

The Worker reads `wrangler.jsonc` at the project root. When that file exists, `build` and `check` compare it with what the adapter writes and report `host/wrangler-mismatch`:

- `assets.directory` must be the output directory, `./dist`.
- `assets.html_handling`, if set, must be `"auto-trailing-slash"`, the default, since every Page's URL ends in `/`.
- `assets.not_found_handling` must be `"404-page"` when the Site has a 404 Page.

```jsonc
{
  "name": "example",
  "compatibility_date": "2026-10-01",
  "assets": {
    "directory": "./dist",
    "not_found_handling": "404-page"
  }
}
```

Deploy with `wrangler deploy`. yuanme reads only `wrangler.jsonc`, not `wrangler.toml` or `wrangler.json`.

A Static file can't take the name of a file the adapter writes. Write its rules in Site config instead:

```text fail host/output-collision static/_headers
/*
  X-Frame-Options: DENY
```

### `cloudflare-pages`

For Cloudflare Pages. It writes the same `_headers` and `_redirects` as `cloudflare`, and checks no `wrangler.jsonc`. Pages serves `404.html` itself.

### `netlify`

It writes `_headers` and `_redirects` in Netlify's form. Netlify serves `404.html` itself.

### `vercel`

For Vercel's Build Output API, which `vercel deploy --prebuilt` reads. The output directory is `.vercel/output/`: every file of the Site goes under `.vercel/output/static/`, and `.vercel/output/config.json` holds the headers, the redirects, the redirects to and from a trailing `/` that other hosts make themselves, and the 404 Page.

### `github-pages`

It writes `.nojekyll`, so Jekyll keeps every file, and a `CNAME` with the Site's domain unless `[site] url` is on `github.io`. GitHub Pages serves `404.html` itself.

GitHub Pages can't send headers, so hashed files aren't cached as immutable. The adapter always reports `host/unsupported` for that, and for each `[[host.headers]]` rule; with `--strict` the build fails:

```toml fail host/unsupported
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[host]
adapter = "github-pages"
```

A redirect becomes an HTML page that refreshes to its `to`: `from = "/old/"` writes `old/index.html`, and `from = "/old"` writes `old.html`. A redirect from a path with another extension, such as `/cv.pdf`, can't be made, and is reported as `host/unsupported`.

### `static`

For Caddy, nginx or any other server. It writes no host configuration, so each `[[host.headers]]` rule and `[[host.redirects]]` rule is reported as `host/unsupported`: the server must send them. It can precompress text files; see below.

## Cache policy

yuanme's own two header rules come before yours. With no rules of yours, the cloudflare adapter writes this `_headers`:

```text
/*
  Cache-Control: public, max-age=0, must-revalidate
  X-Content-Type-Options: nosniff

/assets/*
  ! Cache-Control
  Cache-Control: public, max-age=31536000, immutable
```

- A hashed file in `/assets/` is cached for a year. Its name changes whenever its bytes do, so a new deploy never serves an old one.
- Everything else revalidates on every request: Pages, Static files, feeds. A deploy shows at once, even behind a CDN.

A rule of yours that breaks this policy is reported as `host/cache-rule`: one that keeps a Page from revalidating, or a hashed file from being immutable. A Page may also say `no-cache` or `no-store`.

```toml fail host/cache-rule
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[[host.headers]]
path = "/*"
headers = { "Cache-Control" = "public, max-age=3600" }
```

Narrow the rule's `path` to the files it is for.

## Headers

Each `[[host.headers]]` rule sends headers for the URLs its `path` matches:

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[[host.headers]]
path = "/*"
headers = { "X-Frame-Options" = "DENY" }

[[host.headers]]
path = "/resume.pdf"
headers = { "Cache-Control" = "public, max-age=3600" }
```

- `path` is a URL path with at most one `*`, which matches any characters, `/` included: `/fonts/*`. It can't hold whitespace, `?`, `#` or `:`.
- A header name is letters, digits and `-`.
- Rules apply in order, yuanme's first. A rule with the path of an earlier one joins it. Where two rules set one header for a URL, the later one wins, on every host: here `/resume.pdf` is cached for an hour, and every URL gets `X-Frame-Options`.

## Redirects

Each `[[host.redirects]]` rule redirects one URL:

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[[host.redirects]]
from = "/posts/"
to = "/blog/"

[[host.redirects]]
from = "/talk"
to = "https://example.org/talk"
status = 302
```

- `from` is one URL path, with no `*`, and no two rules share it.
- `to` is a URL path from the Site's root or an absolute URL.
- `status` is 301, 302, 307 or 308. It defaults to 308, a permanent redirect that keeps the method.

A redirect from a URL the Site serves is an error, `host/redirect-conflict`, since hosts disagree about which wins. That includes a Page's URL without its trailing `/`, which every host already redirects:

```toml fail host/redirect-conflict
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[[host.redirects]]
from = "/blog"
to = "/blog/"
```

A `to` on the Site must lead to a file of the build, at the URL it is served from. One where nothing is is `link/broken`:

```toml fail link/broken
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[[host.redirects]]
from = "/posts/"
to = "/archive/"
```

And one that the host would redirect again, such as `/blog` for `/blog/`, is `link/redirected`:

```toml fail link/redirected
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[[host.redirects]]
from = "/posts/"
to = "/blog"
```

The same two codes check every link and every file a Page loads on the Site's own origin. A link to `/about` when the Page is at `/about/`, to `/about/index.html`, to a `from` of `[[host.redirects]]`, or to the Site's address with `http:` or with or without `www.`, costs a redirect, and is `link/redirected`. Link to the URL the help names. Links to other origins aren't checked.

## The 404 Page

A Route at `/404.html` is the Site's 404 Page, which the host serves, with status 404, for a URL where nothing is:

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[routes.missing]
path = "/404.html"
template = "404.html"
```

```jinja check templates/404.html
<!doctype html>
<html lang="en">
<title>Not found</title>
<h1>Not found</h1>
<p>Nothing is here. <a href="/blog/">See the blog</a>.</p>
</html>
```

The host serves it at any URL, so its links and files should be paths from the root, such as `/blog/`. It is left out of the sitemap. Cloudflare Workers serve it only with `not_found_handling` set in `wrangler.jsonc`; with the `static` adapter, configure the server to.

## Precompressing for the `static` adapter

`precompress = true` writes a brotli `.br` and a gzip `.gz` beside each text file: `.html`, `.css`, `.js`, `.mjs`, `.json`, `.xml`, `.txt`, `.svg` and `.webmanifest`, Static files included. The server sends one when the browser accepts it, without compressing per request.

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[routes.missing]
path = "/404.html"
template = "404.html"

[host]
adapter = "static"
precompress = true
```

Only the `static` adapter takes it; no other host serves the files:

```toml fail config/invalid-value
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[host]
adapter = "netlify"
precompress = true
```

The server must also send the cache policy, the headers, the redirects and the 404 Page. A Caddyfile that does, for `dist/` copied to `/srv/example/dist`, with one redirect:

```text
example.com {
	root * /srv/example/dist

	header X-Content-Type-Options nosniff
	@hashed path /assets/*
	header @hashed Cache-Control "public, max-age=31536000, immutable"
	@unhashed not path /assets/*
	header @unhashed Cache-Control "public, max-age=0, must-revalidate"

	redir /posts/ /blog/ 308

	file_server {
		precompressed br gzip
	}

	handle_errors 404 {
		rewrite * /404.html
		file_server {
			precompressed br gzip
		}
	}
}
```

The same for nginx. `brotli_static` needs the ngx_brotli module; without it, leave that line out and the `.gz` files still serve:

```text
server {
    listen 80;
    server_name example.com;
    root /srv/example/dist;

    gzip_static on;
    gzip_vary on;
    brotli_static on;

    error_page 404 /404.html;

    location / {
        add_header Cache-Control "public, max-age=0, must-revalidate" always;
        add_header X-Content-Type-Options nosniff always;
    }

    location /assets/ {
        add_header Cache-Control "public, max-age=31536000, immutable" always;
        add_header X-Content-Type-Options nosniff always;
    }

    location = /posts/ {
        return 308 /blog/;
    }
}
```

Both servers redirect a directory's URL without its `/` to the URL with it, as the other hosts do.

## Host limits

Every Site is held to Cloudflare's limits, the strictest supported host's, whatever the adapter, so it can move hosts. Over them is `host/over-limit`:

- 100 header rules, counting yuanme's two. Rules with one `path` count once, and one rule with a `*`, such as `/fonts/*`, covers many files.
- 2,000 redirects.
- 2,000 characters on one line of `_headers` or `_redirects`.

## Diagnostic codes

`yuanme docs CODE` explains each:

- `host/over-limit`: more rules, redirects or a longer line than the limits.
- `host/redirect-conflict`: a redirect from a URL the Site serves.
- `host/output-collision`: a Static file with the name of an adapter's file.
- `host/unsupported`: the host can't do what Site config asks.
- `host/cache-rule`: a header rule breaks the cache policy.
- `host/wrangler-mismatch`: `wrangler.jsonc` doesn't serve the Site as the cloudflare adapter writes it.
- `link/redirected`: a link or redirect to a URL the host redirects.
- `link/broken`: a link or redirect to a URL where nothing is.

All but `host/over-limit`, `host/redirect-conflict` and `host/output-collision` are warnings, which `--strict` makes errors.
