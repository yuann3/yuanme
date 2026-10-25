---
title: "entry/output-collision"
description: "Two Routes, or a Route and a Static file, would write the same file of the Site."
---

Two Routes, or a Route and a Static file, would write the same file of the Site.

## Example

```
[collections.posts] route = "/{slug}/" and content/pages/about.md, with content/posts/about.md
```

## Fix

Change one Collection's `route`, or rename one of the files.
