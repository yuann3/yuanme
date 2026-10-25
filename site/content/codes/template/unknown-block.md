---
title: "template/unknown-block"
description: "A Template overrides a block that no Layout it extends has, so its content would never be shown."
---

A Template overrides a block that no Layout it extends has, so its content would never be shown.

## Example

```
{% extends "layouts/base.html" %}
{% block contnet %}…{% endblock %}
```

## Fix

Use the name of a block the Layout defines, such as the one the help suggests.
