# Spike: scoped component styles stamped at template load

Throwaway code for [#19](https://github.com/yuann3/yuanme/issues/19). Not Engine code.

## Question

#19 provisionally chose "B-static": for a template `x.html` with a sidecar `x.css`, the Engine adds `data-y-<hash>` to every literal start tag of the template source before minijinja sees it, and appends `[data-y-<hash>]` to every compound selector of the sidecar. Is that sound on the Reference templates, on `minijinja =3.0.0` and `lightningcss =1.0.0-alpha.72`?

## Verdict

**Yes, with caveats.** The mechanism is sound for v1. The #19 probe's CSS half was not: it emitted invalid selectors and could change specificity. Both are fixed here.

- All 68 scoped selectors of the six Reference style blocks come out the same as in the Reference's Astro build, and match the same kinds of element (Chromium 156).
- No scoped selector reached rendered content or another template's elements: 0 of 392 matches. Without the attribute the same selectors would hit 942 more elements.
- The three specificity dependencies resolve as in the Astro build, in either stylesheet order.
- Cost is noise: 34 µs to rewrite all 21 templates, 182 µs to scope all six sidecars, 13 to 31 B of brotli per page, 101 B of brotli CSS.

Caveats, in order of weight:

1. **A class handed to another file's macro does not carry the scope.** Astro passes the cid to every child component; children that spread their props get it (`<time>` in ListRow and BlogPost, `<picture>` and `<img>` in PhotoPile, 19 against 11 stamped elements on `/about/`). B-static never does. No Reference rule depends on it, but the template-check port did: `prose(class="project-body")` and the `post_meta.html` partial both lost their styles until the markup moved back into the styled file. `check` catches both.
2. **The probe's selector rebuild was wrong.** See "Probe bugs" below.
3. **"Matches no literal element" is blind in a template that has one Jinja-built class.** `class="{{ lane }}"` in ListRow may be any class, so a typo in `list_row.css` is not caught.
4. **Quotes that are unbalanced inside each branch of an `{% if %}`** lose the tag end. `check` reports it; the tags after it are not stamped.

## What is here

| Path | What |
|---|---|
| `probe/` | The #19 probe, as it was |
| `site/templates/` | template-check's ports plus the projects index, PhotoPile, a `picture` component and the about page; six sidecars copied verbatim from the Astro `<style>` blocks |
| `src/scope.rs` | Rewriter and offset map (210 lines), literal-element extraction for `check` (90), CSS scoper and validations (190) |
| `src/main.rs` | Commands: `check`, `edges`, `css`, `seeded`, `offsets`, `render`, `bench` |
| `src/check.rs`, `src/site.rs` | template-check's walker and fake site, plus three routes and adversarial content |
| `edge/` | Edge-case templates and CSS |
| `verify/verify.ts` | Playwright check against the Reference's Astro build (`bun verify/verify.ts <astro dist>` after `render`; the build is not in this repo) |
| `results/` | Raw output of every command |

`adversarial.html` and the fake Entry content reuse every class name the sidecars style, marked `data-md`.

## Edge cases (`results/edges.md`, 12 fixtures, all as expected)

The rule: insert ` data-y-<hash>` right after the tag name. The scan never needs the tag end to stamp, only to know where the next tag may start.

| Case | Result | Rule |
|---|---|---|
| Tag split by control flow, `<a {% if x %}class="a"{% endif %}>` | pass | Insert after the name; add a space when the next byte is not whitespace, `>` or `/` |
| `>` or `<` in attribute values, Jinja in values | pass | Quote-aware scan; Jinja delimiters skipped whole and string-aware in every state |
| `<script>`, `<style>`, `<textarea>`, `<title>`, HTML comments, `{# #}`, `{% raw %}` | pass | Bodies copied verbatim; a tag inside `{% raw %}` is not stamped |
| Void and self-closing tags, custom elements | pass | Same rule; `<br data-y-h/>` |
| SVG | pass | Same rule; names keep their case |
| `<html>`, `<head>`, `<body>` | pass | `html` and `body` stamped; `head`, `meta`, `title`, `link`, `base`, `script`, `style`, `noscript`, `template` never; `:root` never scoped |
| `{% include %}` | pass | The included file carries its own id |
| Macro from another file, `caller()` content | pass | Macro body: the macro file's id. Slot content: the id of the file it is written in |
| `{% extends %}` block content, `{% set x %}<a>{% endset %}` | pass | The id of the file the tag is written in (the `set` block is by construction, no fixture) |
| `| safe` strings, safe Values | pass | Never stamped. `| safe` in text position warns; `entry.content` is silent by design |
| `<{{ tag }}>`, `<h{{ n }}>` | pass | Stamped after the expression, with a warning: type selectors cannot be checked |
| `<{% if %}h1{% else %}h2{% endif %}>` | pass | Not stamped, warning |
| Tags in Jinja strings | pass | Never stamped |
| Quotes unbalanced per branch | known limit | `unterminated tag` from `check`; later tags unstamped |

An expression glued to a tag name (`<a{{ attrs|attrs }}>`) reads as a dynamic name. The Reference always has a quoted attribute first.

Stamping every template, not only the six with sidecars, and removing the attributes again gives output byte-identical to the unstamped render on 29 of 29 pages (`results/render.txt`).

## CSS (`results/css-edges.md`)

| Input | Output | Rule |
|---|---|---|
| `.a > * + *` | `.a[h] > [h] + [h]` | Every compound |
| `:hover`, `*` | `:hover[h]`, `[h]` | A compound with no type or class still gets the attribute |
| `.a::before`, `::selection` | `.a[h]:before`, `[h]::selection` | Attribute goes before the pseudo-element |
| `.a:is(.b, .c) .d`, `:not()`, `:has()`, `:where()` | `.a:is(.b, .c)[h] .d[h]` | Arguments are left alone, so `:has(img)` can see content; the element styled is still the template's own |
| `.card { .title {} &:hover {} .dark & {} }` | `.card[h] .title[h]`, `.card[h]:hover`, `.dark[h] .card[h]` | A compound containing `&` is skipped; the parent has the attribute |
| `@media`, `@container`, `@supports`, `@layer` | Children scoped | Names left as written |
| `@keyframes spin`, `animation: spin` | Unchanged | Global, as in Astro |
| `.body > :global(h1)`, `:global(.prose a:hover)` | `.body[h] > h1`, `.prose a:hover` | Unwrapped by the walker |
| `:root`, `html` | `:root`, `html[h]` | The spike's choice; the Reference has no `:root` in a scoped block, so this is not checked against Astro |
| `.card:global(.is-open)` | `.card.is-open[h]` | Correct output, but reported as an error here; it should be a warning |

**Keyframe names are not scoped and should not be.** Astro leaves them global. PhotoPile's script tests `animationName` against `pile-drop` and `pile-fade`, and sidecars use keyframes from the global sheet. `check` should reject a duplicate `@keyframes` name across sidecars.

Rejected by the parser with `css_modules` on: `:global()` (`Invalid empty selector`), `:global .b` (`Ambiguous CSS module class not supported`), `:global(.b, .c)` (`Unexpected token Comma`), an unbalanced `:global(`. `:local()` parses silently, so the walker rejects it. All carry `file.css:line:col`.

### Probe bugs

- `Selector::from(Vec<Component>)` wants each compound in parse order; `iter_raw_parse_order_from` yields it reversed. The probe printed `:hover.list-row[h]`, which happens to be valid. With a type selector the same code prints `:focus-visiblea[h]`, which is not. It also stamped `.a::before` twice.
- The lightningcss `Visitor` is not needed and not used. The scoper walks `ss.rules` itself, so it never sees selectors inside `:is()` or `:has()`.
- A `:global()` left in the tree prints unwrapped, but lightningcss's compatibility check does not know it. At Tailwind v4's default targets it wrapped the list as `:is(.photo[h] picture, .photo[h] img)`. `:is()` takes the highest specificity of its arguments, so a mixed list would change. The walker now unwraps `:global()` itself.

## `check` validations (`results/seeded.md`)

13 of 14 seeded mistakes are reported; the 15th seed is a legal sidecar and stays clean. The unseeded site has 0 diagnostics.

- Caught: a typo class, a type selector for an absent element, a rule for an element in an included partial, a class handed to another file's macro, a child component's element or rendered content styled without `:global()`, five malformed `:global()` or `:local()` forms, `| safe`, a dynamic tag name.
- Missed: the typo class in `list_row.css` (caveat 3).
- Not flagged, correctly: `.photo-tall[data-st='wait']:hover`. Attributes and pseudo-classes are ignored because scripts set them.

## Offsets (`results/offsets.md`)

`check` runs on the original source, so the props pairing and every `check` column are untouched. The rewritten copy is only rendered. The rewriter returns `Vec<(original offset, inserted length)>`; a render error's byte offset goes back through it. Inserted text has no newline, so lines never move.

Seed, with two stamped tags before the error on the same line: `<span>ok</span> <span>{{ entry.reading_tme.x }} min read</span>`

| Path | Location |
|---|---|
| `check`, original source | `layouts/post.html:15:38` |
| `check` if run on the rewritten source | `15:66` (wrong) |
| Render, unmapped | `15:65` (wrong) |
| Render, through the map | `15:37` |
| Render of the original source | `15:37` |

The round trip holds for all 16,804 byte offsets of the 21 templates. Running `check` on rewritten sources also gives 0 diagnostics, so pairing survives either way; only columns need the map.

## Correctness (`results/verify.md`)

Chromium 156 through Playwright 1.64.0, JavaScript off, against the Reference's Astro build with its real global CSS.

- **(a) Own elements only.** 29 pages, 68 selectors, 392 matches, 375 stamped elements. 0 matches on an element written in another template. The selector set equals Astro's (68 against 68) once the attribute name and simple-selector order are normalised. On the five page pairs every exercised rule matches the same kinds of element; on `/about/` and `/projects/` the counts match too (118 stamped elements on `/projects/` in both). The `img` differences on the two Entry pages are hero images that the fake Entries have or lack, not scoping.
- **(b) Content.** 0 matches inside `data-md` content or slot content. The six `:global()` selectors matched 69 elements, 37 of them in content, as intended.
- **(c) Specificity.** `.project` over `.row` at 375 px, `.list-row-arrow` and `.project-arrow` over `.row-arrow`, `.meta-link:hover` over `.text-ink-2`: computed styles equal the Astro build's, also with the scoped sheet loaded before the global one. With the attribute removed and that order, `.project` loses to `.row` (`align-items: center`), so the +0-1-0 is doing real work.
- 26 selectors matched nothing: all need `:hover`, `:focus-visible`, a pseudo-element or state that PhotoPile's script sets.

## Measurements (`results/bench.md`, release build, Apple silicon, one thread)

| | Measured | Budget |
|---|---|---|
| Rewrite, per template | 0.2 to 6.0 µs (Shell, 2,950 B, 36 tags) | |
| Rewrite, all 21 templates | 34 µs | warm build 0.5 s |
| Scope + minify, all six sidecars | 182 µs (PhotoPile 128 µs); 10% over the unscoped pass | warm build 0.5 s |
| Sidecar validations, all six | 176 µs | |
| Post page HTML | +84 B raw, +13 B br (6 attributes) | 14 KB br |
| Post list, 10 rows | +868 B raw, +30 B br (62 attributes) | 14 KB br |
| Projects index, 13 rows | +1,652 B raw, +31 B br (118 attributes) | 14 KB br |
| Sidecar CSS, all six | +1,190 B raw, +101 B br (7,047 B raw, 1,745 B br in all) | 11 KB br |
| Render, 1,125 pages | 26.03 ms plain, 26.18 ms stamped | |
| Edit a sidecar | 145 µs p95, no page re-renders | 50 ms p95 |
| Edit a template | 16 µs p95 for rewrite and compile | 50 ms p95 |

There is no per-page pass. The attribute is template text; the 0.6% render difference is inside run-to-run noise (stamping every template measured faster than plain). Stamping every template, sidecar or not, would cost 91 to 126 B br a page, so stamp only templates that have a sidecar.

## Changes to the design

1. Unwrap `:global()` in the Engine's walker and rebuild compounds in parse order. Do not use the probe's visitor.
2. Stamp only templates that have a sidecar. Never stamp `head`, `meta`, `title`, `link`, `base`, `script`, `style`, `noscript`, `template`. Leave `:root` unscoped.
3. Say in the docs that a sidecar styles only tags written in its own file. A class passed to a macro in another file, an included partial and slot content from a caller are out of reach without `:global()`. Decide whether v1 needs a way to hand the scope to a child (Astro does it implicitly); the Reference does not need one.
4. `check` runs on original sources; the rewriter returns the offset map for render errors.
5. Add to `check`: unterminated tag, duplicate `@keyframes` names, scope-id collisions (6 base36 characters of FNV-1a over the path here), `:local()`.
6. Keyframe, container and layer names stay global.
7. Require a space before an attribute expression after a tag name, or accept the dynamic-name warning.

## Not verified

- Firefox and WebKit. Only Chromium 156 ran.
- Astro's build itself was not re-run; the comparison uses an existing build of the Reference.
- PhotoPile's script against the ported markup, and the 26 state-dependent selectors.
- Astro's documented rule for keyframe names and `:root`. The keyframe finding rests on one build and PhotoPile's script.
- Pixel output. Only selector matches and seven computed-style probes were compared.
- Un-lowered nesting in a browser. The nested fixtures were read, not rendered; the Reference sidecars have no nesting.
- `@scope` and `@starting-style` inside a sidecar, `:host`, `::slotted`, `::part`.
- Whitespace control (`{%-`) next to stamped tags beyond what the ported templates use, and custom delimiters.
- Home, résumé, archive, 404 and feeds were not ported. Shell has no sidecar; it was stamped only in the stamp-everything test.
- The HTML budget against real content: the fake Posts are small, so only the added bytes are meaningful.
