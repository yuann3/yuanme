---
title: "image/missing-alt"
description: "An image in an Entry's body has no alt text, so a screen reader can only say that an image is there."
---

An image in an Entry's body has no alt text, so a screen reader can only say that an image is there. In Org, the file's name, which Emacs writes when nothing else is given, counts as none.

## Example

```
![](plot.png)
```

## Fix

Describe what the image shows: `![Sales by month, rising through June](plot.png)`. In Org, write a `#+CAPTION:` or `#+ATTR_HTML: :alt …` line above the link.
