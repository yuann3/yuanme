---
title: Templates
description: The Jinja files that make each Page, what each kind of Route sees, the filters yuanme adds, Layouts and Partials, and what check finds in them.
---

A Template is a Jinja file in `templates/`, filled by the Engine with [minijinja](https://github.com/mitsuhiko/minijinja) to make a Page. Templates name each other by their path from `templates/`, such as `layouts/base.html`. `yuanme check` reads each one against the schema of what its Pages see before any Page renders.

How a Template is filled:

- A Template whose name ends in `.html` or `.xml` escapes what it prints: `&`, `<`, `>`, `"` and `'`, but not `/`. Any other Template, such as `llms.txt`, prints values as they are.
- `entry.content` and what `markdown`, `org`, `attrs` and `safe` return are finished HTML, printed without escaping.
- `none` prints as nothing, and a boolean prints as `true` or `false`.
- A line that holds only a `{% … %}` tag leaves no blank line behind.
- An absent value is false in `{% if %}`, but printing it, looping over it, comparing it or reading a field of it fails the Page. `check` reports each of those before rendering.
- A `.css`, `.js`, `.mjs`, `.ts` or `.mts` file in `templates/` is a Sidecar, not a Template; `yuanme docs components` covers Sidecars.

## Routes and their Templates

Every Route of the Site has one Template. Site config declares the Routes, and each kind has a default Template:

- **Entry**: `<collection>/entry.html`, or the `template` of `[collections.NAME]`.
- **list**: `<collection>/list.html`, for a Collection with a `list_route`, on each page of the list.
- **Term**: `<taxonomy>/term.html`, or the `template` of `[taxonomies.NAME]`.
- **Taxonomy index**: `<taxonomy>/index.html`, at the part of the Taxonomy's `route` before `{term}`.
- **Template alone**: `[routes.NAME]` names its `template`, which has no default.
- **feed**: `[feeds.NAME]` uses `rss.xml`, or its `template`. Without a `templates/rss.xml`, `rss.xml` is the built-in RSS 2.0 Template.

This Site config has one Route of each kind:

```toml check
[site]
url = "https://example.com"
title = "Example"
description = "Notes on Rust and maths."

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"
paginate = 10

[taxonomies.tags]
collections = ["posts"]
route = "/tags/{term}/"

[routes.home]
path = "/"
template = "home.html"

[feeds.rss]
collections = ["posts"]
path = "/rss.xml"
```

With this Post, its Templates are `posts/entry.html`, `posts/list.html`, `tags/index.html` (at `/tags/`), `tags/term.html` (at `/tags/rust/`), `home.html` and the built-in `rss.xml`. A Template that a Route needs and that isn't there is `template/not-found`. `yuanme routes` lists every Route with its Template.

```md check content/posts/hello.md
---
title: Hello
date: 2025-12-12
tags: [rust]
---

My first Post.
```

The built-in `rss.xml` writes the channel's title from `site.title` (or `site.url`), its description from `site.description` and its language from `site.language`, and one item per Entry with its title, link, `description` if it has one, and date.

## What each kind sees

Every Page sees these:

- `site`: `url`, `timezone`, `extra` (the `[site.extra]` table), and `title`, `description`, `language` and `author`, each only when Site config sets it.
- `data`: each Data file by its name, so `data/nav.yaml` is `data.nav`. Its fields are typed by its `[data.NAME]` schema in Site config, or else from the values in the file.
- `collections`: each Collection's listed Entries in its sort order, such as `collections.posts`. The built-in `pages` Collection, from `content/pages/`, is there too.
- `taxonomies`: each Taxonomy as `name`, `url` (its index) and `terms`.
- `page`: `url`, `canonical` (the full URL, from `site.url`) and `kind`, which is `entry`, `list`, `term`, `index`, `template` or `feed`.

Each kind of Route adds its own:

- `entry`: `entry`.
- `list`: `collection` (`name` and `url`), `entries` (this page's Entries) and `paginator`.
- `term`: `taxonomy`, `term`, `entries` (this page's Entries with the Term) and `paginator`.
- `index`: `taxonomy` and `terms`.
- `template`: nothing more.
- `feed`: `feed` (`name` and `url`) and `entries` (published, listed Entries, newest first).

An Entry has:

- `title`, `slug`, `url`, `draft` and `unlisted`.
- `date`, `updated` and `modified` (`updated`, or else `date`), as `YYYY-MM-DD` text. Each is optional, but `date` and `modified` aren't when the Collection declares `date` required.
- `content`: the body as finished HTML.
- `headings`: each heading's `id`, `text` and `level`.
- `word_count` and `reading_time`, in minutes.
- Every field its Collection declares, under its own name. An optional field without a default may be absent.
- A field for each Taxonomy of its Collection: a list of Term names, or for a `single` Taxonomy one optional name.

A Term has `name`, `slug`, `url` and `count`. A `paginator` has `number` and `total`, and the URLs `prev` and `next`, which are absent on the first and last page. A datetime is ISO 8601 text with its offset.

`yuanme schema context KIND` prints the JSON Schema of what a kind sees, for this Site's Collections and fields, such as `yuanme schema context entry`.

## Filters and functions

Templates have Jinja's own filters, tests and functions, such as `length`, `join`, `default`, `selectattr`, `map`, `first` and `range`, and no methods: `entry.title.lower()` is `template/unknown-filter`, and `entry.title | lower` is the filter. yuanme adds:

- `items | sort_by(["-date", "title"])`: sorts by each field in turn, `-` for descending. An item without the field sorts last whichever the direction, and ties break by Slug, as a Collection's `sort` does. Jinja's own `sort` puts missing values first.
- `date | dateformat(format="%d %B %Y")`: a date or datetime as strftime directives, or `"rfc2822"` or `"rfc3339"`. A datetime is shown in `site.timezone`; a date stays its calendar day.
- `url("/blog/")`: a path of the Site, which must be a Route, feed or Static file, or it fails the Page with `template/unknown-url`. A `#fragment` or `?query` is kept.
- `absolute_url(entry.url)`: a path as a full URL from `site.url`. A full URL is left as it is.
- `n | pluralize`: `""` for 1 and `"s"` otherwise, or `n | pluralize("entry", "entries")`.
- `{"class": "row", "hidden": true} | attrs`: a map as HTML attributes, each with a space before it. An absent, `none` or `false` value is left out, and `true` is a bare attribute.
- `text | markdown` and `text | org`: Markdown or Org from a Data file or Site config, rendered as an Entry's body is. With `inline=true`, a lone paragraph loses its `<p>`.
- `escape` and `e` escape the same five characters as printing does.

```jinja check templates/home.html
<!doctype html>
<html lang="en">
<title>{{ site.title }}</title>
<p>{{ "Notes on *Rust* and maths." | markdown(inline=true) }}</p>
<ul>
{% for post in collections.posts | sort_by(["-date", "title"]) %}
  <li{{ {"class": "draft" if post.draft else none} | attrs }}>
    <a href="{{ post.url }}">{{ post.title }}</a>
    {% if post.date %}<time datetime="{{ post.date }}">{{ post.date | dateformat(format="%d %B %Y") }}</time>{% endif %}
  </li>
{% endfor %}
</ul>
<p>{{ collections.posts | length }} {{ collections.posts | length | pluralize("post", "posts") }}.
<a href="{{ url('/rss.xml') }}">RSS</a></p>
</html>
```

## Layouts

A Layout is a Template that others extend. It defines blocks, and a Template that extends it fills them; anything outside its blocks is ignored. A `vars` comment declares the variables the Layout reads that the Templates extending it set, with the types of a Component's props (`yuanme docs components`); `?` after a name makes it optional.

```jinja check templates/partials/nav.html
<nav>
  <a href="{{ url('/') }}"{{ {"aria-current": "page" if page.url == "/" else none} | attrs }}>Home</a>
  <a href="{{ url('/blog/') }}">Blog</a>
  <a href="{{ url('/tags/') }}">Tags</a>
</nav>
```

```jinja check templates/layouts/base.html
{# vars: title: string, description?: string #}
<!doctype html>
<html lang="en">
<head>
<title>{{ title }} | {{ site.title }}</title>
{% if description %}<meta name="description" content="{{ description }}">{% endif %}
<link rel="canonical" href="{{ page.canonical }}">
<link rel="alternate" type="application/rss+xml" href="{{ url('/rss.xml') }}">
</head>
<body>
{% include "partials/nav.html" %}
<main>{% block content %}{% endblock %}</main>
</body>
</html>
```

A Template that extends it sets each required variable with `{% set %}` outside its blocks, and fills the blocks it wants:

```jinja check templates/posts/entry.html
{% extends "layouts/base.html" %}
{% set title = entry.title %}
{% block content %}
<article>
  <h1>{{ entry.title }}</h1>
  {% if entry.date %}<p><time datetime="{{ entry.date }}">{{ entry.date | dateformat(format="%d %B %Y") }}</time></p>{% endif %}
  {{ entry.content }}
  {% if entry.tags %}<p>Tagged {{ entry.tags | join(", ") }}</p>{% endif %}
</article>
{% endblock %}
```

A required variable that is never set, or set to a value of the wrong type, is `template/bad-var`. Here `title` is missing:

```jinja fail template/bad-var templates/tags/index.html
{% extends "layouts/base.html" %}
{% block content %}
<ul>{% for t in terms %}<li><a href="{{ t.url }}">{{ t.name }}</a> ({{ t.count }})</li>{% endfor %}</ul>
{% endblock %}
```

A block that no Layout in the chain defines is `template/unknown-block`:

```jinja fail template/unknown-block templates/posts/entry.html
{% extends "layouts/base.html" %}
{% set title = entry.title %}
{% block contnet %}{{ entry.content }}{% endblock %}
```

Inside a block, `{{ super() }}` prints the Layout's own content of that block. A Layout may extend another Layout.

## Partials

A Partial is a Template included as it is, with `{% include "partials/nav.html" %}`. It sees everything the including Template sees, `entry` and `entries` included, and is checked as part of each Template that includes it. Use a Partial for markup that reads the Page's own values, and a Component, which takes declared props, for markup that is reused with different values (`yuanme docs components`).

```jinja check templates/partials/pages.html
{% if paginator.prev or paginator.next %}
<nav>
  {% if paginator.prev %}<a href="{{ paginator.prev }}">Newer</a>{% endif %}
  <span>{{ paginator.number }} of {{ paginator.total }}</span>
  {% if paginator.next %}<a href="{{ paginator.next }}">Older</a>{% endif %}
</nav>
{% endif %}
```

```jinja check templates/posts/list.html
{% extends "layouts/base.html" %}
{% set title = "Blog" %}
{% block content %}
<h1>Blog</h1>
<ul>
{% for post in entries %}
  <li><a href="{{ post.url }}">{{ post.title }}</a></li>
{% endfor %}
</ul>
{% include "partials/pages.html" %}
{% endblock %}
```

## Template names are plain strings

`extends`, `include` and `import` name a Template with a quoted string, or a list of them, never an expression. That way yuanme knows every Template a Page uses without rendering it, which is how it checks them and how a Page gets their Sidecars. Choose between Templates with `{% if %}`:

```jinja fail template/non-constant-name templates/tags/term.html
{% extends "layouts/base.html" %}
{% set title = term.name %}
{% block content %}{% include "partials/" ~ page.kind ~ ".html" %}{% endblock %}
```

## What check finds

`yuanme check` walks every Template once for each kind of Route that uses it, with the types of what those Pages see, before any Page renders. It finds a mistake even on a path no Entry reaches, or one that `| default` would hide. A Template shared by two Collections is checked against both, so a field only one of them has is optional there. A Template that no Route uses is checked only as part of the Templates that use it.

A field or variable that isn't in the schema is `template/unknown-field`, with the name it probably meant:

```jinja fail template/unknown-field templates/posts/entry.html
<h1>{{ entry.titel }}</h1>
```

So is a value that only another kind of Route has, such as `entries` on an Entry's Page, or `entry` on a Term's.

An optional field that is printed, looped over, filtered or reached through without a guard is `template/unguarded-optional`. `updated` is optional:

```jinja fail template/unguarded-optional templates/posts/entry.html
<h1>{{ entry.title }}</h1>
<p>Updated {{ entry.updated | dateformat(format="%d %B %Y") }}</p>
```

The guards `check` understands are `{% if %}`, its `{% else %}`, `and`, an inline `if`, `is defined`, a loop's `if`, and `| default(…)`. Each line here checks clean:

```jinja check templates/posts/entry.html
<h1>{{ entry.title }}</h1>
{% if entry.updated %}<p>Updated {{ entry.updated }}</p>{% endif %}
{% if not entry.updated %}<p>Never updated</p>{% else %}<p>Updated {{ entry.updated }}</p>{% endif %}
{% if entry.updated and entry.date and entry.updated != entry.date %}<p>Revised</p>{% endif %}
<p>{{ entry.updated if entry.updated else "Never updated" }}</p>
<p>{{ entry.updated if entry.updated is defined else "Never updated" }}</p>
<p>Updated {{ entry.updated | default("never") }}</p>
{% for post in collections.posts if post.updated %}<p>{{ post.updated }}</p>{% endfor %}
{% if entry.updated %}{% set updated = entry.updated %}<p>{{ updated }}</p>{% endif %}
```

A variable set from a guarded field stays guarded. `| default(none)` keeps a value optional, and an early `{% continue %}` is not a guard.

Comparing an optional field with `==`, `!=`, `<`, `in` or another comparison is `template/compare-optional`, even as the condition of an `{% if %}`, because comparing an absent value fails where testing it doesn't. Guard it first, or compare a default:

```jinja fail template/compare-optional templates/posts/entry.html
<h1>{{ entry.title }}</h1>
{% if entry.updated != entry.date %}<p>Revised.</p>{% endif %}
```

A filter, test or function that neither Jinja nor yuanme has is `template/unknown-filter`, as is a method call:

```jinja fail template/unknown-filter templates/posts/entry.html
<h1>{{ entry.title | lowercase }}</h1>
```

`dateformat` needs a `format`, by name, that it knows, and Jinja's `sort(attribute="-date")` sorts by a field named `-date`, so it should be `sort_by(["-date"])`. Either mistake is `template/bad-filter-argument`. A field that `sort_by`, `sort`, `map`, `groupby`, `selectattr` or `rejectattr` names that the items don't have is `template/unknown-field`.

```jinja fail template/bad-filter-argument templates/posts/entry.html
<h1>{{ entry.title }}</h1>
{% if entry.date %}<time>{{ entry.date | dateformat(format="iso") }}</time>{% endif %}
```

Looping over a value that isn't a list, or giving `attrs` something that isn't a map, is `template/wrong-type`:

```jinja fail template/wrong-type templates/posts/entry.html
{% for tag in entry.title %}{{ tag }}{% endfor %}
```

`check` also reports a Template that doesn't parse (`template/syntax`), one that is named but missing (`template/not-found`), and every mistake in a Component's props or a call to it (`yuanme docs components`). A value of type `object`, in a props comment or a Data file's schema, isn't checked. What `check` can't see fails when the Page renders, as `template/render`, or `template/unknown-url` for a `url()` that leads nowhere:

```jinja fail template/unknown-url templates/partials/nav.html
<nav><a href="{{ url('/abuot/') }}">About</a></nav>
```

`yuanme docs CODE` explains each code, such as `yuanme docs template/unguarded-optional`.
