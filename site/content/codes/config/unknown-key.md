---
title: "config/unknown-key"
description: "Site config has a key yuanme doesn't know."
---

Site config has a key yuanme doesn't know. Unknown keys are errors, not ignored, so a typo can't silently do nothing. Only `[site.extra]` takes keys of the Author's own.

## Example

```
[site]
url = "https://example.com"
titel = "My site"
```

## Fix

Use the key the help suggests, such as `title`, or move a key of your own into `[site.extra]`.
