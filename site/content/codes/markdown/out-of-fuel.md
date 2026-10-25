---
title: "markdown/out-of-fuel"
description: "A Markdown body would take longer to parse than yuanme allows for its length, so the build stops instead of hanging."
---

A Markdown body would take longer to parse than yuanme allows for its length, so the build stops instead of hanging. Only pathological input does this, such as hundreds of block directives opened inside each other and never closed.

## Example

```
:::a
:::a
:::a
(and so on, a few hundred times)
```

## Fix

Close the blocks that start at the reported line, or nest them less deeply.
