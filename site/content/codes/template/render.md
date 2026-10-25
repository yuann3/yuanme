---
title: "template/render"
description: "A Template failed while rendering a Page: it printed a value that is absent, reached through one, or did something its values don't allow."
---

A Template failed while rendering a Page: it printed a value that is absent, reached through one, or did something its values don't allow. `{% if entry.hero %}` on an absent field is false, but printing it or reading `entry.hero.src` is an error.

## Example

```
<img src="{{ entry.hero }}">
(on an Entry without `hero`)
```

## Fix

Guard the optional value with `{% if %}` or `| default(...)`, or correct the name if it is a typo.
