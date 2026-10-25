---
title: "markdown/empty-directive"
description: "A `:::` line opens a block directive with no name."
---

A `:::` line opens a block directive with no name. A bare `:::` meant as a closing fence opens a new block instead when it is no longer than an open block's fence, and that block swallows the rest of the file.

## Example

```
::::note
:::tip
Text
:::
:::
(the last line should be `::::`)
```

## Fix

Give the opening fence more colons than any block inside it, and close it with the same number, or name the directive.
