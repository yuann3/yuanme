---
title: "host/wrangler-mismatch"
description: "`wrangler.jsonc` doesn't serve the Site the way the cloudflare adapter writes it: `assets.directory` must be the output directory, `assets.html_handling` must be \"auto-trailing-slash\", the default, and `assets.not_found_handling` must be \"404-page\" for the Site's 404 Page to be served."
---

`wrangler.jsonc` doesn't serve the Site the way the cloudflare adapter writes it: `assets.directory` must be the output directory, `assets.html_handling` must be "auto-trailing-slash", the default, and `assets.not_found_handling` must be "404-page" for the Site's 404 Page to be served.

## Example

```
{ "assets": { "directory": "./public" } }
```

## Fix

Set the value the message names, here `"directory": "./dist"`.
