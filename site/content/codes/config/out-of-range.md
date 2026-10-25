---
title: "config/out-of-range"
description: "A number in Site config is outside the range its key allows."
---

A number in Site config is outside the range its key allows.

## Example

```
[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"
paginate = 0
```

## Fix

Use a number inside the range the message names. Here `paginate` is at least 1.
