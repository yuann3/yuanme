---
title: "asset/invalid-script"
description: "A Script can't be bundled: it isn't valid JavaScript or TypeScript, or isn't UTF-8, or it imports a name the other module doesn't export, or a `<script type=\"module\">` names a file that is no Script."
---

A Script can't be bundled: it isn't valid JavaScript or TypeScript, or isn't UTF-8, or it imports a name the other module doesn't export, or a `<script type="module">` names a file that is no Script. A Script is a `.ts`, `.js`, `.mts` or `.mjs` file.

## Example

```
let x = ;
```

## Fix

Fix the code at the reported place. Here the variable needs a value.
