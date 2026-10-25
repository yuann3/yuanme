---
title: "template/unknown-url"
description: "`url()` names a path that is no Route, feed or Static file of the Site, so the link would lead nowhere."
---

`url()` names a path that is no Route, feed or Static file of the Site, so the link would lead nowhere.

## Example

```
<a href="{{ url('/abuot/') }}">About</a>
```

## Fix

Use a path the Site has, such as the one the help suggests. Every URL of a Site is declared in Site config.
