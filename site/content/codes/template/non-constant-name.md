---
title: "template/non-constant-name"
description: "`extends`, `include` or `import` names a Template with an expression."
---

`extends`, `include` or `import` names a Template with an expression. Template names are plain strings, so yuanme knows every Template a Page uses without rendering it, which is how a Page gets their Sidecars.

## Example

```
{% include "partials/" ~ page.kind ~ ".html" %}
```

## Fix

Name the Template by its path from `templates/`, such as `{% include "partials/entry.html" %}`, and choose between several with `{% if %}`.
