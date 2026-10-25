---
title: "markdown/no-effect"
description: "Something in a Markdown body is valid but changes nothing in the output, such as a `[title]` on a directive that isn't a Theorem block."
---

Something in a Markdown body is valid but changes nothing in the output, such as a `[title]` on a directive that isn't a Theorem block.

## Example

```
:::tip[Heads up]
Text
:::
```

## Fix

Remove it, or use a construct that shows it.
