---
title: "asset/type-error"
description: "With `[typescript] check = true`, TypeScript's `tsc` finds a type error in a Script, or in a file a Script imports."
---

With `[typescript] check = true`, TypeScript's `tsc` finds a type error in a Script, or in a file a Script imports. The message is TypeScript's, after its own code, such as `TS2322`. yuanme checks every TypeScript Script a Page loads, Sidecar scripts included, with the Site's `tsconfig.json`, when it has one, and the types in `.yuanme/types/yuanme.d.ts`.

## Example

```
const year: number = "2026";
```

## Fix

Change the code as the message says, here `const year: number = 2026;`. Search for the TS code for TypeScript's explanation of it.
