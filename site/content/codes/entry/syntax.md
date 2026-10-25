---
title: "entry/syntax"
description: "An Entry's front matter, or a Data file, can't be read: it isn't valid YAML, TOML or JSON, its `---` is never closed, or the file isn't UTF-8."
---

An Entry's front matter, or a Data file, can't be read: it isn't valid YAML, TOML or JSON, its `---` is never closed, or the file isn't UTF-8.

## Example

```
---
title: [Pew
---
```

## Fix

Fix the syntax at the reported place. Here the list needs its closing bracket, or the title needs quotes: `title: "[Pew"`.
