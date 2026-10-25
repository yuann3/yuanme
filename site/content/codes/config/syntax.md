---
title: "config/syntax"
description: "`yuanme.toml` is not valid TOML, so nothing else in it can be read."
---

`yuanme.toml` is not valid TOML, so nothing else in it can be read.

## Example

```
[site]
url = https://example.com
```

## Fix

Fix the TOML at the reported place. Here the string needs quotes: `url = "https://example.com"`.
