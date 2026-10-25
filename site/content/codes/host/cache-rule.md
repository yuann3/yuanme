---
title: "host/cache-rule"
description: "A `[[host.headers]]` rule changes `Cache-Control` where yuanme's cache policy has to hold: on a hashed file in `/assets/`, which must stay `immutable`, or on a Page, which must revalidate with `max-age=0, must-revalidate` so that a deploy shows at once."
---

A `[[host.headers]]` rule changes `Cache-Control` where yuanme's cache policy has to hold: on a hashed file in `/assets/`, which must stay `immutable`, or on a Page, which must revalidate with `max-age=0, must-revalidate` so that a deploy shows at once.

## Example

```
[[host.headers]]
path = "/*"
headers = { "Cache-Control" = "public, max-age=3600" }
```

## Fix

Narrow the rule's `path` to the files it is for, such as `/resume.pdf`.
