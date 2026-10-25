---
title: "budget/unknown-page"
description: "An Acknowledged overage in `[budgets.overages]` names a URL that is no HTML Page of the Site, so it silences nothing."
---

An Acknowledged overage in `[budgets.overages]` names a URL that is no HTML Page of the Site, so it silences nothing.

## Example

```
[budgets.overages."/abuot/"]
js = 3
```

## Fix

Use the Page's URL, as `yuanme routes` lists it, such as the one the help suggests, or delete the overage if the Page is gone.
