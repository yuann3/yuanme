---
title: "math/unsupported"
description: "Math uses LaTeX that yuanme can't render, or isn't valid LaTeX: an unknown command or environment, a group that is never closed, or `\\label` outside a numbered environment."
---

Math uses LaTeX that yuanme can't render, or isn't valid LaTeX: an unknown command or environment, a group that is never closed, or `\label` outside a numbered environment. yuanme renders math to MathML when it builds, and fails the build rather than show the TeX.

## Example

```
$$\begin{eqnarray} a &=& b \end{eqnarray}$$
```

## Fix

Use what the help suggests, such as `align` for `eqnarray`, or define a missing command in `[math.macros]` in yuanme.toml.
