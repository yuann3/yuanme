---
title: "entry/invalid-value"
description: "A field's value has the right type but isn't allowed: an `enum` value not in its list, a URL that isn't absolute, an image that doesn't exist, or an item repeated in a `unique` list."
---

A field's value has the right type but isn't allowed: an `enum` value not in its list, a URL that isn't absolute, an image that doesn't exist, or an item repeated in a `unique` list.

## Example

```
---
title: Pew
kind: esay
---
(with kind = { type = "enum", values = ["essay", "note"] })
```

## Fix

Change the value as the message says. Here `kind: essay`.
