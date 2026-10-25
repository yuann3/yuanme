---
title: "config/missing-key"
description: "Site config leaves out a key that has no default."
---

Site config leaves out a key that has no default.

## Example

```
[site]
title = "My site"
```

## Fix

Add the key. Here every Site needs its address: `url = "https://example.com"`.
