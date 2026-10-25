---
title: "budget/css"
description: "The CSS a Page loads is over its Site budget, `budgets.css`: its stylesheets and `<style>` elements, each in kB of Brotli at quality 11."
---

The CSS a Page loads is over its Site budget, `budgets.css`: its stylesheets and `<style>` elements, each in kB of Brotli at quality 11. The Starters' budget is 6 kB.

## Example

```
<link rel="stylesheet" href="/styles/all-of-bootstrap.css">
```

## Fix

Load less CSS: drop rules no Page uses, and move a Component's styles into its Sidecar, which only the Pages that render it load. If the Page is right as it is, raise `budgets.css`, or let this one Page go over with an Acknowledged overage, as the help shows.
