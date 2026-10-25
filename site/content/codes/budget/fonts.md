---
title: "budget/fonts"
description: "The text fonts a Page loads are over their Site budget, `budgets.fonts`: every font file that a stylesheet the Page loads names in `url()`, or that the Page preloads, but the math font, in kB of Brotli at quality 11."
---

The text fonts a Page loads are over their Site budget, `budgets.fonts`: every font file that a stylesheet the Page loads names in `url()`, or that the Page preloads, but the math font, in kB of Brotli at quality 11. A font counts whether or not the Page's text uses it. The Starters' budget is 30 kB.

## Example

```
@font-face { font-family: Inter; src: url("/fonts/inter-all-weights.woff2"); }
```

## Fix

Use fewer weights and styles, subset each font to the characters the Site shows, and serve WOFF2. If the Page is right as it is, raise `budgets.fonts`, or let this one Page go over with an Acknowledged overage, as the help shows.
