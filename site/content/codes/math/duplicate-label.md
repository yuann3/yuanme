---
title: "math/duplicate-label"
description: "Two things in one Entry have the same Label."
---

Two things in one Entry have the same Label. Equations' `\label`s, heading ids and Org's `#+NAME`s and `<<targets>>` share one namespace, because each becomes an `id` on the Page.

## Example

```
## Energy {#energy}

$$\begin{equation} E = mc^2 \label{energy} \end{equation}$$
```

## Fix

Rename one of them. A prefix that says what it labels, such as `eq:energy`, keeps them apart.
