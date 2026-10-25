---
title: "asset/script-warning"
description: "The bundler warns about a Script, or a package it imports, that works but is likely a mistake, such as calling `eval` or importing a name that is always `undefined`."
---

The bundler warns about a Script, or a package it imports, that works but is likely a mistake, such as calling `eval` or importing a name that is always `undefined`. The message is the bundler's.

## Example

```
eval("1 + 1");
```

## Fix

Change the code as the message says. A warning in a package can only be fixed in the package.
