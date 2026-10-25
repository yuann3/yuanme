---
title: "budget/html"
description: "A Page's first-flight HTML is over its Site budget, `budgets.html`: the whole document in kB of Brotli at quality 11, but an Entry's Page without its body, which the build report lists apart."
---

A Page's first-flight HTML is over its Site budget, `budgets.html`: the whole document in kB of Brotli at quality 11, but an Entry's Page without its body, which the build report lists apart. The first 14 kB is what a server sends before it waits for the browser, so a Page over it takes another round trip to show. The Starters' budget is 14 kB.

## Example

```
<!doctype html>
<style>/* 40 kB of CSS inline */</style>
(in a Template)
```

## Fix

Move what the Page doesn't need at first out of its HTML: inline CSS and SVG into files, a long list into pages with `paginate`. If the Page is right as it is, raise `budgets.html`, or let this one Page go over with an Acknowledged overage, as the help shows.
