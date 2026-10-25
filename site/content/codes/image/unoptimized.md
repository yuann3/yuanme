---
title: "image/unoptimized"
description: "A Page loads a raster image over 50 KB as it is, at full size and in its own format: a Static file, an image a stylesheet's `url()` or a `<link>` names, or an animated GIF, WebP or PNG."
---

A Page loads a raster image over 50 KB as it is, at full size and in its own format: a Static file, an image a stylesheet's `url()` or a `<link>` names, or an animated GIF, WebP or PNG. Only an `image` field, `image()` and a body image are resized and converted. `yuanme build --strict` makes this warning an error.

## Example

```
<img src="/photos/sea.jpg" alt="The sea">
(where static/photos/sea.jpg is 400 KB)
```

## Fix

Move the image out of `static/` and write it with `image()` or as a body image, which make smaller copies of it. Make an animated image a video, or smaller. List an image that must be served as it is, such as a favicon, in `[images] allow_unoptimized`.
