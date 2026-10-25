---
title: "link/redirected"
description: "A Page links to a URL of the Site that hosts answer with a redirect, which costs the visitor a round trip: a Page's URL without its trailing `/` or with `index.html` on the end, a `.html` file, a `[[host.redirects]]` rule's `from`, or the Site's address with or without `www.`."
---

A Page links to a URL of the Site that hosts answer with a redirect, which costs the visitor a round trip: a Page's URL without its trailing `/` or with `index.html` on the end, a `.html` file, a `[[host.redirects]]` rule's `from`, or the Site's address with or without `www.`. Every internal link should be the URL a Page is served at, such as `/about/`.

## Example

```
<a href="/about">About</a>
(with a Page at /about/)
```

## Fix

Link to the URL the help names, here `/about/`.
