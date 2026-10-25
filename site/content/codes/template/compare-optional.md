---
title: "template/compare-optional"
description: "A Template compares an optional field with `==`, `!=`, `<`, `in` or another comparison without a guard."
---

A Template compares an optional field with `==`, `!=`, `<`, `in` or another comparison without a guard. Comparing an absent value is an error, even where testing it with `{% if %}` is not.

## Example

```
{% if entry.website != entry.github %}…{% endif %}
(with `github` optional)
```

## Fix

Guard it first, `{% if entry.github and entry.website != entry.github %}`, or compare a default: `entry.website != entry.github | default("")`.
