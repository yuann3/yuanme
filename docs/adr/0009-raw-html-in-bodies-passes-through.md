# Raw HTML in Entry bodies passes through, in both Source formats

Raw HTML in an Entry's body reaches the Page unchanged. That covers HTML blocks and inline HTML in Markdown, and `#+begin_export html`, `@@html:…@@` and `#+HTML:` in Org. There is no switch to turn it off. Org export blocks and snippets for other back ends (`latex`, `hugo`, `md`) are dropped, as ox-html drops them. `#+ATTR_HTML` puts any attribute on its element, since it adds nothing raw HTML could not already do.

The content of a Site belongs to its Author, and the output is static, so there is no untrusted input to defend against. Astro passed raw HTML through (rehype-raw), so the Reference site's future Posts keep the contract they were written under. Embeds such as video iframes need it. Decided in [What is the Org subset, and how do Org and Markdown share one metadata model?](https://github.com/yuann3/yuanme/issues/16).

## Considered Options

- **Omit raw HTML unless the Author opts in** (rejected): Hugo's default (`unsafe = false`) protects multi-author Sites that render other people's content, which yuanme Sites do not do. It silently drops content, which the Org subset forbids.
- **Sanitise with an allow-list** (rejected): a sanitiser dependency and a policy to maintain, and it still breaks embeds or lets them through by exception.
- **Make raw HTML a build error** (rejected): there would be no way to embed media, and Authors would push markup into Templates that belongs to one Post.

## Consequences

- Tightening this later breaks existing Posts, so it is effectively permanent for v1.
- The Engine warns when a body's raw HTML leaves an element open or closes one it did not open, because that breaks the Template around the body. It never rewrites the HTML.
- Raw HTML is never scoped (ADR 0006) and never touched by smartypants.
