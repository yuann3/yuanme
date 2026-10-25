---
title: "asset/unmatched-selector"
description: "A selector in a Sidecar stylesheet can match no tag its own Template writes."
---

A selector in a Sidecar stylesheet can match no tag its own Template writes. A Sidecar's styles are scoped to the tags written in its Template's own file, so an element written in another Template, a Partial it includes, the content passed into a Component or an Entry's rendered body is out of its reach. `check` compares the selector's type, classes and id with each tag; attributes and pseudo-classes are left out, since Scripts set them. A class printed with `{{ }}` may be any class, so it matches every class. The Page still builds.

## Example

```
.card .title { font-weight: 600 }
(in components/card.css, when the title is written by components/title.html)
```

## Fix

Move the rule to the Sidecar of the Template that writes the element, fix the typo, or reach outside the Template on purpose with `:global()`: `.card :global(.title)`.
