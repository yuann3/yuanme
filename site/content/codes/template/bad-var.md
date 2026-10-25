---
title: "template/bad-var"
description: "A Template extends a Layout without setting a variable its vars comment requires, or sets one to a value of the wrong type."
---

A Template extends a Layout without setting a variable its vars comment requires, or sets one to a value of the wrong type.

## Example

```
{% extends "layouts/base.html" %}
{% block content %}…{% endblock %}
(with `{# vars: title: string #}` in the Layout)
```

## Fix

Set the variable before the first block, `{% set title = "Tags" %}`, with a value of the type the vars comment gives it.
