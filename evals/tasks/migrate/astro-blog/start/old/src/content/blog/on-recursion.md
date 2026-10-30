---
title: On recursion
description: A function that calls itself, and why that terminates.
pubDate: 2024-06-03
---

A recursive function terminates when every call makes some measure smaller, such as the length of a list.

```python
def length(xs):
    return 0 if not xs else 1 + length(xs[1:])
```

Each call gets a shorter list, so the calls end.
