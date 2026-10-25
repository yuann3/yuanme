The Reference site's Templates that scoped-styles spike ported (#19,
branch `spike/scoped-styles`), over `crates/yuanme-render/tests/sites/eyuan`,
with the six Sidecars it copied verbatim from the Astro components' `<style>`
blocks. `projects/index.html` is `projects/list.html` here, the name the
Engine gives a Collection's list Template.

`astro.css` is what Astro 5.16.5 scoped from five of those blocks: the
`<style>` elements of the Pages named in its comments, from the `dist/` that
`../reference-css` comes from. PhotoPile's scoped rules
are in `../reference-css/styles/about.css`.
