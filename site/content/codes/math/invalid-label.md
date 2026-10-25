---
title: "math/invalid-label"
description: "A Label has a character other than letters, digits, `:`, `_`, `.` and `-`."
---

A Label has a character other than letters, digits, `:`, `_`, `.` and `-`. A Label becomes an `id` and a link's `#fragment` exactly as written, so it is kept to characters that need no escaping.

## Example

```
$$\begin{equation} E = mc^2 \label{eq energy} \end{equation}$$
```

## Fix

Use the Label the help suggests, here `eq-energy`, and change its references to match.
