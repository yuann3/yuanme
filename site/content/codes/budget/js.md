---
title: "budget/js"
description: "The JS a Page runs as it loads is over its Site budget, `budgets.js`: its Scripts, the chunks they import and its inline `<script>`s, each in kB of Brotli at quality 11."
---

The JS a Page runs as it loads is over its Site budget, `budgets.js`: its Scripts, the chunks they import and its inline `<script>`s, each in kB of Brotli at quality 11. A chunk imported with `import()` doesn't count until it loads, nor does a script from another origin. The Starters' budget is 3 kB.

## Example

```
<script type="module" src="/scripts/charts.ts"></script>
(where charts.ts imports a charting library)
```

## Fix

Load the code only when it is needed, with `import()`, such as when the element that uses it scrolls into view, or replace a large package with a smaller one. If the Page is right as it is, raise `budgets.js`, or let this one Page go over with an Acknowledged overage, as the help shows.
