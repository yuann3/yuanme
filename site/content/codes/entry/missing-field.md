---
title: "entry/missing-field"
description: "An Entry, or an object inside one, leaves out a field that is required: `title`, or a declared field with neither `optional = true` nor a `default`."
---

An Entry, or an object inside one, leaves out a field that is required: `title`, or a declared field with neither `optional = true` nor a `default`.

## Example

```
---
date: 2025-12-12
---
```

## Fix

Add the field, here `title: Pew`, or make it optional in `yuanme.toml`.
