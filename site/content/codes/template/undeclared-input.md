---
title: "template/undeclared-input"
description: "A Component reads a variable it wasn't given: not one of its props, nor `site`, `data`, `collections`, `taxonomies` or `page`, which every Page sees."
---

A Component reads a variable it wasn't given: not one of its props, nor `site`, `data`, `collections`, `taxonomies` or `page`, which every Page sees. Rendering lets a macro see the variables of the Template that imports it, so this works on one Page and breaks on another.

## Example

```
{# props: title: string #}
{% macro row(title) %}{{ entries | length }}{% endmacro %}
```

## Fix

Add the value to the props comment and the macro's parameters, and pass it in at every call.
