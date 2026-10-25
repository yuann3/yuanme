---
title: "asset/invalid-css"
description: "A stylesheet can't be bundled: it isn't valid CSS or UTF-8, or it imports one file twice in ways that can't be merged, such as under two different `@layer`s."
---

A stylesheet can't be bundled: it isn't valid CSS or UTF-8, or it imports one file twice in ways that can't be merged, such as under two different `@layer`s.

## Example

```
a$b { color: red }
```

## Fix

Fix the CSS at the reported place. Here `$` is not part of a selector.
