---
title: "asset/dependencies-not-installed"
description: "A Script or stylesheet imports a package, and the project has a `package.json` but no `node_modules`."
---

A Script or stylesheet imports a package, and the project has a `package.json` but no `node_modules`. yuanme reads packages from `node_modules` and never runs a package manager itself.

## Example

```
import { nanoid } from "nanoid";
(with package.json and no node_modules)
```

## Fix

Run `bun install` in the project's folder, then build again.
