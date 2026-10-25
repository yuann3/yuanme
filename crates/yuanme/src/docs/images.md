---
title: Images
description: Where to keep the images Pages show, how Entry bodies and the image field name them, and what yuanme does with images today.
---

yuanme doesn't resize or convert images yet, and Templates have no image helper. A Page shows an image as the file you wrote, served from `static/`.

## Images in `static/`

Keep the images that Pages show in `static/`. A file there is a Static file: yuanme copies it to the same path on the Site, without `static/`, unchanged. `static/img/cat.svg` is served at `/img/cat.svg`:

```xml check static/img/cat.svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="7"/></svg>
```

A Template names it by that URL:

```jinja check templates/posts/entry.html
<!doctype html>
<html lang="en">
<title>{{ entry.title }}</title>
<img src="/img/cat.svg" alt="" width="16" height="16">
<h1>{{ entry.title }}</h1>
{{ entry.content }}
</html>
```

Give every `<img>` its `width` and `height`, so the Page doesn't move as the image loads; yuanme doesn't add them.

yuanme checks every URL a Page loads. An `<img src>` that names no Page, Static file or asset of the build is a `link/broken` warning. That includes an image elsewhere in the project, such as `images/cat.png`: yuanme doesn't copy a file an `<img>` names unless it is in `static/`.

## Images in Entry bodies

In Markdown, write an image with `![alt](url)`, and the URL of a Static file:

```md check content/posts/hello.md
---
title: Hello
---

![A black circle](/img/cat.svg)
```

The Page has `<img src="/img/cat.svg" alt="A black circle">`. In Org, a link to an image file, such as `[[/img/cat.svg]]`, is shown as the image; `yuanme docs org` covers Org links.

A relative URL in a body is relative to the Page's URL, `/blog/hello/` here, not to the Entry's file, and an image beside the Entry isn't copied. So keep the image in `static/` and start its URL from `/`:

```md fail link/broken content/posts/hello.md
---
title: Hello
---

![A black circle](cat.svg)
```

## The `image` field type

A Collection can declare a field of type `image`, such as a cover for each Post:

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"
fields = { cover = { type = "image", optional = true } }
```

Its value is one of:

- a path relative to the Entry's folder, such as `cover.png` for an image beside it;
- a path from the project root, starting with `/`, such as `/static/img/cat.svg`;
- an `http://` or `https://` URL, which isn't checked.

```md check content/posts/hello.md
---
title: Hello
cover: /static/img/cat.svg
---

Some text.
```

yuanme checks that a path names a file of the project. One that doesn't is `entry/invalid-value`:

```md fail entry/invalid-value content/posts/hello.md
---
title: Hello
cover: cat.svg
---
```

A Template sees the field as text: the file's path from the project root, such as `static/img/cat.svg`, or the URL as written. yuanme doesn't copy or process the image a field names yet, so keep it in `static/` and drop `static/` from the path to make its URL:

```jinja check templates/posts/entry.html
<!doctype html>
<html lang="en">
<title>{{ entry.title }}</title>
{% if entry.cover %}
<img src="/{{ entry.cover | replace('static/', '') }}" alt="">
{% endif %}
<h1>{{ entry.title }}</h1>
{{ entry.content }}
</html>
```

For a remote URL, write `{{ entry.cover }}` as it is. A Component prop can have the type `image` too; `yuanme docs components` shows props.

## Images in stylesheets

A `url()` in a stylesheet that names an image elsewhere in the project, such as `url("../images/grain.png")`, copies it to `/assets/` with a hash in its name, and the stylesheet names the copy. A `url()` of a Static file keeps its URL. `yuanme docs styles` covers stylesheets.

Outside `static/`, yuanme reads only the image types `.avif`, `.gif`, `.jpeg`, `.jpg`, `.png`, `.webp`, `.svg` and `.ico`. In `static/`, any file is copied.
