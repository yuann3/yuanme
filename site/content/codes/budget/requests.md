---
title: "budget/requests"
description: "A Page makes more requests on its render path than its Site budget, `budgets.requests`, allows."
---

A Page makes more requests on its render path than its Site budget, `budgets.requests`, allows. They are the Page itself, each stylesheet, each Script and chunk it imports, each font its stylesheets name, and each `<link rel="preload">`, such as of the image a visitor sees first, whatever their origin. The Starters' budget is 4.

## Example

```
<link rel="stylesheet" href="/styles/a.css">
<link rel="stylesheet" href="/styles/b.css">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Inter">
```

## Fix

Load fewer files: `@import` one stylesheet from another, which yuanme bundles into one, import one Script from another, and self-host fonts and scripts from other origins. If the Page is right as it is, raise `budgets.requests`, or let this one Page go over with an Acknowledged overage, as the help shows.
