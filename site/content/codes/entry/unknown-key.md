---
title: "entry/unknown-key"
description: "Front matter, or a Data file with a schema, has a key its Collection or schema doesn't declare, or an Org Entry has a keyword that is neither a field nor one of Org's."
---

Front matter, or a Data file with a schema, has a key its Collection or schema doesn't declare, or an Org Entry has a keyword that is neither a field nor one of Org's. Unknown keys are errors, so a typo can't silently do nothing.

## Example

```
---
title: Pew
draf: true
---
```

## Fix

Use the key the help suggests, such as `draft`, or declare the field under `[collections.<name>.fields]` in `yuanme.toml`.
