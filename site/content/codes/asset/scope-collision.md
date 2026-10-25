---
title: "asset/scope-collision"
description: "Two Sidecar stylesheets have the same scope attribute, which yuanme makes from each one's path, so each one's styles would reach the other's Template."
---

Two Sidecar stylesheets have the same scope attribute, which yuanme makes from each one's path, so each one's styles would reach the other's Template. It is about one chance in two billion for a pair.

## Example

```
templates/components/a.css and another Sidecar whose path hashes the same
```

## Fix

Rename one of the two Templates and its Sidecar.
