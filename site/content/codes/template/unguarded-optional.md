---
title: "template/unguarded-optional"
description: "A Template prints, loops over, filters or reaches through an optional field without a guard, so a Page whose Entry leaves it out fails to render."
---

A Template prints, loops over, filters or reaches through an optional field without a guard, so a Page whose Entry leaves it out fails to render. The guards `check` understands are `{% if %}`, its `else`, `and`, an inline `if`, `is defined`, a loop's `if`, and `| default(…)`; an early `{% continue %}` is not one.

## Example

```
<img src="{{ entry.hero.src }}">
(with `hero` optional)
```

## Fix

Guard it, `{% if entry.hero %}<img src="{{ entry.hero.src }}">{% endif %}`, or give it a value: `{{ entry.subtitle | default("") }}`.
