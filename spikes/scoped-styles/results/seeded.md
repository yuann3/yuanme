unseeded site: 0 diagnostics

| # | Seed | Diagnostics |
|---|---|---|
| 1 | typo in a sidecar class | `projects/index.css:8:1: `.project-nam`: `.project-nam` matches no literal element of this template (elements of other templates, slots and rendered content need :global())` |
| 2 | the same typo where the template has an element with a Jinja-built class (`class="{{ lane }}"`): MISSED, that element may carry any class | none |
| 3 | type selector for an element the template does not write | `components/pagination.css:5:1: `nav li`: `li` matches no literal element of this template (elements of other templates, slots and rendered content need :global())` |
| 4 | rule targets an element that lives in an included partial (the template-check port's shape) | `layouts/post.css:1:1: `.meta-link`: `.meta-link` matches no literal element of this template (elements of other templates, slots and rendered content need :global())`<br>`layouts/post.css:5:1: `.meta-link:hover`: `.meta-link` matches no literal element of this template (elements of other templates, slots and rendered content need :global())` |
| 5 | class handed to another file's macro (`prose(class="project-body")`, the template-check port's shape) | `projects/entry.css:27:1: `.project-body > h1:first-of-type`: `.project-body` matches no literal element of this template (elements of other templates, slots and rendered content need :global())` |
| 6 | child component's element styled without :global() | `components/photo_pile.css:31:1: `.photo picture`: `picture` matches no literal element of this template (elements of other templates, slots and rendered content need :global())` |
| 7 | rendered content styled without :global() | `layouts/post.css:10:1: `article em`: `em` matches no literal element of this template (elements of other templates, slots and rendered content need :global())` |
| 8 | `:global()` empty | `layouts/post.css:10:17: Invalid empty selector` |
| 9 | `:global` without parentheses | `layouts/post.css:10:10: Ambiguous CSS module class not supported` |
| 10 | `:global(a, b)` list | `layouts/post.css:10:19: Unexpected token Comma` |
| 11 | `:global(` unbalanced | `layouts/post.css:14:1: Unexpected end of input` |
| 12 | `:local()` | `layouts/post.css:10:1: `article em`: :local() is not supported; sidecar selectors are scoped by default` |
| 13 | `\| safe` in a template with a sidecar | `components/pagination.html:5:55: warning: `\| safe` output is not stamped: sidecar rules reach its elements only through :global()` |
| 14 | dynamic tag name in a template with a sidecar | `components/pagination.html:4:1: warning: expression glued to a tag name is read as a dynamic tag name: stamped, but `check` cannot match type selectors against it` |
| 15 | NOT a mistake: class built with Jinja (`photo-{{ kind }}`), state attributes, `:hover` | none |
