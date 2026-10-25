---
title: "org/wrong-weekday"
description: "An Org timestamp in a date field names a day of the week that isn't the date's."
---

An Org timestamp in a date field names a day of the week that isn't the date's. Org ignores the day name, and so does yuanme, but it is probably a typo in the date.

## Example

```
#+DATE: [2025-12-12 Mon]
(the 12th was a Friday)
```

## Fix

Check the date, and correct the day name, here to `Fri`.
