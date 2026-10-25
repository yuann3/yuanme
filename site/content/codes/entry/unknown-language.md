---
title: "entry/unknown-language"
description: "A code block names a language that no grammar highlights, so it is shown as plain text."
---

A code block names a language that no grammar highlights, so it is shown as plain text. yuanme reads Shiki's language names and aliases, Org's own names such as `elisp` and `C`, and `jupyter-X` as `X`. `text`, `none`, `fundamental` and `example` are plain text with no warning.

## Example

```
```pyhton
print(1)
```
```

## Fix

Correct the name, here `python`, or write `text` for a block that is meant to be plain.
