# Term paths are Slugs, never percent-encoded punctuation

A Term's URL uses its Slug, not its raw name: `c++` is served at `/tags/cplusplus/`, not `/tags/c++/` or `/tags/c%2B%2B/`. Before slugging, the Engine spells out the symbols that carry meaning (`+` as `plus`, `#` as `sharp`, a leading `.` as `dot`) and strips other punctuation. Non-ASCII letters stay and are written as uppercase percent-encoded UTF-8. The Author can override any Term's Slug in Site config, and two Terms with the same Slug are a build error. The page still displays the Term as written.

The reason is that `+` has no spelling that works on every static host. Probes of live sites on 2026-10-09 found that Netlify redirects `/c%2B%2B/` to `/c++/`, Cloudflare Workers static assets redirects `/c++/` to `/c%2B%2B/`, and Vercel reads `+` as a space. Whichever spelling goes into the canonical URL and sitemap, it is wrong on at least one host, and Netlify refuses to deploy file names containing `#` or `?`. Uppercase percent-encoded UTF-8 returned 200 without a redirect on every host tested. Decided in [What is the content model and the Site config?](https://github.com/yuann3/yuanme/issues/18).

## Considered Options

- **Raw names as directory names** (rejected): what Astro and Hugo do. It redirects on Workers, the default adapter, and breaks on Vercel.
- **Percent-encode every reserved character** (rejected): redirects on Netlify. The host-portability research note suggested this before the hosts were probed.
- **Plain github-slugger** (rejected): it turns `C++` and `C#` into `c`, which files those Entries under the `C` tag without telling anyone.

## Consequences

- Term URLs are public, so the symbol spellings and the slugger are frozen once v1 ships. A change needs redirects.
- The Reference site's four tags (`c`, `programming`, `life`, `draft`) keep their URLs, so this needs no Parity exception.
