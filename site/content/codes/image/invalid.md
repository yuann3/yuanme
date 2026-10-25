---
title: "image/invalid"
description: "An image can't be processed: it isn't an image yuanme reads, it is damaged, it is over 100 megapixels, or it is a Static file, which is copied as it is."
---

An image can't be processed: it isn't an image yuanme reads, it is damaged, it is over 100 megapixels, or it is a Static file, which is copied as it is. yuanme resizes JPEG, PNG, WebP and GIF, and copies SVG, AVIF and animated images as they are.

## Example

```
![Sales by month](plot.png)
(where plot.png is really a PDF)
```

## Fix

Export the image again as JPEG or PNG, under 100 megapixels. Move an image out of `static/` to have it processed, or name a Static file by its URL, without `static/`.
