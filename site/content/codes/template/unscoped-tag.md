---
title: "template/unscoped-tag"
description: "A Template with a Sidecar stylesheet writes a tag that yuanme can't scope, or can't check, as written."
---

A Template with a Sidecar stylesheet writes a tag that yuanme can't scope, or can't check, as written. A tag whose name is built with `{{ }}`, such as `<h{{ level }}>`, gets the scope attribute, but no type selector can be checked against it. A tag whose name `{% if %}` chooses gets none. A tag whose end yuanme never finds, because its quotes are unbalanced in each branch of an `{% if %}`, leaves every tag after it without one. `check` warns; the Page still builds.

## Example

```
<{% if entry.featured %}h1{% else %}h2{% endif %}>{{ entry.title }}</h1>
(in a Template with a Sidecar)
```

## Fix

Write each tag whole once per branch, `{% if entry.featured %}<h1>…</h1>{% else %}<h2>…</h2>{% endif %}`, and close quotes in the branch that opens them. A `{{ }}` that prints attributes needs a space after the tag name: `<a {{ attrs | attrs }}>`.
