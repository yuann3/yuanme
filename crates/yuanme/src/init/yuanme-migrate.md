---
name: yuanme-migrate
description: Bring a blog or site from Astro, Hugo (ox-hugo Org included), Jekyll or Zola into this yuanme Site, keeping every URL. Maps front matter, dates, Slugs and Taxonomies. Use when asked to migrate, import or port content from another site generator.
---

# Migrate a site to yuanme

Read `yuanme docs collections` and `yuanme docs config` first. Then take the steps in order.

1. **Save the old URLs.** List every URL the old site serves, from its sitemap or its built output. Each one must still work at the end.
2. **Declare the Collections and Taxonomies** in `yuanme.toml`. Give each Collection the `route` its URLs already have: Hugo's `/posts/:slug/` is `route = "/posts/{slug}/"`, and a Taxonomy's `route` is like `/tags/{term}/`. Declare each front matter key the content uses as a field with its type, rather than dropping it: an undeclared key is an error.
3. **Move the content** into `content/COLLECTION/`. Markdown stays Markdown and Org stays Org. Map the front matter:
   - `title`, `date` and `draft` keep their names. A date without an offset is in `[site] timezone`.
   - The Slug comes from the file name. Keep the old one with `slug:`, or rename the file: Jekyll's `2024-05-01-hello.md` becomes `hello.md`, and Hugo's and Zola's `slug` stays `slug`.
   - Tags and categories go under the Taxonomy's name.
   - Hugo's `summary`, and Astro's and Zola's `description`, become a field you declare.
   - Hugo's `aliases` and Jekyll's `redirect_from` become `[[host.redirects]]` in Site config.
4. **ox-hugo.** One Entry is one file. Split a file of many posts into one Org file per subtree that has `:EXPORT_FILE_NAME:`, which gives the Slug. Turn `#+HUGO_TAGS` or the subtree's tags into `#+FILETAGS: :a:b:` when the Taxonomy is `tags`, and drop the `HUGO_` export keywords. `yuanme docs org` lists what renders.
5. **Rewrite what isn't content.** Shortcodes, Liquid tags and MDX components don't run in a body: write them as Markdown or Org, or move them into a Template or Component (`yuanme docs templates`, `yuanme docs components`). Labels and cross-references follow `yuanme docs math`.
6. **Check the URLs.** Run `yuanme check --json` until it exits 0. Then compare `yuanme routes --json` with the saved list: each old URL must be a Route or the `from` of a redirect. Add a redirect for each one that moved.
