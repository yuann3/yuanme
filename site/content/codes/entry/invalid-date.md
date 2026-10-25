---
title: "entry/invalid-date"
description: "A `date` or `datetime` field can't be read."
---

A `date` or `datetime` field can't be read. Dates are ISO 8601, such as 2025-12-12; datetimes are RFC 3339, such as 2025-12-12T09:30:00+08:00, and one without an offset is in `site.timezone`.

## Example

```
---
title: Pew
date: Dec 12 2025
---
```

## Fix

Write the date in ISO form, `date: 2025-12-12`; the help suggests it when the date can be read another way.
