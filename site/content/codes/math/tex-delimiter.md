---
title: "math/tex-delimiter"
description: "A Markdown paragraph has `\\(…\\)`, `\\[…\\]` or a bare `\\begin{…}`, which Markdown doesn't read as math: there `\\(` and `\\[` are escaped brackets, as on GitHub and in Pandoc."
---

A Markdown paragraph has `\(…\)`, `\[…\]` or a bare `\begin{…}`, which Markdown doesn't read as math: there `\(` and `\[` are escaped brackets, as on GitHub and in Pandoc. The paragraph is shown as written.

## Example

```
The area is \(\pi r^2\).
```

## Fix

Write inline math as `$…$` and display math as `$$…$$`, with any environment inside the `$$`.
