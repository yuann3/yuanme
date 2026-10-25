---
title: "template/unknown-filter"
description: "A Template calls a filter, test or function that neither Jinja nor yuanme has, a macro its import doesn't define, or a method: Templates have no methods, such as Python's `.lower()`."
---

A Template calls a filter, test or function that neither Jinja nor yuanme has, a macro its import doesn't define, or a method: Templates have no methods, such as Python's `.lower()`.

## Example

```
{{ entry.date | date_format(format="%Y") }}
```

## Fix

Correct the name, here `dateformat`, or use the filter a method's help names, such as `| lower`. yuanme adds `sort_by`, `dateformat`, `url`, `absolute_url`, `pluralize`, `attrs`, `markdown` and `org` to Jinja's own.
