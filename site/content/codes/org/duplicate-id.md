---
title: "org/duplicate-id"
description: "Two Org Entries or headings have the same `:ID:` property, so an `id:` link to it could lead to either."
---

Two Org Entries or headings have the same `:ID:` property, so an `id:` link to it could lead to either.

## Example

```
content/posts/a.org and content/posts/b.org, both with `:ID: 1234`
```

## Fix

Give one of them a new ID; in Emacs, `org-id-get-create` makes one.
