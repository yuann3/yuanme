---
title: "math/unknown-label"
description: "An `\\eqref`, or an Org `\\ref`, names a Label that no numbered equation in the Entry has."
---

An `\eqref`, or an Org `\ref`, names a Label that no numbered equation in the Entry has.

## Example

```
$$\begin{equation} E = mc^2 \label{eq:energy} \end{equation}$$

By $\eqref{eq:enrgy}$, …
```

## Fix

Use the Label the equation has, which the help suggests when one is close, or give the equation a `\label`.
