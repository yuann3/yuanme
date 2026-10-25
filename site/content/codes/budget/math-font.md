---
title: "budget/math-font"
description: "The math font a Page that shows math loads is over its Site budget, `budgets.math_font`, in kB of Brotli at quality 11."
---

The math font a Page that shows math loads is over its Site budget, `budgets.math_font`, in kB of Brotli at quality 11. yuanme subsets it once for the whole Site, to every character the Site's math shows, so one Entry's rare symbols make it larger on every Page with math. The Starters' budget is 55 kB.

## Example

```
$$\mathfrak{A} \boxtimes \mathscr{B}$$
(with many other alphabets elsewhere on the Site)
```

## Fix

Use fewer alphabets and symbols across the Site's math. If the Site's math needs them, raise `budgets.math_font`, or let the Pages go over with Acknowledged overages, as the help shows.
