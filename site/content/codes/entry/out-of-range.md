---
title: "entry/out-of-range"
description: "A field's value is outside the `min` and `max` its Collection declares: a string's length in characters, or a number."
---

A field's value is outside the `min` and `max` its Collection declares: a string's length in characters, or a number.

## Example

```
---
title: Pew
order: 9
---
(with order = { type = "int", max = 5 })
```

## Fix

Use a value inside the range, or change the range in `yuanme.toml`.
