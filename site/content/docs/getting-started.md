---
title: "Getting started"
description: "Install yuanme, make a Site with one Post, check it and build it."
---

yuanme builds a static Site from Markdown and Org files. A project is a folder with `yuanme.toml` at its root. This page makes one by hand, with a blog of Posts, and builds it.

## Install

Until the first release, install from the repository with Rust's `cargo`:

```sh
cargo install --locked --git https://github.com/yuann3/yuanme yuanme
```

## Start from a Starter

`yuanme init my-site` makes a project in `my-site/` from the `blog` Starter: Posts in Markdown and Org with math and code, projects, tags, an about Page and an RSS feed, ready to build, with the files a coding agent reads. `yuanme init my-site --starter minimal` makes one with a Layout, a home Page and nothing else, to design your own. Then:

```sh
cd my-site
yuanme dev
```

`yuanme dev` serves the Site and rebuilds it on every save. The first `check` or `build` of the `blog` Starter downloads TypeScript's `tsc`, about 9 MB, to check the types of its copy button's Script; `yuanme docs scripts` says how to check offline. `yuanme docs cli` shows what `init` writes. The rest of this page makes a project by hand instead.

## Site config

`yuanme.toml` is the Site config. It declares the Site and its Collections: here one Collection, `posts`, whose Entries are the files in `content/posts/`.

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"
```

`route` is the URL of each Post, with `{slug}` where its Slug goes; `list_route` is the page that lists them. `yuanme docs config` lists every key.

## A Post

An Entry is one file in a Collection's folder, in Markdown or Org. Its Slug comes from the file name, so `content/posts/hello.md` is served at `/blog/hello/`.

```md check content/posts/hello.md
---
title: Hello
date: 2025-12-12
---

My first Post, with some *emphasis* and a [link](https://commonmark.org).
```

Every Entry needs a `title`. `yuanme docs collections` shows the other fields, and how a Collection declares its own.

## Templates

A Template is a Jinja file in `templates/` that the Engine fills to make a Page. Each Route has a default one: an Entry of `posts` uses `templates/posts/entry.html`, and its list `templates/posts/list.html`.

```jinja check templates/posts/entry.html
<!doctype html>
<html lang="en">
<title>{{ entry.title }}</title>
<h1>{{ entry.title }}</h1>
{{ entry.content }}
</html>
```

```jinja check templates/posts/list.html
<!doctype html>
<html lang="en">
<title>Blog</title>
<ul>
{% for post in entries %}
  <li><a href="{{ post.url }}">{{ post.title }}</a></li>
{% endfor %}
</ul>
</html>
```

`yuanme docs templates` covers what each kind of Page sees.

## Check and build

```sh
yuanme check
yuanme build
```

`check` reports every Diagnostic the build would, without writing anything. `build` writes the Site to `dist/`: here `dist/blog/hello/index.html` and `dist/blog/index.html`. Every file in it can be served as it is from any static host; `yuanme docs deploying` shows how.

`yuanme routes` lists every URL of the Site and the file and Template that make it.

## When something is wrong

A Diagnostic names its code, the file and the place in it, and usually how to fix it:

```md fail entry/unknown-key content/posts/hello.md
---
title: Hello
draf: true
---
```

`yuanme check` reports `error[entry/unknown-key]` here, with help to use `draft`. `yuanme docs entry/unknown-key` explains the code. Add `--json` to any command for one JSON object per line, which is what a coding agent should read.
