---
title: "image/remote"
description: "`image()` is given a remote URL."
---

`image()` is given a remote URL. yuanme never fetches a remote image, so it can't know its size or make smaller copies of it. A remote image in an Entry's body, or in an `<img>`, is left as it is.

## Example

```
{{ image("https://example.com/cat.jpg") | picture(alt="A cat") }}
```

## Fix

Save the image in the project and pass its path, or write an `<img>` with the URL, `width` and `height`.
