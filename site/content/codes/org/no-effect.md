---
title: "org/no-effect"
description: "Something in an Org Entry is valid Org but changes nothing in yuanme's output, such as a keyword for Org's own exporter, an `#+OPTIONS` setting yuanme doesn't do, or a `#+CAPTION` on a paragraph."
---

Something in an Org Entry is valid Org but changes nothing in yuanme's output, such as a keyword for Org's own exporter, an `#+OPTIONS` setting yuanme doesn't do, or a `#+CAPTION` on a paragraph.

## Example

```
#+TITLE: Pew
#+AUTHOR: Yuan
```

## Fix

Remove it, or do it the way the note says. Here the author goes in `[site]` in yuanme.toml.
