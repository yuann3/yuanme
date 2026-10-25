---
title: "org/out-of-fuel"
description: "An Org paragraph would take longer to parse than yuanme allows for its length, so the build stops instead of hanging."
---

An Org paragraph would take longer to parse than yuanme allows for its length, so the build stops instead of hanging. Only pathological input does this, such as hundreds of openers like `[[` or `*` that never close.

## Example

```
[[a [[a [[a [[a
(and so on, many thousands of times)
```

## Fix

Close or remove the openers in the reported paragraph.
