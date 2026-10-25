---
title: "template/wrong-type"
description: "A Template uses a value as a type it isn't, such as looping over a string or passing a list to `attrs`."
---

A Template uses a value as a type it isn't, such as looping over a string or passing a list to `attrs`.

## Example

```
{% for tag in entry.title %}…{% endfor %}
```

## Fix

Use the value the message expects, here a list such as `entry.tags`.
