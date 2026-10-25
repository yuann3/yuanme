---
title: "math/named-environment"
description: "An Org LaTeX environment has a `#+NAME`."
---

An Org LaTeX environment has a `#+NAME`. Equations are labelled with `\label` inside the math in both Source formats, which is also how `\eqref` finds them.

## Example

```
#+NAME: eq:energy
\begin{equation}
E = mc^2
\end{equation}
```

## Fix

Remove the `#+NAME` and put `\label{eq:energy}` inside the environment.
