---
title: "template/unknown-field"
description: "A Template reads a field or a variable that isn't in the schema of what its Page sees: a typo, a field of another Collection, a value only another kind of Route has, such as `entries` on a Taxonomy's index, or a field of something that has none, such as a string."
---

A Template reads a field or a variable that isn't in the schema of what its Page sees: a typo, a field of another Collection, a value only another kind of Route has, such as `entries` on a Taxonomy's index, or a field of something that has none, such as a string. `check` finds it even on a path no Entry reaches, or one `| default` would hide.

## Example

```
<h1>{{ entry.titel }}</h1>
```

## Fix

Use the name the help suggests, here `entry.title`, or declare the field in the Collection's `[collections.<name>.fields]`.
