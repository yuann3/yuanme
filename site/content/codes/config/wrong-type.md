---
title: "config/wrong-type"
description: "A key in Site config has a value of the wrong TOML type, such as a string where a number goes."
---

A key in Site config has a value of the wrong TOML type, such as a string where a number goes.

## Example

```
[collections.posts]
route = "/blog/{slug}/"
paginate = "10"
```

## Fix

Write the value as the type the message names. Here `paginate = 10`, without quotes.
