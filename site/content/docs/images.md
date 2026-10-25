---
title: "Images"
description: "How Entry bodies name images, what yuanme makes of them, the image field type, and the [images] settings."
---

yuanme resizes and converts the images in Entry bodies: each becomes a set of smaller AVIF and JPEG files, and the Page loads the one that fits the screen. Templates have no image helper yet; an `<img>` in a Template names a Static file.

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

Each output is kept in `.yuanme/cache`, so only a new or changed image is encoded again. `yuanme check` encodes nothing.

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
- a path from the project root, starting with `/`, such as `/static/img/cat.svg`;
- an `http://` or `https://` URL, which isn't checked.

```xml check static/img/cat.svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="7"/></svg>
```

```md check content/posts/trip.md
---
title: Trip
cover: /static/img/cat.svg
---

Some text.
```

yuanme checks that a path names a file of the project. One that doesn't is `entry/invalid-value`:

```md fail entry/invalid-value content/posts/trip.md
---
title: Trip
cover: cat.svg
---
```

A Template sees the field as text: the file's path from the project root, such as `static/img/cat.svg`, or the URL as written. yuanme doesn't process the image a field names yet, so keep it in `static/` and drop `static/` from the path to make its URL:

```jinja check templates/posts/entry.html
<!doctype html>
<html lang="en">
<title>{{ entry.title }}</title>
{% if entry.cover %}
<img src="/{{ entry.cover | replace('static/', '') }}" alt="" width="16" height="16">
{% endif %}
<h1>{{ entry.title }}</h1>
{{ entry.content }}
</html>
```

Give every `<img>` in a Template its `width` and `height`; yuanme doesn't add them. An `<img src>` that names no Page, Static file or asset of the build is a `link/broken` warning. For a remote URL, write `{{ entry.cover }}` as it is. A Component prop can have the type `image` too; `yuanme docs components` shows props.

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
```

- `widths`: the widths each image is resized to. These are the defaults.
- `sizes`: how wide images are shown, which picks the width a browser fetches. It defaults to `100vw`, the whole screen; set it to your text's width, as here, and phones fetch smaller files.
- `formats`: what is offered before the JPEG. Every browser yuanme targets by default reads AVIF; add `"webp"` for older ones, at the cost of encoding every image once more.
- `avif_speed` and `quality`: each encoder's own settings, from 1 to 10 and from 1 to 100. A lower speed makes smaller AVIFs, slowly.

## Images in stylesheets

A `url()` in a stylesheet that names an image elsewhere in the project, such as `url("../images/grain.png")`, copies it to `/assets/` with a hash in its name, and the stylesheet names the copy. A `url()` of a Static file keeps its URL. `yuanme docs styles` covers stylesheets.

Outside `static/`, yuanme reads only the image types `.avif`, `.gif`, `.jpeg`, `.jpg`, `.png`, `.webp`, `.svg` and `.ico`. In `static/`, any file is copied.
