---
title: "entry/wrong-type"
description: "A field has a value of the wrong type, such as a number where text goes."
---

A field has a value of the wrong type, such as a number where text goes. YAML reads `2025` as a number and `yes` as text, so either may need quotes or rewriting.

## Example

```
---
title: 2025
---
```

## Fix

Write the value as the type the message names. Here `title: "2025"`.
