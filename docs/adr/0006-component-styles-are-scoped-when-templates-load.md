# Component styles are scoped when Templates load

A Template can have a stylesheet beside it (`row.html` and `row.css`), and those styles apply only to the elements that Template itself writes. The Engine does this when it loads the Template, not when it renders a Page: it adds one generated attribute to every literal tag in the Template's source, and adds the same attribute to every selector in the stylesheet. Content passed into a Component, and rendered Markdown or Org, are not literal tags, so they are never scoped; `:global(...)` is the deliberate way to style them. Layouts and Templates for whole Pages work the same way as Components.

Authors asked for CSS as capable as in Astro or SvelteKit, where a component's styles cannot leak. Those frameworks compile templates, so they own both the markup and the CSS. Our Templates are filled in at runtime (ADR 0002), which is why the scoping happens on the Template's source. A probe measured about half a microsecond per Template at load and no cost per Page. Decided in [How do Authors write templates and components, and how is template context typed?](https://github.com/yuann3/yuanme/issues/19).

## Considered Options

- **Native CSS `@scope`** (rejected as the default): it needs no change to the markup, but it only became broadly available in March 2026 and cannot be converted for older browsers, so Components would silently lose their styles there. It stays available as an opt-in for Sites that only target recent browsers.
- **CSS Modules** (rejected): free with lightningcss, but selectors on tag names still leak, and Authors would write `class="{{ styles.row }}"` beside plain utility classes.
- **Scoping rendered HTML on every Page** (rejected): the same result, but it costs an HTML rewrite per Page inside the incremental build budget.
- **No scoping, global CSS only** (rejected): it was the first proposal. It is the least work, and it is not what Authors coming from current frameworks expect.

## Consequences

- A stylesheet styles only the tags written in its own Template file. Unlike Astro, the scope is not passed down to the Components a Template calls, so markup that a stylesheet targets has to live in the same file. `check` reports a selector that matches nothing in its own Template.
- A tag whose name is built from an expression may not be scoped, and `check` warns about it.
- Only Templates that have a stylesheet are rewritten, which keeps the attribute out of every other Page.
- The generated attribute appears in Sites' HTML, so its name and the way it is derived are frozen once v1 ships.
- The Engine rewrites Template source before the template engine sees it. `check` reads the Author's original file, and render errors are mapped back to the original line and column.
- Animation names stay global, as they do in Astro, because scripts refer to them by name.

A spike ran this over the Reference site's real Templates (branch `spike/scoped-styles`): all 68 scoped selectors matched the same elements as the Astro build in Chromium, nothing leaked into slot or rendered content, and rewriting all 21 Templates took 34 microseconds. Firefox and WebKit were not tested.
