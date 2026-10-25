---
title: "config/invalid-value"
description: "A value in Site config has the right type but means nothing to yuanme: a URL that isn't absolute, a Route without its placeholder, an unknown field type, or a name that refers to nothing."
---

A value in Site config has the right type but means nothing to yuanme: a URL that isn't absolute, a Route without its placeholder, an unknown field type, or a name that refers to nothing.

## Example

```
[collections.posts]
route = "/blog/"
```

## Fix

Change the value as the message says. Here an Entry's Route needs `{slug}`: `route = "/blog/{slug}/"`.
