---
title: "org/too-deep"
description: "Blocks, lists or markup in an Org body nest inside each other too deeply for yuanme to parse."
---

Blocks, lists or markup in an Org body nest inside each other too deeply for yuanme to parse.

## Example

```
#+BEGIN_QUOTE
#+BEGIN_QUOTE
#+BEGIN_QUOTE
(and so on, thousands of times)
```

## Fix

Nest them less deeply, or close the ones that never end.
