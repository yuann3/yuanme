---
title: "asset/ambiguous-url"
description: "A URL names both a project file, which yuanme processes and hashes, and a Static file, which it copies unchanged: `/styles/site.css` is both `styles/site.css` and `static/styles/site.css`."
---

A URL names both a project file, which yuanme processes and hashes, and a Static file, which it copies unchanged: `/styles/site.css` is both `styles/site.css` and `static/styles/site.css`.

## Example

```
<link rel="stylesheet" href="/styles/site.css">
(with both styles/site.css and static/styles/site.css)
```

## Fix

Delete or rename one of the two files.
