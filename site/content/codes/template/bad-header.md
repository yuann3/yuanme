---
title: "template/bad-header"
description: "A props comment on a Component, or a vars comment on a Layout, can't be read or doesn't match its macro: an entry that isn't `name: type` or `name?: type`, an unknown type, a props comment that isn't directly above a `{% macro %}`, a prop the macro doesn't have or a parameter the comment doesn't declare, or a macro in `templates/components/` with no props comment."
---

A props comment on a Component, or a vars comment on a Layout, can't be read or doesn't match its macro: an entry that isn't `name: type` or `name?: type`, an unknown type, a props comment that isn't directly above a `{% macro %}`, a prop the macro doesn't have or a parameter the comment doesn't declare, or a macro in `templates/components/` with no props comment.

## Example

```
{# props: href: uri #}
{% macro row(href) %}…{% endmacro %}
```

## Fix

Write the comment directly above its macro, one `name: type` for each parameter, `?` after the name of an optional one. The types are `string`, `int`, `float`, `bool`, `date`, `datetime`, `url`, `image`, `enum(a|b)`, `list<…>`, `object`, `slot`, `attrs`, `entry<collection>`, `term` and `paginator`.
