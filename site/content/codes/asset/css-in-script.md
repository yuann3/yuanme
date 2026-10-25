---
title: "asset/css-in-script"
description: "A Script imports a stylesheet."
---

A Script imports a stylesheet. yuanme bundles the stylesheets that Pages link with `<link rel="stylesheet">`, not those a Script imports, so the CSS would never reach a Page. This includes a package whose own JS imports CSS, which can't be used yet.

## Example

```
import "./photo.css";
```

## Fix

Delete the import, and import the stylesheet from one that a Page links instead, as the help shows: `@import "/scripts/photo.css";`, or for a package, its CSS file, such as `@import "nprogress/nprogress.css";`. The suggested edit deletes the import.
