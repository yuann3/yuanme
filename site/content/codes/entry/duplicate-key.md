---
title: "entry/duplicate-key"
description: "A key appears twice in the same mapping of front matter or a Data file, or an Org keyword that sets a field is given twice."
---

A key appears twice in the same mapping of front matter or a Data file, or an Org keyword that sets a field is given twice. YAML forbids the first, and Org would join the two values (Org divergence 5).

## Example

```
---
title: Pew
title: Pew 2
---
```

## Fix

Delete one of them.
