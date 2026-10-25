---
title: "org/unsupported-construct"
description: "An Org body uses something outside the Org subset: macros, `#+INCLUDE`, `#+CALL` and other code that runs, citations, radio targets, dynamic blocks, inline tasks, headlines deeper than level 5, or a link type yuanme can't follow."
---

An Org body uses something outside the Org subset: macros, `#+INCLUDE`, `#+CALL` and other code that runs, citations, radio targets, dynamic blocks, inline tasks, headlines deeper than level 5, or a link type yuanme can't follow. Emacs would export it; yuanme fails the build rather than drop it or pass it through.

## Example

```
#+TITLE: Pew

Built on {{{date}}}.
```

## Fix

Write what the construct would produce, as the help says. Here, write the date itself.
