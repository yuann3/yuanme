---
title: "template/bad-filter-argument"
description: "A filter or function got an argument it can't use: a missing or unknown one, a value of the wrong type, a `dateformat` format it doesn't know, or a `-` key passed to Jinja's own `sort`, which reads it as a field name."
---

A filter or function got an argument it can't use: a missing or unknown one, a value of the wrong type, a `dateformat` format it doesn't know, or a `-` key passed to Jinja's own `sort`, which reads it as a field name.

## Example

```
{{ entry.date | dateformat(format="iso") }}
```

## Fix

Pass what the message asks for. A `dateformat` format is strftime directives, such as "%Y-%m-%d", or "rfc2822" or "rfc3339". To sort descending, use `sort_by(["-count"])`.
