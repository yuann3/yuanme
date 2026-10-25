---
title: "Site config"
description: "yuanme.toml, the one file that declares the Site, with every key it takes."
---

`yuanme.toml`, at the root of the project, is the Site config. Only `[site]` and its `url` are required:

```toml check
[site]
url = "https://example.com"
```

Unknown keys are errors, not ignored, so a typo can't silently do nothing. The help names the key that was probably meant:

```toml fail config/unknown-key
[site]
url = "https://example.com"
titel = "Example"
```

Keys of your own go in `[site.extra]`, which Templates see as `site.extra`:

```toml check
[site]
url = "https://example.com"
title = "Example"

[site.extra]
github = "https://github.com/example"
```

`yuanme schema config` prints Site config's JSON Schema, which `check`, `build` and the other commands also write to `.yuanme/schemas/yuanme.toml.json`. An editor with a TOML language server, such as Taplo or Tombi, reads it from a first line of:

```toml
#:schema ./.yuanme/schemas/yuanme.toml.json
```

Collections and their fields have their own page, `yuanme docs collections`.

## Reference

### Top level

- `data` (table of types, below): Schemas for Data files: `[data.nav]` checks `data/nav.toml`, `.yaml` or `.json`. A Data file without one is not checked.
- `yuanme` (string): The oldest yuanme release this project works with, as "X.Y". A newer minor release in the same major builds it; before 1.0, only the same 0.Y does.

### `[build]`

How yuanme builds the Site's stylesheets and Scripts.

### `[build.targets]`

The oldest browsers the Site supports, as explicit versions such as { chrome = "111", safari = "16.4" }. yuanme adds the prefixes and fallbacks they need and drops the rest; a browser left out gets none. Defaults to Tailwind v4's baseline: Chrome 111, Firefox 128 and Safari 16.4.

- `chrome` (string): Chrome.
- `edge` (string): Edge.
- `firefox` (string): Firefox.
- `opera` (string): Opera.
- `safari` (string): Safari, on macOS and on iOS.
- `samsung` (string): Samsung Internet.

### `[collections.NAME]`

Named groups of Entries sharing one schema, such as posts or projects.

- `dir` (string): The folder of the Collection's Entries. Defaults to "content/<name>".
- `fields` (table of fields, below): Fields Entries have beyond the built-in `title`, `date`, `updated`, `slug`, `draft` and `unlisted`. Declaring `date` makes it required.
- `list_route` (string): The URL of the page listing the Collection, such as "/blog/". Page N ≥ 2 is at "<list_route>N/".
- `paginate` (integer): Entries per list page. Needs `list_route`.
- `route` (string, required): The URL of each Entry, with `{slug}` where its Slug goes, such as "/blog/{slug}/".
- `sort` (list of strings): Field names to sort Entries by, each prefixed with `-` for descending. Entries missing a key sort last; ties break by Slug. Defaults to ["-date"].
- `template` (string): The Template of each Entry's Page. Defaults to "<name>/entry.html".

### `[feeds.NAME]`

Feeds of published, listed Entries, newest first.

- `collections` (list of strings, required): The Collections whose Entries the feed holds.
- `path` (string, required): The URL of the feed, such as "/rss.xml".
- `template` (string): The Template that writes the feed. Defaults to "rss.xml": the built-in RSS 2.0 Template, unless `templates/rss.xml` replaces it.

### `[host]`

The static host the Site is deployed to, and the headers and redirects it sends.

- `adapter` (one of `"cloudflare"`, `"netlify"`, `"vercel"`, `"github-pages"`, `"cloudflare-pages"`, `"static"`): The host whose configuration the build writes: "cloudflare" (Workers static assets, the default), "netlify", "vercel", "github-pages", "cloudflare-pages" or "static".
- `precompress` (boolean): Write a brotli `.br` and a gzip `.gz` beside each text file, for Caddy or nginx. Only with the `static` adapter: no other host serves them.

### `[[host.headers]]`

Response headers for the URLs a path matches, after yuanme's own cache rules. Where two rules set one header, the later one wins.

- `headers` (table of strings, required): The headers, by name: { "Cache-Control" = "public, max-age=3600" }.
- `path` (string, required): The URL path the headers are for, such as "/resume.pdf". One `*` matches any characters: "/fonts/*".

### `[[host.redirects]]`

URLs the host redirects, such as a Page's old address.

- `from` (string, required): The URL path that redirects, such as "/old-post/".
- `status` (integer): 301, 302, 307 or 308. Defaults to 308, a permanent redirect that keeps the method.
- `to` (string, required): Where it goes: a URL path such as "/blog/new-post/", or an absolute URL.

### `[images]`

How yuanme resizes and encodes the images that bodies, `image` fields and `image()` name.

- `allow_unoptimized` (list of strings): Rasters over 50 KB that a Page may load as they are, by URL from the root, such as ["/favicon.png"]: `image/unoptimized` doesn't warn of them.
- `avif_speed` (integer): How hard the AVIF encoder works, from 1, the smallest files and slowest, to 10. Defaults to 6.
- `formats` (list of one of `"avif"`, `"webp"`): The formats offered before the JPEG fallback, or PNG for a source with transparency, in order. Defaults to ["avif"], which every default target browser decodes; add "webp" for older ones.
- `sizes` (string): The `sizes` attribute: how wide an image is shown, which picks the width a browser fetches. Defaults to "100vw"; set it to the prose width, such as "(min-width: 768px) 720px, 100vw".
- `widths` (list of integers): The widths each image is resized to, in pixels. A source narrower than one uses its own width instead: nothing is upscaled. Defaults to [480, 800, 1200, 1600, 2400].

### `[images.quality]`

Each encoder's quality, on its own scale from 1 to 100.

- `avif` (integer): Defaults to 72.
- `jpeg` (integer): Defaults to 80.
- `webp` (integer): Defaults to 80.

### `[math]`

How math renders.

- `macros` (table of strings): LaTeX macros for every Entry's math, by name without the backslash: `R = '\mathbb{R}'`, or with arguments, `norm = '\left\lVert #1 \right\rVert'`.

### `[math.theorems.NAME]`

Theorem block kinds, by the name a block is written with: `[math.theorems.claim]` makes `:::claim` in Markdown and `#+begin_claim` in Org Theorem blocks. The built-in kinds are theorem, lemma, proposition, corollary, conjecture, definition, example and exercise, numbered on one shared counter, and remark, note and proof, unnumbered; a table here changes one of them or adds a kind.

- `counter` (string or `false`): The counter the kind's blocks are numbered on, per Entry: "theorem", which the built-in numbered kinds share, a name of its own, or false for unnumbered blocks. Defaults to the built-in kind's, or to "theorem".
- `label` (string): What the kind's blocks are called on the Page, as in "Claim 2". Defaults to the built-in kind's, or to the name with its first letter capitalized.

### `[router]`

How a visitor moves between Pages.

- `mode` (one of `"off"`, `"native"`, `"swap"`): "off" (the default): every move between Pages is a full page load. "native": full page loads that the browser animates with a cross-document View Transition, with no JS. "swap": the Router on every Page, which swaps the next Page into the current document, so elements marked `data-yuanme-persist` survive; browsers without the Navigation API get full page loads.

### `[routes.NAME]`

Pages made by a Template alone, such as the home page or the 404 page.

- `path` (string, required): The URL of the Page, such as "/" or "/404.html".
- `template` (string, required): The Template that makes the Page, by its path in `templates/`, such as "home.html".

### `[site]`

The Site's metadata.

- `author` (string): The Author's name.
- `description` (string): One or two sentences about the Site, for feeds and search engines.
- `extra` (table): Anything else Templates need, under keys of your own.
- `language` (string): The language of the Site's content, as a BCP 47 tag such as "en" or "zh-Hans".
- `timezone` (string): The IANA time zone, such as "Asia/Singapore", for datetimes written without an offset. Defaults to "UTC", never the machine's zone.
- `title` (string): The Site's name.
- `url` (string, required): The address the Site is served from, such as "https://example.com". Feeds, sitemaps and canonical links are built from it.

### `[taxonomies.NAME]`

Named ways of grouping Entries by a shared value, such as tags or series.

- `collections` (list of strings, required): The Collections whose Entries it groups. Each gets a field with the Taxonomy's name.
- `lowercase` (boolean): Show every Term in lowercase. Otherwise a Term shows as first spelled.
- `paginate` (integer): Entries per Term page.
- `route` (string, required): The URL of each Term's page, with `{term}` where its Slug goes, such as "/tags/{term}/".
- `single` (boolean): Each Entry has at most one Term, written as a string rather than a list.
- `slugs` (table of strings): Slugs that override the ones yuanme derives, by Term: { "C++" = "cpp" }.
- `sort` (list of strings): How Entries sort on a Term's page, as for a Collection.
- `template` (string): The Template of each Term's Page. Defaults to "<name>/term.html".

### Field types

#### `type = "string"`

Text.

- `default` (any value): The value an Entry that leaves the field out gets. A field with a default is optional.
- `description` (string): What the field is for.
- `max` (integer): The most characters.
- `min` (integer): The fewest characters.
- `optional` (boolean): Entries may leave the field out.

#### `type = "int"`

A whole number.

- `default` (any value): The value an Entry that leaves the field out gets. A field with a default is optional.
- `description` (string): What the field is for.
- `max` (integer): The largest value.
- `min` (integer): The smallest value.
- `optional` (boolean): Entries may leave the field out.

#### `type = "float"`

A number.

- `default` (any value): The value an Entry that leaves the field out gets. A field with a default is optional.
- `description` (string): What the field is for.
- `max` (number): The largest value.
- `min` (number): The smallest value.
- `optional` (boolean): Entries may leave the field out.

#### `type = "bool"`

true or false.

- `default` (any value): The value an Entry that leaves the field out gets. A field with a default is optional.
- `description` (string): What the field is for.
- `optional` (boolean): Entries may leave the field out.

#### `type = "date"`

A calendar date, such as 2025-12-12.

- `default` (any value): The value an Entry that leaves the field out gets. A field with a default is optional.
- `description` (string): What the field is for.
- `optional` (boolean): Entries may leave the field out.

#### `type = "datetime"`

A date and time. Without an offset it is in `site.timezone`.

- `default` (any value): The value an Entry that leaves the field out gets. A field with a default is optional.
- `description` (string): What the field is for.
- `optional` (boolean): Entries may leave the field out.

#### `type = "url"`

An absolute URL.

- `default` (any value): The value an Entry that leaves the field out gets. A field with a default is optional.
- `description` (string): What the field is for.
- `optional` (boolean): Entries may leave the field out.

#### `type = "enum"`

One of a fixed set of strings.

- `default` (any value): The value an Entry that leaves the field out gets. A field with a default is optional.
- `description` (string): What the field is for.
- `optional` (boolean): Entries may leave the field out.
- `values` (list of strings, required): The strings allowed.

#### `type = "list"`

A list of values of one type.

- `default` (any value): The value an Entry that leaves the field out gets. A field with a default is optional.
- `description` (string): What the field is for.
- `items` (a type, below, required): The type of each item.
- `optional` (boolean): Entries may leave the field out.
- `unique` (boolean): No item may appear twice.

#### `type = "object"`

A table with named fields.

- `default` (any value): The value an Entry that leaves the field out gets. A field with a default is optional.
- `description` (string): What the field is for.
- `fields` (table of fields, below, required): The object's fields.
- `optional` (boolean): Entries may leave the field out.

#### `type = "image"`

A path to an image beside the Entry or in the project, or a remote URL.

- `default` (any value): The value an Entry that leaves the field out gets. A field with a default is optional.
- `description` (string): What the field is for.
- `optional` (boolean): Entries may leave the field out.

### Types of Data files and list items

#### `type = "string"`

Text.

- `description` (string): What the field is for.
- `max` (integer): The most characters.
- `min` (integer): The fewest characters.

#### `type = "int"`

A whole number.

- `description` (string): What the field is for.
- `max` (integer): The largest value.
- `min` (integer): The smallest value.

#### `type = "float"`

A number.

- `description` (string): What the field is for.
- `max` (number): The largest value.
- `min` (number): The smallest value.

#### `type = "bool"`

true or false.

- `description` (string): What the field is for.

#### `type = "date"`

A calendar date, such as 2025-12-12.

- `description` (string): What the field is for.

#### `type = "datetime"`

A date and time. Without an offset it is in `site.timezone`.

- `description` (string): What the field is for.

#### `type = "url"`

An absolute URL.

- `description` (string): What the field is for.

#### `type = "enum"`

One of a fixed set of strings.

- `description` (string): What the field is for.
- `values` (list of strings, required): The strings allowed.

#### `type = "list"`

A list of values of one type.

- `description` (string): What the field is for.
- `items` (a type, below, required): The type of each item.
- `unique` (boolean): No item may appear twice.

#### `type = "object"`

A table with named fields.

- `description` (string): What the field is for.
- `fields` (table of fields, below, required): The object's fields.

#### `type = "image"`

A path to an image beside the Entry or in the project, or a remote URL.

- `description` (string): What the field is for.

