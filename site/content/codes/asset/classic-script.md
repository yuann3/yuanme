---
title: "asset/classic-script"
description: "A `<script src>` without `type=\"module\"` names a project file."
---

A `<script src>` without `type="module"` names a project file. yuanme bundles every Script as a module, so it can't load one as a classic script. Static files and `https://` URLs are left as they are.

## Example

```
<script src="/scripts/site.ts"></script>
```

## Fix

Add `type="module"`: `<script type="module" src="/scripts/site.ts"></script>`. A module is deferred and runs in strict mode, so check a script that relied on running at once. A file that must stay classic goes in `static/`.
