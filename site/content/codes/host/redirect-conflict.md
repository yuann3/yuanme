---
title: "host/redirect-conflict"
description: "A `[[host.redirects]]` rule redirects a URL the Site serves: a Page, a Static file, or a Page's URL without its trailing `/`, which hosts already redirect."
---

A `[[host.redirects]]` rule redirects a URL the Site serves: a Page, a Static file, or a Page's URL without its trailing `/`, which hosts already redirect. Hosts disagree about which wins: Cloudflare follows the rule, and Netlify serves the file.

## Example

```
[[host.redirects]]
from = "/blog/"
to = "/writing/"
(with a Page at /blog/)
```

## Fix

Remove the rule, or the Page or file it hides.
