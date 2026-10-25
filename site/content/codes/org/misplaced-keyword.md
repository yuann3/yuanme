---
title: "org/misplaced-keyword"
description: "A keyword that sets one of the Entry's fields, such as `#+TITLE` or `#+DATE`, comes after the first headline or body text."
---

A keyword that sets one of the Entry's fields, such as `#+TITLE` or `#+DATE`, comes after the first headline or body text. Fields are written at the top of the file.

## Example

```
* Intro
#+DATE: 2025-12-12
```

## Fix

Move the keyword to the top of the file, before any headline or text.
