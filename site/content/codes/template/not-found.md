---
title: "template/not-found"
description: "A Template that a Route renders, or that another Template extends, includes or imports, is not in `templates/`."
---

A Template that a Route renders, or that another Template extends, includes or imports, is not in `templates/`. Each Route has one default Template, such as `posts/entry.html` for an Entry of `posts`, unless Site config names another with `template`.

## Example

```
[collections.posts]
route = "/blog/{slug}/"
(with no templates/posts/entry.html)
```

## Fix

Create the file the message names, or set `template` to a Template that exists. Templates name each other by their path from `templates/`.
