---
title: "template/syntax"
description: "A Template is not valid Jinja, so no Page that uses it can render."
---

A Template is not valid Jinja, so no Page that uses it can render.

## Example

```
{% if entry.draft %}Draft
```

## Fix

Fix the syntax at the reported place. Here the `if` needs its `{% endif %}`.
