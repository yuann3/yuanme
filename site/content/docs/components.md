---
title: "Components"
description: "Reusable pieces of Templates with declared props, how to call them, how they differ from Partials, and the Sidecars beside them."
---

A Component is a Jinja macro in `templates/components/` with a props comment directly above it. The props comment declares what the Component takes, and `yuanme check` checks every call against it and the Component's body against what it may read. `yuanme docs templates` covers Templates in general.

## A Component

```md check content/posts/hello.md
---
title: Hello
date: 2025-12-12
---

My first Post.
```

```jinja check templates/components/row.html
{# props: href: url, title: string, date?: date, note?: string #}
{% macro row(href, title, date=none, note="") %}
<li>
  <a href="{{ href }}">{{ title }}</a>
  {% if date %}<time datetime="{{ date }}">{{ date | dateformat(format="%d %B %Y") }}</time>{% endif %}
  {{ note }}
</li>
{% endmacro %}
```

The props comment is `{# props: … #}`, one `name: type` for each of the macro's parameters, separated by commas. `?` after a name makes the prop optional, and its parameter needs a default:

- `=none` leaves the prop absent when a call doesn't pass it, so the body guards it, as `date` is guarded here.
- Any other default, such as `note=""`, is what the body sees when a call doesn't pass it, so the body needs no guard.

A required prop has no default. One file may hold several Components, each with its own props comment.

## Calling a Component

Import a Component with `from … import`, or import its file under a name, and call it with its props by name:

```jinja check templates/posts/list.html
{% from "components/row.html" import row %}
<!doctype html>
<html lang="en">
<title>Blog</title>
<ul>
{% for post in entries %}
  {{ row(href=post.url, title=post.title, date=post.date) }}
{% endfor %}
</ul>
</html>
```

```jinja check templates/posts/entry.html
{% import "components/row.html" as ui %}
<!doctype html>
<html lang="en">
<title>{{ entry.title }}</title>
<h1>{{ entry.title }}</h1>
{{ entry.content }}
<ul>{{ ui.row(href=url("/blog/"), title="All Posts", note="Every Post, newest first") }}</ul>
</html>
```

`post.date` is optional, which is fine for the optional `date`. A call that passes a prop the Component doesn't declare, leaves out a required one, passes a value of the wrong type, or passes an optional value to a required prop is `template/bad-prop`:

```jinja fail template/bad-prop templates/posts/list.html
{% from "components/row.html" import row %}
<ul>{% for post in entries %}{{ row(href=post.url, titel=post.title) }}{% endfor %}</ul>
```

A name the file doesn't define, such as `{% from "components/row.html" import rwo %}`, is `template/unknown-filter`. `yuanme docs CODE` explains each code.

## Prop types

- `string`: text. A `url`, an `enum` value, an `image` or a `slot` may be passed.
- `int` and `float`: numbers. A `float` takes an `int` too.
- `bool`.
- `date` and `datetime`: an Entry's `date` or a field of those types. A `datetime` takes a `date` too.
- `url`: a link, such as `entry.url` or `url("/blog/")`. Text may be passed.
- `image`: an image field.
- `enum(a|b)`: one of the values; a literal that isn't one is `template/bad-prop`.
- `list<T>`: a list of `T`, such as `list<string>` or `list<entry<posts>>`.
- `object`: any value, whose fields aren't checked.
- `slot`: finished HTML, such as a `{% set %}` block or another Component's output.
- `attrs`: a map of HTML attributes, printed with `| attrs`.
- `entry<collection>`: an Entry of that Collection, such as `entry<posts>`, whose fields are checked.
- `term`: a Term, as `term` and `terms` hold them.
- `paginator`: a list's `paginator`.

A `none` may be passed to any prop. A type that doesn't exist, a props comment that isn't directly above its macro, a parameter it doesn't declare or a prop the macro doesn't have, a required prop with a default, and a macro in `templates/components/` without a props comment are each `template/bad-header`:

```jinja fail template/bad-header templates/components/row.html
{# props: href: uri, title: string, date?: date, note?: string #}
{% macro row(href, title, date=none, note="") %}
<li><a href="{{ href }}">{{ title }}</a></li>
{% endmacro %}
```

## What a Component reads

A Component's body reads only its props and what every Page sees: `site`, `data`, `collections`, `taxonomies` and `page`. Reading anything else, such as `entry` or `entries`, is `template/undeclared-input`, because it would work on the Pages that have it and fail on the rest:

```jinja fail template/undeclared-input templates/components/row.html
{# props: href: url, title: string, date?: date, note?: string #}
{% macro row(href, title, date=none, note="") %}
<li><a href="{{ href }}">{{ title }}</a> {{ entries | length }}</li>
{% endmacro %}
```

Declare the value as a prop and pass it in. A Component file may import other Components at its top, and its body may call them.

## Slots and `{% call %}`

A Component that calls `caller()` takes a body, which a `{% call %}` passes in. The body is filled in the calling Template, so it reads that Template's values:

```jinja check templates/components/card.html
{# props: title: string, attrs?: attrs #}
{% macro card(title, attrs={}) %}
<section class="card"{{ attrs | attrs }}>
  <h2>{{ title }}</h2>
  {{ caller() }}
</section>
{% endmacro %}
```

```jinja check templates/posts/entry.html
{% from "components/card.html" import card %}
<!doctype html>
<html lang="en">
<title>{{ entry.title }}</title>
{% call card(title=entry.title, attrs={"id": entry.slug, "data-draft": entry.draft}) %}
  {{ entry.content }}
{% endcall %}
</html>
```

`attrs` here sets `id`, and leaves out `data-draft` while it is `false`. A Component that calls `caller()` needs a `{% call %}`, unless it tests `{% if caller %}` first, and one that never calls it takes none; a call that gets this wrong is `template/bad-prop`.

For a second piece of HTML, declare a `slot` prop and pass it a `{% set %}` block:

```jinja check templates/components/header.html
{# props: title: string, actions?: slot #}
{% macro header(title, actions="") %}
<header><h1>{{ title }}</h1>{{ actions }}</header>
{% endmacro %}
```

The built-in `pages` Collection's Entries, in `content/pages/`, use `pages/entry.html`:

```md check content/pages/about.md
---
title: About
---

About this Site.
```

```jinja check templates/pages/entry.html
{% from "components/header.html" import header %}
{% set actions %}<a href="{{ url('/blog/') }}">Blog</a>{% endset %}
<!doctype html>
<html lang="en">
<title>{{ entry.title }}</title>
{{ header(title=entry.title, actions=actions) }}
{{ entry.content }}
</html>
```

## Partials and Components

- A Partial, such as `{% include "partials/nav.html" %}`, takes no inputs and reads everything the including Template sees. It is checked as part of each Template that includes it.
- A Component takes only the props it declares, plus what every Page sees, and its body is checked once, on its own. Each call is checked against its props.

Use a Partial for one piece of a Page that reads the Page's values, and a Component for markup reused with different values. A macro in a Template outside `templates/components/` needs no props comment, and only its parameters are checked.

## Sidecars

A Sidecar is a stylesheet or Script with the same name as a Template, beside it: `row.css` and `row.ts` beside `components/row.html`. A `.css`, `.js`, `.mjs`, `.ts` or `.mts` file in `templates/` is never read as a Template.

```css check templates/components/row.css
li time {
  color: gray;
}
```

- **A Sidecar stylesheet**: yuanme bundles the Sidecar stylesheets of every Template that any Page uses into one stylesheet, and links it from every HTML Page, after its last `<link rel="stylesheet">` or else just before `</head>`. yuanme doesn't scope Sidecar styles to their Template yet, so a Sidecar's rules apply to every Page.
- **A Sidecar script**: yuanme bundles it as a module and loads it, from the same place, on each Page that uses its Template, and calls its exported `mount` on each of those Page loads (`yuanme docs scripts`).

A Page with neither a `<link rel="stylesheet">` nor a `</head>` gets no Sidecars.

A Page uses its Route's Template and every Template that one extends, includes or imports, at any depth, which is why Template names are plain strings. `yuanme docs styles` and `yuanme docs scripts` cover stylesheets and Scripts.
