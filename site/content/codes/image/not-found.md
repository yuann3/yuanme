---
title: "image/not-found"
description: "An image in an Entry's body, an `image` field or `image()` names a path where no image is."
---

An image in an Entry's body, an `image` field or `image()` names a path where no image is. In an Entry, a relative path starts from the Entry's folder, so in `content/posts/sea/index.md`, `![](plot.png)` is `content/posts/sea/plot.png`; a path starting with `/` starts from the project's root, as every path `image()` takes does.

## Example

```
![Sales by month](plt.png)
(with plot.png beside the Entry)
```

## Fix

Correct the path, here `plot.png`, or put the image where the path points.
