---
title: "org/invalid-attribute"
description: "An `#+ATTR_HTML` line has text that isn't an attribute."
---

An `#+ATTR_HTML` line has text that isn't an attribute. Attributes are written `:name value`.

## Example

```
#+ATTR_HTML: class="wide"
[[file:cat.png]]
```

## Fix

Write each attribute as `:name value`, here `#+ATTR_HTML: :class wide`.
