---
title: "entry/duplicate-data-file"
description: "Two Data files share a name, such as `data/nav.toml` and `data/nav.yaml`, so `data.nav` would be ambiguous."
---

Two Data files share a name, such as `data/nav.toml` and `data/nav.yaml`, so `data.nav` would be ambiguous.

## Example

```
data/nav.toml and data/nav.yaml
```

## Fix

Delete or rename one of them.
