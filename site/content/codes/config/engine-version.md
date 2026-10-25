---
title: "config/engine-version"
description: "The project needs a different version of yuanme than the one running: `yuanme = \"X.Y\"` is the oldest compatible version, with Cargo's caret meaning, so 1.2 accepts any 1.x from 1.2 on and 0.3 accepts only 0.3.x."
---

The project needs a different version of yuanme than the one running: `yuanme = "X.Y"` is the oldest compatible version, with Cargo's caret meaning, so 1.2 accepts any 1.x from 1.2 on and 0.3 accepts only 0.3.x.

## Example

```
yuanme = "9.0"

[site]
url = "https://example.com"
```

## Fix

Install a version of yuanme that the project accepts, with `yuanme self update X.Y.Z`, or change `yuanme = ` if the project really works with the version you have.
