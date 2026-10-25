---
title: "template/bad-prop"
description: "A call to a Component passes a prop it doesn't declare, leaves out one it requires, passes one of the wrong type or an optional value to a required prop, or gives a `{% call %}` body to a Component that takes none, or none to one that needs it."
---

A call to a Component passes a prop it doesn't declare, leaves out one it requires, passes one of the wrong type or an optional value to a required prop, or gives a `{% call %}` body to a Component that takes none, or none to one that needs it.

## Example

```
{{ list_row(titel="Writing", href="/blog/") }}
(with `{# props: href: url, title: string #}`)
```

## Fix

Pass the props the Component's props comment declares, with the types it gives them. Guard an optional value, or give it `| default(…)`.
