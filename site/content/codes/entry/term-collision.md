---
title: "entry/term-collision"
description: "Two different Terms of a Taxonomy end up with the same Slug, because a `slugs` override in `yuanme.toml` gives one the Slug of another."
---

Two different Terms of a Taxonomy end up with the same Slug, because a `slugs` override in `yuanme.toml` gives one the Slug of another.

## Example

```
slugs = { "C++" = "c" } in [taxonomies.tags], with Entries tagged both `C++` and `C`
```

## Fix

Change the override so every Term keeps its own Slug.
