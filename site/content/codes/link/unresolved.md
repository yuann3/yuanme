---
title: "link/unresolved"
description: "A link to another file or a fragment doesn't lead anywhere in the Site: no Entry has that file as its source, or the page has no element with that id."
---

A link to another file or a fragment doesn't lead anywhere in the Site: no Entry has that file as its source, or the page has no element with that id.

## Example

```
[the setup](setup.md#install), where setup.md has no `## Install` heading
```

## Fix

Link to an Entry's file by its path from this file, and to a heading by its id. Links to URLs and to `/` paths are left as they are.
