---
title: "Collections and fields"
description: "Collections and their Entries, the fields every Entry has and the ones a Collection declares, Slugs, Taxonomies, Data files, and yuanme new."
---

A Collection is a named group of Entries that share one schema, such as posts or projects. Site config declares each one under `[collections.NAME]`, and each Entry is one Markdown (`.md`) or Org (`.org`) file in the Collection's folder.

## Collections

`route` is required and holds `{slug}`. The other keys are optional:

- `dir`: the folder of the Entries. Defaults to `content/NAME`. It must exist, even empty, or the build reports `config/invalid-value`.
- `list_route`: the URL of the Page that lists the Collection.
- `paginate` and `sort`: see [Sort and pagination](#sort-and-pagination).
- `fields`: the fields Entries have beyond the built-in ones.
- `template`: the Template of each Entry's Page. Defaults to `NAME/entry.html`. The list Page always uses `NAME/list.html`.

Every `.md` and `.org` file under `dir` is an Entry, in subfolders too. A file in the folders of two Collections belongs to the one with the deeper folder. Other files in the folder are not Entries.

### The `pages` Collection

Every Site has a built-in Collection, `pages`, for prose pages outside any other: its folder is `content/pages`, its route is `/{slug}/`, it has no list and no declared fields, and its Template is `pages/entry.html`. Its folder doesn't need to exist until you put an Entry in it.

```md check content/pages/about.md
---
title: About
---

I write about Rust and mathematics.
```

This Entry is served at `/about/`. Declare `[collections.pages]` yourself to change any of that; your declaration replaces the built-in one.

## Built-in fields

Every Entry has these fields, whatever its Collection declares:

- `title`: a string. Required.
- `date`: when the Entry was published, as a date such as `2025-12-12`. Optional, unless the Collection declares `date`.
- `updated`: when it last changed, as a date.
- `slug`: its URL-safe name. Defaults to one made from the file's path.
- `draft`: `true` for a Draft.
- `unlisted`: `true` for an Unlisted Entry.

```md check content/posts/hello.md
---
title: Hello
date: 2025-12-12
updated: 2026-01-05
slug: hello-world
---

My first Post.
```

Front matter is YAML 1.2 between `---` lines at the top of the file. `yes`, `no`, `on` and `off` are strings, not booleans: write `true` and `false`. A key that isn't a field is an error, `entry/unknown-key`, with the field you probably meant; a key written twice is `entry/duplicate-key`. A key with nothing after it (`summary:`) leaves the field out. TOML front matter between `+++` lines is `entry/toml-front-matter`.

Templates see a few more fields that the Engine derives, so no declared field can take their names: `modified` (`updated`, or else `date`), `word_count`, `reading_time` in minutes, `headings`, `url` and `content`. `yuanme docs templates` shows them.

### Drafts and Unlisted Entries

A Draft (`draft: true`) is left out of the build: it has no Page, and it is in no list, Taxonomy or feed. Pass `--drafts` to `yuanme build`, `yuanme check` or `yuanme routes` to include Drafts while you preview the Site. A link to a Draft from a published Entry is an error, `link/unresolved`, because the published Site wouldn't have the Page it leads to.

```md check content/posts/next.md
---
title: Coming soon
draft: true
---

Not finished.
```

An Unlisted Entry (`unlisted: true`) is published and has its Page at its URL, but it is left out of the Collection's list, every Term, every feed and the sitemap.

## Slugs

An Entry's Slug comes from its path in the Collection's folder. Each folder name and the file name, without its extension, is lowercased, spaces become `-`, and punctuation is dropped. Letters in any script stay.

- `content/posts/Hello World.md` → `hello-world`
- `content/posts/2025/recap.md` → `2025/recap`, served at `/blog/2025/recap/`
- `content/posts/trip/index.md` → `trip`, so the Entry's images can sit beside it in `trip/`

`slug:` in front matter overrides it. It must already be a valid Slug: lowercase letters, digits, `-` and `_`, in segments separated by `/`. Anything else is `entry/invalid-slug`, with the corrected Slug as a fix:

```md fail entry/invalid-slug content/posts/hello.md
---
title: Hello
slug: Hello World
---
```

Two Entries of one Collection with the same Slug are an error, `entry/duplicate-slug`. `hello.md` above already has the Slug `hello-world`, so this file collides with it:

```md fail entry/duplicate-slug content/posts/hello-world.md
---
title: Hello again
---
```

Two Routes that would write the same file, such as an Entry at `/blog/2/` and the second page of the list, are `entry/output-collision`.

## Declared fields

A Collection declares more fields under `fields`, each with a `type`. Each field also takes:

- `description`: what the field is for. Diagnostics and the JSON Schema show it.
- `optional = true`: Entries may leave it out. Templates must check that an optional field is there before they use it.
- `default`: the value an Entry that leaves the field out gets. A field with a default is optional. The default is checked against the type.

A field without `optional` or `default` is required, and an Entry without it is `entry/missing-field`.

The types, and the keys each one takes:

- `string`: text. `min` and `max` bound its length in characters.
- `int`: a whole number. `min` and `max` bound it.
- `float`: a number. `min` and `max` bound it.
- `bool`: `true` or `false`.
- `date`: a calendar date, `2025-12-12`.
- `datetime`: a date and time, `2025-12-12T09:30:00+08:00`. Without an offset it is in `site.timezone`, which defaults to UTC.
- `url`: an absolute `http://` or `https://` URL.
- `enum`: one of the strings in `values`.
- `list`: a list of values of the type `items` names. `unique = true` forbids repeats.
- `object`: a mapping with its own `fields`, declared as a Collection's are.
- `image`: a path to an image file, relative to the Entry's folder or, starting with `/`, to the project root, or a remote URL. A path that names no file is `image/not-found`.

Declaring `date`, with `type = "date"`, makes the built-in `date` required. Declaring any other built-in field is an error.

This Site config adds a `projects` Collection that uses most of them:

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[collections.projects]
route = "/projects/{slug}/"
list_route = "/projects/"
sort = ["-stars", "title"]

[collections.projects.fields]
summary = { type = "string", max = 160, description = "One sentence for lists." }
stars = { type = "int", min = 0, default = 0 }
rating = { type = "float", min = 0, max = 5, optional = true }
archived = { type = "bool", default = false }
released = { type = "date", optional = true }
started = { type = "datetime", optional = true }
repo = { type = "url" }
status = { type = "enum", values = ["active", "paused", "done"] }
languages = { type = "list", items = { type = "string" }, unique = true }
cover = { type = "image", optional = true }

[collections.projects.fields.links]
type = "object"
optional = true
fields = { docs = { type = "url" }, demo = { type = "url", optional = true } }
```

An Entry of `projects`, in `content/projects/`:

```md check content/projects/yuanme.md
---
title: yuanme
summary: A static site generator for Markdown and Org.
repo: https://github.com/yuann3/yuanme
status: active
languages: [Rust, TypeScript]
stars: 120
started: 2025-10-01T09:30:00+08:00
links:
  docs: https://docs.example.org/
---

What it does, and why.
```

`archived` gets its default, `false`; `rating`, `released`, `cover` and `links.demo` are absent. Each mistake has its own code. A value outside `values`:

```md fail entry/invalid-value content/projects/yuanme.md
---
title: yuanme
summary: A static site generator for Markdown and Org.
repo: https://github.com/yuann3/yuanme
status: finished
languages: [Rust]
---
```

A value of the wrong type is `entry/wrong-type`; a number or length out of bounds is `entry/out-of-range`; a date that isn't in ISO form is `entry/invalid-date`, with the ISO form as a fix where yuanme can read the date. A required field left out:

```md fail entry/missing-field content/projects/yuanme.md
---
title: yuanme
status: active
languages: [Rust]
---
```

## Sort and pagination

`sort` lists field names to sort the Collection's list by, each prefixed with `-` for descending. It defaults to `["-date"]`, newest first. Entries that have no value for a key sort after those that do, in either direction, and ties break by Slug. Any field but a list or an object can be a key, the derived `modified`, `word_count` and `reading_time` included.

`paginate = N` splits the list into Pages of N Entries; it needs `list_route`. Page 1 is at `list_route`, and page N at `list_route` followed by `N/`:

```toml
[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"
paginate = 10
```

This gives `/blog/`, `/blog/2/`, `/blog/3/` and so on. The list Template sees the page's `entries` and a `paginator` with `number`, `total`, and `prev` and `next` where there is such a page.

## Taxonomies and Terms

A Taxonomy groups Entries by a shared value, such as tags or series. Site config declares each one under `[taxonomies.NAME]`:

- `collections`: the Collections whose Entries it groups. Each one gets a field with the Taxonomy's name.
- `route`: the URL of each Term's Page, with `{term}` where its Slug goes. The part before `{term}` is the Taxonomy's index Page, which lists its Terms.
- `single = true`: each Entry has at most one Term, written as a string. Otherwise the field is a list of Terms.
- `lowercase = true`: show every Term in lowercase. Otherwise a Term shows as it was first spelled.
- `slugs`: Slugs you choose, by Term, overriding the ones yuanme makes.
- `paginate` and `sort`: as for a Collection, for each Term's Page.
- `template`: the Template of each Term's Page. Defaults to `NAME/term.html`; the index Page uses `NAME/index.html`.

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[taxonomies.tags]
collections = ["posts"]
route = "/tags/{term}/"
slugs = { "C++" = "cpp" }

[taxonomies.series]
collections = ["posts"]
route = "/series/{term}/"
single = true
```

```md check content/posts/cpp.md
---
title: Moving from C++
date: 2026-02-01
tags: [C++, Rust]
series: Rewrites
---

Why I rewrote it.
```

This Entry is listed at `/tags/cpp/`, `/tags/rust/` and `/series/rewrites/`, and the index Pages are `/tags/` and `/series/`. A Term's Slug spells out the symbols that carry meaning before it slugs the rest: `C++` would be `cplusplus` without the override, `C#` is `csharp` and `.NET` is `dotnet`. Spellings with the same Slug, such as `Rust` and `rust`, are one Term.

A list where `single` wants a string, or a string where the Taxonomy wants a list, is `entry/wrong-type`:

```md fail entry/wrong-type content/posts/cpp.md
---
title: Moving from C++
tags: Rust
---
```

Two Terms that `slugs` gives the same Slug are `entry/term-collision`. A Term with no letters or digits, such as `!!!`, is `entry/invalid-value`. Drafts and Unlisted Entries are in no Term, so a Term only they use has no Page.

## Data files

A Data file is structured data that belongs to the Site but isn't an Entry, such as navigation links or a résumé. Each file in `data/` named `NAME.toml`, `NAME.yaml`, `NAME.yml` or `NAME.json` is one, and Templates read it as `data.NAME`. Subfolders of `data/` aren't read. Two files with one name, such as `nav.toml` and `nav.yaml`, are `entry/duplicate-data-file`.

Without a schema, a Data file is read as it is written:

```yaml check data/nav.yaml
- title: Blog
  url: /blog/
- title: About
  url: /about/
```

`[data.NAME]` in Site config declares a schema, written as one field is: a `type` and the keys that type takes. The file is then checked with the same rules, and the same codes, as an Entry's fields. A `[data.NAME]` with no file is `config/invalid-value`.

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[taxonomies.tags]
collections = ["posts"]
route = "/tags/{term}/"
slugs = { "C++" = "cpp" }

[taxonomies.series]
collections = ["posts"]
route = "/series/{term}/"
single = true

[data.nav]
type = "list"
items = { type = "object", fields = { title = { type = "string" }, url = { type = "string" } } }
```

`data/nav.yaml` above matches it. An item without its `url` doesn't:

```yaml fail entry/missing-field data/nav.yaml
- title: Blog
  url: /blog/
- title: About
```

## Fields in Org

An Org Entry sets the same fields with keywords at the top of the file, before the first headline or text. A keyword's name is the field's name in any case, so `#+TITLE` sets `title` and `#+SUMMARY` sets `summary`. A string field takes the keyword's text as it is written; every other value is YAML, as in front matter, so a list is `[a, b]`. A date or datetime may also be an Org timestamp, `[2026-03-01 Sun]` or `<2026-03-01 Sun 10:00>`. When the Entry's Collection has a Taxonomy named `tags`, `#+FILETAGS` sets it, written `:a:b:`.

```text check content/posts/in-org.org
#+TITLE: Written in Org
#+DATE: [2026-03-01 Sun]
#+FILETAGS: :rust:org:
#+SERIES: Rewrites

The body, in Org.
```

A boolean is `true` or `false`, not Org's `t` or `nil`. A field keyword below the first headline or text is `org/misplaced-keyword`. `yuanme docs org` covers the rest of an Org Entry.

## `yuanme new`

`yuanme new COLLECTION "Title"` writes a new Entry whose required fields are filled in, so it builds before you write a word, and prints its path and URL:

```sh
yuanme new projects "My Tool"
```

It writes `content/projects/my-tool.md`, with `title`, today's `date` in `site.timezone`, and a placeholder for each required declared field: `TODO` for a string, `0` (or the nearest value `min` and `max` allow) for a number, `false`, the first of an enum's `values`, `[]` for a list, `https://example.com/` for a URL. Replace the placeholders. An `image` field can't have one, so `new` warns that you must fill it in. Add `--format org` for an Org Entry with keywords instead. `new` never overwrites a file that exists, and takes `pages` as a Collection too.

## Schemas

`yuanme schema entry COLLECTION` prints the JSON Schema of a Collection's front matter: the built-in fields, its declared fields and its Taxonomies. `yuanme schema data NAME` prints the schema of the Data file `[data.NAME]` declares. `yuanme check` and `yuanme build` also write them to `.yuanme/schemas/entry/COLLECTION.json` and `.yuanme/schemas/data/NAME.json`. Read them before you write an Entry or a Data file, rather than guessing at its fields.
