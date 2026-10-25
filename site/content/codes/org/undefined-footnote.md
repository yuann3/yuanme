---
title: "org/undefined-footnote"
description: "An Org footnote reference names a footnote that is never defined."
---

An Org footnote reference names a footnote that is never defined. Org's exporter fails on it too.

## Example

```
#+TITLE: Pew

A claim.[fn:1]
```

## Fix

Define the footnote, as `[fn:1] The note.` at the start of a line, or inline, as `[fn:1:The note.]`.
