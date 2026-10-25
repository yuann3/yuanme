---
title: "host/over-limit"
description: "Site config has more header rules or redirects, or a longer line, than a host yuanme supports accepts."
---

Site config has more header rules or redirects, or a longer line, than a host yuanme supports accepts. The limits are the strictest host's, Cloudflare's, whatever the adapter, so a Site can move hosts: 100 header rules, of which yuanme's cache rules take 2, 2,000 redirects, and 2,000 characters on a line of `_headers` or `_redirects`.

## Example

```
101 [[host.headers]] rules
```

## Fix

Merge rules: one rule with a `*` in its path, such as `/fonts/*`, covers many files.
