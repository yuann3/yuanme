---
title: "host/unsupported"
description: "The Site's host can't do what the Site needs."
---

The Site's host can't do what the Site needs. GitHub Pages can't send custom headers, immutable caching for hashed assets, brotli or HTTP/3, and redirects only by a page that refreshes. The `static` adapter writes no host configuration, so headers and redirects are the server's to send.

## Example

```
[host]
adapter = "github-pages"
```

## Fix

Choose a host that can, such as `adapter = "cloudflare"`, the default. With the `static` adapter, configure the server as yuanme's deploying docs show.
