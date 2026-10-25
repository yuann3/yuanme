---
title: "asset/jsx"
description: "A Script is a `.jsx` or `.tsx` file, or has JSX in it."
---

A Script is a `.jsx` or `.tsx` file, or has JSX in it. yuanme doesn't compile JSX.

## Example

```
const el = <div>Hi</div>;
```

## Fix

Build the element with the DOM, `document.createElement("div")`, or write it in a Template, and rename the file to `.ts` or `.js`.
