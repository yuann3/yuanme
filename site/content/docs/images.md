---
title: "Images"
description: "How Entry bodies name images, what yuanme makes of them, the image field type, and the [images] settings."
---

yuanme resizes and converts the images in Entry bodies and the ones Templates pass to `image()`: each becomes a set of smaller AVIF and JPEG files, and the Page loads the one that fits the screen.

## Images in Entry bodies

In Markdown, write an image with `![alt](path)`. In Org, a link to an image file, such as `[[file:cat.svg]]`, is shown as the image; `yuanme docs org` covers Org links.

A relative path starts from the Entry's file. Give an Entry its own folder, with its text in `index.md` and its images beside it:

```xml check content/posts/hello/cat.svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="7"/></svg>
```

```md check content/posts/hello/index.md
---
title: Hello
---

![A black circle](cat.svg)
```

`content/posts/hello/index.md` is the Post `hello`, and `cat.svg` beside it is the image. A path starting with `/` starts from the project root instead, such as `/images/cat.svg` for `images/cat.svg`. A path that names no file is `image/not-found`:

```md fail image/not-found content/posts/hello/index.md
---
title: Hello
---

![A black circle](dog.svg)
```

### What a body image becomes

A JPEG, PNG, WebP or GIF is resized to each of `[images] widths` and encoded as AVIF and as JPEG, or PNG for an image that can be transparent. Nothing is made wider than the image itself. The Page gets a `<picture>`:

```html
<picture><source type="image/avif" srcset="/assets/beach-480-….avif 480w, /assets/beach-800-….avif 800w" sizes="100vw"><img src="/assets/beach-800-….jpg" srcset="/assets/beach-480-….jpg 480w, /assets/beach-800-….jpg 800w" sizes="100vw" width="800" height="533" alt="A beach at noon" loading="eager" decoding="async"></picture>
```

- `width` and `height` keep the Page from moving as the image loads.
- The first image in a body is `loading="eager"`, since it may be the largest thing on the screen; the others are `lazy`.
- The files are named by a hash of everything they depend on, so a changed image or setting gets new names, and a browser never keeps an old one.

Each output is kept in `.yuanme/cache`, so only a new or changed image is encoded again. `yuanme check` encodes nothing, and `yuanme dev` encodes an output only when a browser first asks for it.

An SVG, an AVIF and an animated GIF, WebP or PNG are copied into `/assets/` as they are, with a hash in the name. The Page above gets `<img src="/assets/cat-….svg" width="16" height="16" alt="A black circle" loading="eager" decoding="async">`; an SVG's size comes from its `width` and `height`, or else its `viewBox`.

A remote image, such as `![A cat](https://example.com/cat.jpg)`, is left as it is, and so is the URL of a Static file: `![A circle](/img/circle.svg)` names `static/img/circle.svg` when the project has no `img/circle.svg`.

### Alt text

The alt text describes the image for someone who can't see it. A body image without it is the warning `image/missing-alt`:

```md fail image/missing-alt content/posts/hello/index.md
---
title: Hello
---

![](cat.svg)
```

In Org, a `#+CAPTION` gives the alt text, or `#+ATTR_HTML: :alt` does. Emacs writes the file's name when neither is there, and yuanme warns about that as it does about no alt text:

```text check content/posts/hello-org.org
#+title: Hello from Org

#+CAPTION: A black circle
[[file:hello/cat.svg]]
```

A file that isn't an image yuanme reads, or is damaged, or is over 100 megapixels, is `image/invalid`.

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
- a path from the project root, starting with `/`, such as `/images/cat.svg`;
- an `http://` or `https://` URL, which isn't checked.

```md check content/posts/hello/index.md
---
title: Hello
cover: cat.svg
---

![A black circle](cat.svg)
```

yuanme checks that a path names a file of the project. One that doesn't is `image/not-found`:

```md fail image/not-found content/posts/hello/index.md
---
title: Hello
cover: dog.svg
---
```

A Template sees the field as text: the file's path from the project root, such as `content/posts/hello/cat.svg`, or the URL as written. Pass it to `image()`.

## Images in Templates

`image(src)` makes an image's smaller copies, as a body image gets, and `picture` writes the `<picture>` for them:

```jinja check templates/posts/entry.html
<!doctype html>
<html lang="en">
<head>
<title>{{ entry.title }}</title>
</head>
<body>
{% if entry.cover %}
{{ image(entry.cover, sizes="(min-width: 768px) 720px, 100vw") | picture(alt="", priority=true) }}
{% endif %}
<h1>{{ entry.title }}</h1>
{{ entry.content }}
</body>
</html>
```

`src` is a path from the project root, with a leading `/` or without, as an `image` field gives it. A remote URL is `image/remote`: yuanme never fetches an image, so it can't know its size. Write an `<img>` with its `width` and `height` for one.

`image()` takes these keyword arguments, each defaulting to its `[images]` setting:

- `widths`: the widths to make, such as `[800, 1600]`; or `width` for one.
- `formats`: the `<source>` formats before the fallback, `"avif"` and `"webp"`.
- `format`: one format for every file, with no `<source>`: `"avif"`, `"webp"`, `"jpeg"` or `"png"`.
- `quality`: from 1 to 100, for every format.
- `sizes`: how wide the image is shown.

It returns an Image, whose fields a Template can read: `width` and `height`, of `url`, the largest fallback file; `srcset` and `sizes`; and `sources`, each with a `type` and a `srcset`. `width=` with one `format=` makes one file, as `og:image` wants:

```jinja
{% if entry.cover %}
<meta property="og:image" content="{{ absolute_url(image(entry.cover, width=1200, format="jpeg").url) }}">
{% endif %}
```

`picture` takes:

- `alt`, which is required: what the image shows, or `alt=""` for one that is only decoration;
- `priority=true` for the image that is the largest on the screen as the Page opens, such as a cover: it isn't lazy, it gets `fetchpriority="high"`, and the Page's `<head>` gets a `<link rel="preload">` for it, so the browser fetches it first;
- `loading`, `"lazy"`, the default, or `"eager"`;
- `class`, and `attrs`, a map of more attributes for the `<img>`, as the `attrs` filter takes.

An SVG, an AVIF or an animated image is copied as it is, so its Image has `url` alone, with `width` and `height` when the file says. An image in `static/` is a Static file, copied as it is: `image()` of one is `image/invalid`, so keep the images you pass to it elsewhere. An `<img>` you write yourself is left alone; give it its `width` and `height`.

## Settings

`[images]` in Site config sets what every image becomes:

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"
fields = { cover = { type = "image", optional = true } }

[images]
widths = [480, 800, 1200, 1600, 2400]
sizes = "(min-width: 768px) 720px, 100vw"
formats = ["avif"]
avif_speed = 6
quality = { avif = 72, jpeg = 80, webp = 80 }
allow_unoptimized = ["/favicon.png"]
```

- `widths`: the widths each image is resized to. These are the defaults.
- `sizes`: how wide images are shown, which picks the width a browser fetches. It defaults to `100vw`, the whole screen; set it to your text's width, as here, and phones fetch smaller files.
- `formats`: what is offered before the JPEG. Every browser yuanme targets by default reads AVIF; add `"webp"` for older ones, at the cost of encoding every image once more.
- `avif_speed` and `quality`: each encoder's own settings, from 1 to 10 and from 1 to 100. A lower speed makes smaller AVIFs, slowly.
- `allow_unoptimized`: images over 50 KB that may be served as they are, below.

## Images served as they are

A raster image over 50 KB that a Page loads as it is, at full size, is the warning `image/unoptimized`: a Static file in an `<img>` or `<link>`, an image a stylesheet's `url()` or a `<link>` copies, or an animated GIF, WebP or PNG. A plain link, such as `<a href="/photo.jpg">`, loads nothing, so it doesn't warn. `yuanme build --strict` makes the warning an error.

Write such an image with `image()` or as a body image instead, which make smaller copies. List one that must stay as it is, such as a favicon, in `allow_unoptimized`, by the URL you name it by from the root: `/favicon.png` for `static/favicon.png`, and `/images/grain.png` for `images/grain.png`.

## Images in stylesheets

A `url()` in a stylesheet that names an image elsewhere in the project, such as `url("../images/grain.png")`, copies it to `/assets/` with a hash in its name, and the stylesheet names the copy. A `url()` of a Static file keeps its URL. `yuanme docs styles` covers stylesheets.

Outside `static/`, yuanme reads only the image types `.avif`, `.gif`, `.jpeg`, `.jpg`, `.png`, `.webp`, `.svg` and `.ico`. In `static/`, any file is copied.
