---
title: "host/output-collision"
description: "A Static file has the name of a file the Host adapter writes, such as `static/_headers` with the cloudflare or netlify adapter."
---

A Static file has the name of a file the Host adapter writes, such as `static/_headers` with the cloudflare or netlify adapter.

## Example

```
static/_headers, with `[host] adapter = "cloudflare"`
```

## Fix

Delete the Static file, and write its rules in Site config as `[[host.headers]]` and `[[host.redirects]]`, which every adapter renders.
