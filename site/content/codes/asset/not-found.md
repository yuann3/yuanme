---
title: "asset/not-found"
description: "A `<link href>` or `<script src>` in a Page, an `@import` or `url()` in a stylesheet, or an `import` in a Script names a path that is no file of the project, no Static file and no URL of the Site."
---

A `<link href>` or `<script src>` in a Page, an `@import` or `url()` in a stylesheet, or an `import` in a Script names a path that is no file of the project, no Static file and no URL of the Site. A path from `/` names a file from the project's root, such as `/styles/site.css` for `styles/site.css`, or a Static file without its `static/`; any other path is relative to the Page, the stylesheet or the Script. `@import "pkg/x.css"` also looks in `node_modules`, and so does an `import` of a bare name such as `nanoid`. A Script's `import "./x"` may leave out `.ts` or `.js`, and `./x.js` also finds `x.ts`.

## Example

```
<link rel="stylesheet" href="/styles/sit.css">
(with styles/site.css)
```

## Fix

Correct the path, here `/styles/site.css`, or create the file. A Static file is named without `static/`. For a package, install it with `bun install`.
