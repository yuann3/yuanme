---
title: "asset/tailwind-error"
description: "Tailwind can't build a stylesheet that does `@import \"tailwindcss\"`: the Tailwind CLI reports an error, such as an `@apply` of a utility that doesn't exist or an `@plugin` it can't find, or a stylesheet that another one imports, or a Sidecar, imports Tailwind."
---

Tailwind can't build a stylesheet that does `@import "tailwindcss"`: the Tailwind CLI reports an error, such as an `@apply` of a utility that doesn't exist or an `@plugin` it can't find, or a stylesheet that another one imports, or a Sidecar, imports Tailwind. Only a stylesheet a Template links with `<link rel="stylesheet">` can import it. The message is Tailwind's.

## Example

```
@import "tailwindcss";
.card { @apply rounded-lgg; }
```

## Fix

Change the stylesheet as the message says, here `rounded-lg`. Install a plugin with `bun add`. Move `@import "tailwindcss"` into the stylesheet a Template links; a Sidecar uses Tailwind's theme through `var(--color-sky-500)` instead of `@apply`.
