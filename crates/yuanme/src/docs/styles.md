---
title: Styles
description: Link stylesheets from a Template; yuanme bundles their imports, adds prefixes for your browsers, minifies and hashes them, and copies Static files as they are.
---

A Page gets a stylesheet the way any HTML page does: its Template has a `<link rel="stylesheet" href>` that names a CSS file of the project. yuanme bundles that file, minifies it, writes it under `/assets/` with a hash in its name, and rewrites the `href`. You write plain CSS; nothing needs to be installed.

## Link a stylesheet

Keep stylesheets anywhere in the project outside `static/`, such as `styles/`:

```css check styles/site.css
body {
  font-family: system-ui, sans-serif;
  line-height: 1.5;
}
```

Name it from the `<head>` of a Template by its path from the project root, with a leading `/`:

```jinja check templates/posts/entry.html
<!doctype html>
<html lang="en">
<head>
<title>{{ entry.title }}</title>
<link rel="stylesheet" href="/styles/site.css">
</head>
<h1>{{ entry.title }}</h1>
{{ entry.content }}
</html>
```

With a Post to render it:

```md check content/posts/hello.md
---
title: Hello
---

Some text.
```

the built Page, `dist/blog/hello/index.html`, has `<link rel="stylesheet" href="/assets/site-0123456789abcdef.css">` instead, and `dist/assets/` holds the bundle.

How yuanme reads a URL in a `<link href>`, a `<script src>`, an `@import` or a `url()`:

- A path from `/` names a file from the project root, such as `/styles/site.css` for `styles/site.css`, or a Static file without its `static/`, such as `/favicon.svg` for `static/favicon.svg`.
- Any other path is relative to where it is written: to the Page's URL in a Template, and to the stylesheet's own file in CSS. In Templates, always start from `/`: a Page at `/blog/hello/` would look for `href="styles/site.css"` in `/blog/hello/styles/`.
- A URL of another origin, a `data:` URL and a `#fragment` alone are left as written.
- A `?query` or `#fragment` after a path is kept on the rewritten URL.

yuanme reads the rendered Page, so an `href` written with an expression works too. A `<link rel="preload" as="style">` of a stylesheet gets the same bundle as its `<link rel="stylesheet">`.

## Bundling and `@import`

Each stylesheet a Page links becomes one file, with every `@import` of a project file inlined where it stands. A relative `@import` is relative to the importing stylesheet:

```css check styles/parts/type.css
h1,
h2 {
  font-weight: 600;
  text-wrap: balance;
}
```

```css check styles/site.css
@import "parts/type.css";

body {
  font-family: system-ui, sans-serif;
  line-height: 1.5;
}
```

An `@import` may also start from `/`, such as `@import "/styles/parts/type.css";`. Media queries and layers on an `@import`, such as `@import "print.css" print;`, are kept around the inlined rules. An `@import` of another origin, or of a Static file, stays an `@import` of that URL.

### Packages

`@import "pkg/file.css"` looks for a project file relative to the stylesheet first, and then for a package in the project's `node_modules`, through the `style` condition of the package's `exports`, or else its `style` field. `.css` may be left out.

```css
@import "@fontsource/geist";
@import "nprogress/nprogress.css";
```

The fonts and images such a package's CSS names are copied into `/assets/` like your own. Install packages with `bun install`; yuanme reads `node_modules` and never runs a package manager. A project with a `package.json` and no `node_modules` reports `asset/dependencies-not-installed`.

## `url()`

A `url()` that names a project file copies that file to `/assets/` under its own hash, and the bundle names the copy. A `url()` of a Static file keeps its URL from the root. Here `fonts/geist.woff2` is a file of the project:

```css
@font-face {
  font-family: Geist;
  src: url("../fonts/geist.woff2") format("woff2");
}
```

Outside `static/`, yuanme reads only files it knows: `.css`, Templates, Scripts, Data files, the images `.avif`, `.gif`, `.jpeg`, `.jpg`, `.png`, `.webp`, `.svg` and `.ico`, and the fonts `.woff`, `.woff2`, `.ttf` and `.otf`. Put any other file a stylesheet names in `static/`.

A `<link>` that isn't a stylesheet, such as `<link rel="preload" as="font" href="/fonts/geist.woff2" crossorigin>` or an icon, also gets the hashed copy when it names a project file, the same one the `url()` gets.

## Browser targets

`[build.targets]` in Site config gives the oldest version of each browser the Site supports. yuanme adds the vendor prefixes and fallbacks those browsers need and drops the ones none of them need. Without it, the targets are Chrome 111, Firefox 128 and Safari 16.4.

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[build.targets]
chrome = "108"
safari = "15.4"
```

Versions are strings, such as `"16.4"`. `safari` covers Safari on macOS and on iOS. The other keys are `edge`, `firefox`, `opera` and `samsung`; a browser left out gets no prefixes or fallbacks of its own. With the targets above, this CSS:

```css
.hero { height: 100vh; height: 100dvh; }
.card { user-select: none; }
```

is written as `.hero{height:100dvh}.card{-webkit-user-select:none;user-select:none}`: every target supports `dvh`, so the fallback goes, and Safari still needs the prefix. Scripts are compiled for the same targets.

## Minified and hashed

Every bundle is minified; there is no setting for it. Its name is its file name with the first 16 hex digits of its hash, such as `assets/site-0123456789abcdef.css`, so the name changes whenever its bytes do, including the names of the files it refers to. The host configuration yuanme writes caches `/assets/` as immutable, where the host allows it; `yuanme docs deploying` shows how.

## Sidecar stylesheets

A Sidecar stylesheet is a `.css` file with the same name as a Template, beside it:

```jinja check templates/partials/byline.html
<p class="byline">Written by the Author.</p>
```

```css check templates/partials/byline.css
.byline {
  color: gray;
  font-size: 0.9em;
}
```

```jinja check templates/posts/entry.html
<!doctype html>
<html lang="en">
<head>
<title>{{ entry.title }}</title>
<link rel="stylesheet" href="/styles/site.css">
</head>
<h1>{{ entry.title }}</h1>
{% include "partials/byline.html" %}
{{ entry.content }}
</html>
```

You don't link a Sidecar. yuanme puts the Sidecars of every Template that some Page rendered into one stylesheet, `sidecars.css`, and links it on every Page: just after the Page's last `<link rel="stylesheet">`, or else just before `</head>`. A Page with neither gets no Sidecar stylesheet. A Sidecar whose Template no Page renders is left out.

### Scoped to its Template

A Sidecar's rules apply only to the tags written in its own Template's file. yuanme adds one attribute, made from the Sidecar's path, to every start tag of the Template before it renders anything, and the same attribute to each compound selector of the Sidecar. The Page above gets `<p data-y-q454u2 class="byline">`, and `sidecars.css` has `.byline[data-y-q454u2]`, so a `.byline` anywhere else on the Site keeps its own styles. Only Templates with a Sidecar get the attribute.

Out of a Sidecar's reach are:

- tags another Template writes: a Layout it extends, a Partial it includes, a Component it calls. Give that Template its own Sidecar.
- the body of a `{% call %}`, and a `{% block %}` filled in for a Layout: they belong to the Template they are written in, so that Template's Sidecar styles them, not the Component's or the Layout's.
- what Templates print rather than write: an Entry's `entry.content`, the output of `| markdown` or `| org`, and strings printed with `| safe`.
- `<head>` and what is in it: `meta`, `title`, `link`, `style` and `script` never get the attribute, and nor do `base`, `noscript` and `template`. `<html>` and `<body>` do.

`:global()` reaches outside on purpose. What is inside it is left as written, and the rest is scoped:

```css check templates/partials/byline.css
.byline {
  color: gray;
  font-size: 0.9em;
}

.byline :global(a) {
  color: inherit;
}

:global(.dark) .byline {
  color: silver;
}
```

Here `.byline :global(a)` becomes `.byline[data-y-q454u2] a`, which reaches a link printed into the byline, and `:global(.dark) .byline` becomes `.dark .byline[data-y-q454u2]`.

These stay as written: `:root`; `@keyframes` names, which Scripts read, and container and layer names; and every rule inside `@scope`. Native `@scope` is how a Sidecar opts out of the attribute, for a Site whose targets all support it; yuanme can't convert it for older browsers, where its rules do nothing. A `.css` file a Sidecar `@import`s is not scoped.

### What `check` warns about

A selector that no tag of the Template can match is `asset/unmatched-selector`: here `byline.html` writes no `<a>`, so `.byline a` reaches nothing.

```css fail asset/unmatched-selector templates/partials/byline.css
.byline a {
  color: inherit;
}
```

`check` compares a selector's type, classes and id with each tag; attributes and pseudo-classes are left out, since Scripts set them, and a class printed with `{{ }}` could be any class.

A tag whose name is built with `{{ }}` gets the attribute after the expression, but `check` can't match type selectors against it, so it is `template/unscoped-tag`:

```jinja fail template/unscoped-tag templates/partials/byline.html
{% set level = 3 %}
<h{{ level }} class="byline">Written by the Author.</h{{ level }}>
```

So is a tag whose name `{% if %}` chooses, which gets no attribute, and a tag whose quotes are unbalanced in each branch of an `{% if %}`, after which no tag gets one. Write the whole tag once in each branch instead.

## Static files

A file in `static/` is a Static file: yuanme copies it to the same path on the Site, without `static/`, unchanged and unhashed. Files that Git ignores, and names starting with `.` other than `.well-known`, are left out. `static/robots.txt` is served at `/robots.txt`:

```txt check static/robots.txt
User-agent: *
Allow: /
```

A stylesheet in `static/` is a Static file too: a `<link rel="stylesheet">` of it loads the file as it is, with no bundling, prefixes or hash. A URL can't name both a project file and a Static file:

```css fail asset/ambiguous-url static/styles/site.css
body { margin: 0 }
```

A Static file at the path of a Page is `entry/output-collision`, and one with the name of a file the Host adapter writes, such as `static/_headers`, is `host/output-collision`.

## Tailwind

yuanme doesn't run Tailwind yet.

## When something is wrong

A URL that names nothing is `asset/not-found`, with help naming the file you probably meant:

```jinja fail asset/not-found templates/posts/entry.html
<!doctype html>
<html lang="en">
<head>
<title>{{ entry.title }}</title>
<link rel="stylesheet" href="/styles/sit.css">
</head>
{{ entry.content }}
</html>
```

CSS that can't be parsed is `asset/invalid-css`, located in the file it is in, even an imported one:

```css fail asset/invalid-css styles/parts/type.css
h1 { font-weight: 600 }

a$b { color: red }
```

The codes for stylesheets and the files they name:

- `asset/not-found`: a `<link href>`, `@import` or `url()` names no file of the project, no Static file and no URL of the Site.
- `asset/ambiguous-url`: a URL names both a project file and a Static file.
- `asset/invalid-css`: a stylesheet isn't valid CSS or UTF-8, imports one file twice in ways that can't be merged, or is a Sidecar with `:local()`, or with a `:global` that has no parentheses, is empty or holds a list.
- `asset/dependencies-not-installed`: an `@import` of a package, with a `package.json` and no `node_modules`.
- `asset/unmatched-selector`: a Sidecar's selector matches no tag its Template writes. A warning.
- `asset/scope-collision`: two Sidecars' paths make the same attribute. Rename one of them.

`yuanme docs CODE` explains each one.
