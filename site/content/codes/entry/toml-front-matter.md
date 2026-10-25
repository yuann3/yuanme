---
title: "entry/toml-front-matter"
description: "An Entry starts with `+++` TOML front matter."
---

An Entry starts with `+++` TOML front matter. yuanme reads front matter as YAML only, between `---` lines.

## Example

```
+++
title = "Pew"
+++
```

## Fix

Rewrite it as YAML between `---` lines: `title: Pew`.
