# Runtime template engine for ADR 0002

Ticket: [#6](https://github.com/yuann3/yuanme/issues/6). Researched 2026-10-04 against minijinja 2.24.0 (plus 3.0.0-alpha.2) and tera 2.4.0, from crates.io metadata, the published crate sources, the upstream changelogs and docs, and a throwaway Rust spike (`/private/tmp/tpl-spike`, Apple M4 Max, `cargo build --release`).

## Question

Which Rust runtime template engine should the Engine use for the Jinja-style templates promised by [ADR 0002](../adr/0002-runtime-templates.md)? Compare render speed, error messages and spans, whether the variables a template uses can be statically analysed (so `check` can validate them against a Collection's schema), auto-escaping, macros/includes/inheritance, how components are expressed, LLM familiarity (closeness to Jinja2), maintenance health and hot reload.

## Answer

**Use minijinja.** Pin it exactly (`=2.24.x` today; move to 3.0 once it is stable), and enable the `unstable_machinery` feature so the Engine can walk the template AST.

1. **It keeps ADR 0002's main premise: Jinja2 syntax that LLMs already know.** minijinja tracks Jinja2 on purpose. Most 2026 releases are "for Jinja2 compatibility" fixes, and its compatibility document lists only small gaps. Tera 2 says outright that it "intentionally deviates from Jinja2/Django in many ways". It also removed macros and replaced them with a JSX-like component syntax, released in June 2026, which coding agents trained before then have not seen.
2. **It is the only candidate where the ADR's schema check can be built without forking.** minijinja exposes its parser and AST (semver-exempt) and gives a nested `undeclared_variables`. In the spike, a 130-line AST walker resolved loop variables (`p` in `for p in posts` → `posts[].title`) and reported `list.html:4:36: unknown field posts[].titel`. Tera 2's AST is crate-private. Its `get_template_variables` returns only top-level names (`post`, not `post.title`).
3. **What Tera 2 does better can be added on top of minijinja; what minijinja does better cannot be added to Tera 2.** Tera 2 renders about 1.7–1.9× faster, but both take microseconds per page. Tera 2 has nicer stock error text and typed component props. minijinja gives every error a kind, template name, line and byte range, which is all the Engine needs to print its own rustc-style and JSON diagnostics. Prop typing can live in the Engine's `check` layer, as a props declaration that the AST walker validates.

**Runner-up: Tera 2.** Its components (typed props, `body` slot, `...rest`, introspection through `get_component_definition`) are the closest thing in Rust to Astro components. Choose it instead if typed components out of the box turn out to matter more than Jinja2 fidelity and AST-level `check`. Do not choose it before an Agent eval shows that agents write Tera 2 component syntax correctly.

## Comparison

| Criterion | minijinja 2.24.0 | tera 2.4.0 |
|---|---|---|
| Current version / release cadence | 2.24.0 (2026-08-12); 3.0.0-alpha.2 (2026-09-23). 23 releases since 2025-01 ([crates.io](https://crates.io/crates/minijinja/versions)) | 2.4.0 (2026-09-11). 2.0.0 shipped 2026-06-26 after alphas from 2026-02 ([crates.io](https://crates.io/crates/tera/versions)) |
| Render, 300-post list via macro/component | 300 µs | 177 µs |
| Render, 300-post list, inline loop | 188 µs | 99 µs |
| Render, one 34 KB post | 6.6 µs | 4.0 µs |
| Load + compile 5 templates | 60 µs | 146 µs |
| Error output | Kind + name + line + byte range on `Error`. With `set_debug(true)`: source window, caret, and a dump of referenced variables | rustc-style report with caret, "Available fields: …", include-chain notes. `ReportError` exposes message, filename and `Span` (lines, cols, byte range) |
| Unknown filter detected | At render | At template add (compile time) |
| Static variable analysis | `undeclared_variables(nested)` per template, e.g. `site.description`. Full AST via `unstable_machinery` | `get_template_variables` follows extends/include but gives top-level names only. AST is private |
| Undefined variables | Lenient by default; `UndefinedBehavior::Strict` available | Error by default; optional chaining `a?.b` |
| Auto-escape default | `.html .htm .xml` (+ `.json/.js/.yaml` with `json` feature). Escapes `& < > " ' /` (`'`→`&#x27;`, `/`→`&#x2f;`). Replaceable via `set_formatter` | `.html .htm .xml`. Escapes `& < > " '` (`'`→`&#39;`), the same set as Astro's text expressions. Replaceable via `set_escape_fn` |
| Inheritance / include | `extends`, `block`, `super()`, `required` blocks, `include`: parity with Jinja2 | `extends`, `block`, `include`. `include ignore missing` removed |
| Macros / components | Jinja2 `macro`, `import`, `from … import`, `call` + `caller()` as a slot. No `varargs`/`kwargs` | No macros. `{% component %}` with typed args, defaults, `...rest`, `@implicit` params, `body` slot. Called as `{{<ui.button label="x" />}}` / `{% <card> %}…{% </card> %}` |
| Jinja2 fidelity (LLM familiarity) | High: [COMPATIBILITY.md](https://github.com/mitsuhiko/minijinja/blob/main/COMPATIBILITY.md); used to run real-world HF chat templates | Low and getting lower: keyword-only filter args (`default(value=…)`), no macros, new component syntax |
| Hot reload | `path_loader` is lazy. `minijinja-autoreload` (notify-based). Rebuilding the Environment costs ~60 µs | `full_reload()` re-reads the glob (feature `glob_fs`). No watcher |
| Maintenance | 100+ commits to main since 2026-07-04; 18 open issues; Armin Ronacher (1397 of the commits) | 48 commits since 2026-07-04; 5 open issues; Vincent Prouillet (364). Zola moved to Tera 2 in 0.23.0 |
| License | Apache-2.0 | MIT |

Benchmark numbers are means over 2000 renders with a prebuilt context, from the spike described under [Render speed](#render-speed).

## Candidates considered

- **minijinja** and **tera**: the two maintained runtime Jinja-style engines ([crates.io template-engine category, sorted by recent downloads](https://crates.io/categories/template-engine?sort=recent-downloads)). Recent downloads: minijinja 11.9 M and tera 6.3 M (crates.io API, 2026-10-04).
- **askama** (and **rinja**, now `0.4.0+deprecated` and folded back into askama, see [crates.io/crates/rinja](https://crates.io/crates/rinja)): compiled templates. ADR 0002 rejects these.
- **upon 0.11**: runtime and Jinja-flavoured, but it has no inheritance, no macros and no HTML escaping by default, and it uses Liquid-style filter arguments (`| replace: "\t", " "`) ([README](https://docs.rs/crate/upon/0.11.0/source/README.md)). Out.
- **handlebars**, **liquid**: runtime, but not Jinja-style. Out under ADR 0002.
- **jinja 0.1.1** ([crates.io](https://crates.io/crates/jinja)): about 3 k downloads in total. Too immature.
- **tera 1.x**: superseded. 1.20.1 (2025-10-30) is the last 1.x release, and Zola has left it.

## Details

### Render speed

Spike: equivalent templates for both engines. They use a `base.html` shaped like the Reference site's Shell (`data-state`/`data-section` on `<html>`, rail nav loop, `[data-panel-content]` main, included footer), a `PanelHeader` component with an actions slot, and a `row` component per post. Post titles contain `& < > " '`. Strict undefined for minijinja.

```
== list of 300 posts
minijinja load+compile 5 templates             60.3 us
tera2 load+compile 5 templates                146.0 us
minijinja blog.html   (macro per row)         299.7 us/render  (83308 bytes)
minijinja inline.html (loop only)             187.7 us/render
minijinja post.html   (34 KB body | safe)       6.6 us/render
tera2 blog.html       (component per row)     176.8 us/render  (82971 bytes)
tera2 inline.html                              99.4 us/render
tera2 post.html                                 4.0 us/render
```

With 20 posts the ratio is the same (minijinja 26 µs, tera 13.5 µs). If the context is serialized from serde on every render, both pay the same extra cost: +80 µs for a 300-post context, which is more than the render itself. So the Engine should build each Collection's `Value` once and share it between pages, whichever engine it uses.

Conclusion: Tera 2 is faster (its migration guide claims 2–4× over Tera 1, [MIGRATION.md § Performance](https://github.com/Keats/tera/blob/master/MIGRATION.md)). For a static site the difference does not matter: 1,000 list-heavy pages × 0.3 ms is 0.3 s on one core, and both `Environment` and `Tera` render behind `&self` and are `Send + Sync` (checked in the spike), so pages can render in parallel.

### Error messages and spans

Spike output, abridged.

Typo in a nested field inside an included template:

```
tera2:
error: Field `autor` is not defined. Available fields: author, description, footer_links, sections, title
 --> footer.html:2:18
2 |   &copy; {{ site.autor }}
  |                  ^^^^^
note: called from page.html:2:12

minijinja (Strict, set_debug(true)):
could not render include: error in "footer.html" (in page.html:2)
  caused by: undefined value (in footer.html:2)
   2 >   &copy; {{ site.autor }}
     i             ^^^^^^^^^^ undefined value
Referenced variables: { site: { …entire map dumped… } }
```

Component or macro called with a misspelled argument:

```
tera2:     error: Unknown argument(s) `titel` in component call. Possible argument(s) are: `title`  --> p.html:2:5
minijinja: too many arguments: unknown keyword argument `titel` (in p.html:3)   [span 41..57]
tera2:     error: Component argument `title` (type: `i64`) does not match expected type: `string`
```

- Tera 2's stock messages are better: they name the missing field, list the alternatives, and add an include-chain note. They are built from `ReportError { message, filename, span }`, which exposes `Span { start_line, start_col, end_line, end_col, range }` ([errors.rs](https://docs.rs/crate/tera/2.4.0/source/src/errors.rs), [utils.rs](https://docs.rs/crate/tera/2.4.0/source/src/utils.rs)).
- minijinja's `Error` exposes `kind()`, `name()`, `line()`, `range()` (byte range), `detail()` and `template_source()`, with the include chain as `source()` ([error.rs](https://docs.rs/crate/minijinja/2.24.0/source/src/error.rs)). The debug report is on only when `set_debug(true)` is set. It defaults to `cfg!(debug_assertions)`, so release builds of the Engine must turn it on ([environment.rs](https://docs.rs/crate/minijinja/2.24.0/source/src/environment.rs)). Its "Referenced variables" dump becomes noise with real Collection contexts. The undefined error says "undefined value" and does not say which attribute was missing.
- Syntax errors are mediocre in both. For an unclosed `{% for %}`, Tera points at end of input and minijinja points at line 3. Neither points at the opening tag.
- minijinja reports unknown filters only at render time. Tera 2 rejects them when the template is added ([MIGRATION.md](https://github.com/Keats/tera/blob/master/MIGRATION.md): "Tera also now checks at compile-time that all functions/tests/filters/components are present").

For the Engine this matters less than it seems. ADR 0002 promises compile-style diagnostics, and an agent-native Engine also needs machine-readable output (for example JSON with file, line, col and message). Either way the Engine formats its own diagnostics from structured errors, and both engines provide byte ranges. The quality gap then comes down to wording, such as adding "did you mean `author`?" from the schema, and the Engine can do that because it knows the schema.

### Static analysis for `check`

ADR 0002: "the Engine validates template variables against the content schema at build time". Doing that needs every field path a template reads, resolved through loops, `set` and includes.

- **minijinja**: `Template::undeclared_variables(nested: bool)`. With `nested = true` it returns dotted paths (`site.description`, `post.title`). It works per template and does not follow `extends`/`include`/`import` ([template.rs](https://docs.rs/crate/minijinja/2.24.0/source/src/template.rs), doc comment on `undeclared_variables`). The `unstable_machinery` feature exports `machinery::{parse, ast, Span, tokenize, …}` with "no semver guarantees" ([lib.rs](https://docs.rs/crate/minijinja/2.24.0/source/src/lib.rs)). 3.0.0-alpha.2 still exports both.
  - Spike: a 130-line walker over `ast::Stmt`/`ast::Expr` maps loop targets to `<path>[]`, follows `set` aliases, skips `loop` and imported namespaces, and checks against a schema path set. Output on a template with three seeded typos:
    ```
    list.html:4:36: unknown field `posts[].titel` (not in content schema)
    list.html:8:12: unknown field `site.autor` (not in content schema)
    list.html:8:56: unknown field `site.sections[].lable` (not in content schema)
    ```
    The same walker can follow `Extends`/`Include`/`Import` nodes (their names are expressions in the AST), check filter and test names against the registry, and check macro calls against a props declaration.
- **tera 2**: `Tera::get_template_variables` follows `extends` and nested `include`, but returns only top-level names. For the spike's `post.html` it returned `["description", "post", "section", "site"]` and no fields. Its doc says it "does a best-effort to find the top level variables" ([tera.rs](https://docs.rs/crate/tera/2.4.0/source/src/tera.rs)). The parser and AST live in a private `mod parsing` ([lib.rs:77](https://docs.rs/crate/tera/2.4.0/source/src/lib.rs)). Field-level checking would mean a fork or an upstream API.
  - Tera 2 does error on an undefined field at render time, and so does minijinja in Strict mode. So "render every page in `check`" catches typos on paths that the current content actually executes. It misses branches that no content reaches yet, such as `{% if post.hero %}{{ post.hero.alt }}{% endif %}` while no post has a hero. Only static analysis covers those.

### Auto-escaping, compared with the Reference site

The [parity inventory](astro-parity-inventory.md) (§2, "Escaping") lists three escaping schemes. Template text expressions use html-escaper, which escapes `& < > ' "` to `&amp; &lt; &gt; &#39; &quot;`.

- Tera 2 `escape_html` escapes exactly `& < > " '`, with `'` → `&#39;` ([utils.rs:108](https://docs.rs/crate/tera/2.4.0/source/src/utils.rs)). The spike confirmed it byte for byte against that scheme.
- minijinja escapes `'` → `&#x27;` and also `/` → `&#x2f;` ([utils.rs:329-337](https://docs.rs/crate/minijinja/2.24.0/source/src/utils.rs)). So every `href="/about"` filled from a variable comes out as `href="&#x2f;about"`, which renders the same but changes the bytes. A 20-line `Environment::set_formatter` reproduces html-escaper exactly; the spike verified this.
- Neither engine knows whether it is in text or attribute context. Astro's attribute scheme (only `&` and `"`; http(s) URLs left raw) needs either a Parity exception or an explicit filter. Inventory question 19 (the snapshot bar) decides which.
- minijinja ≥2.22 renders `none` as `None` for Jinja2 compatibility ([CHANGELOG 2.22.0](https://github.com/mitsuhiko/minijinja/blob/main/CHANGELOG.md)). In the spike, a `null` description rendered `content="None"`. The custom formatter should render none as empty, or the Engine should leave absent optional frontmatter undefined rather than null. Tera rendered `""`.
- minijinja strips one trailing newline per template, like Jinja2 (`set_keep_trailing_newline`). Tera keeps it. This matters only for the byte-identical bar.

### Inheritance, includes, macros and components

- **minijinja**: `extends`, `block`, `include`, `import`, `macro`, `call`, `with`, `set`, `filter`, `autoescape` and `raw` all have "feature parity with Jinja2". The exceptions: `include` has no `with/without context`, and macros have no `varargs`/`kwargs` ([COMPATIBILITY.md](https://github.com/mitsuhiko/minijinja/blob/main/COMPATIBILITY.md)). 2.20 added Jinja `required` blocks ([CHANGELOG](https://github.com/mitsuhiko/minijinja/blob/main/CHANGELOG.md)).
  - A component is a macro, and a slot is `caller()`:
    ```jinja
    {% macro panel_header(title) %}<header>…{% if caller is defined %}<div class="actions">{{ caller() }}</div>{% endif %}…</header>{% endmacro %}
    {% call ui.panel_header("Writing") %}<a href="/archive/">ARCHIVE</a>{% endcall %}
    ```
    In Strict mode, `{% if caller %}` errors when a macro is called without a call block, so the test must be `caller is defined`. Agents get this wrong unless the docs say so.
  - Missing compared with Astro components: typed props, rest props (`...attrs`, because of no `kwargs`) and named slots. The Engine can add props on top. Options: a `{# props: title: string, actions?: slot #}` header that `check` parses, or a sidecar schema per component file. Either way `check` validates every call site with the AST walker. Named slots can be done as macro arguments holding `{% set %}` blocks.
  - Without varargs, pass-through attributes have to be an explicit `attrs={…}` map.
- **tera 2**: `extends`, `block`, `include`. "Macros are gone… replaced with components" ([MIGRATION.md](https://github.com/Keats/tera/blob/master/MIGRATION.md)). Components:
  - are global, so no import is needed;
  - have optional types (`string, bool, integer, float, number, array, map, bytes`), defaults, `...rest`, metadata, and `@implicit` parameters resolved from the caller's context (2.4.0);
  - get their content as `body`;
  - are called JSX-style.
  - `get_component_definition` returns typed `ComponentInfo` ([components.rs](https://docs.rs/crate/tera/2.4.0/source/src/components.rs), [tests/introspection.rs](https://docs.rs/crate/tera/2.4.0/source/tests/introspection.rs), [docs](https://keats.github.io/tera/)). In the spike, both a misspelled prop and a wrongly typed prop produced a pointed error.
  - This is the strongest argument for Tera 2. Zola 0.23 also uses Tera templating inside Markdown content in place of shortcodes ([Zola CHANGELOG 0.23.0](https://github.com/getzola/zola/blob/master/CHANGELOG.md)), which is a ready-made model for Astro-style components in Posts.

### LLM familiarity

ADR 0002's justification is "LLMs also know Jinja syntax extremely well". That holds for Jinja2 itself and for any engine close to it.

- Jinja2's corpus is very large: the syntax of Ansible, Flask, dbt, Salt and Hugging Face chat templates. As a rough proxy, GitHub code search reports about 750 k `.j2` files and 156 k `.jinja` files ([search](https://github.com/search?q=extension%3Aj2&type=code)). For Tera, about 5 k `config.toml` files that look like Zola's (`base_url` + `compile_sass`) and 441 `zola.toml` (same search API, 2026-10-04). These counts are only orders of magnitude.
- minijinja is used to execute real-world Python-authored Jinja2: Hugging Face text-generation-inference's router depends on it for chat templates ([router/Cargo.toml](https://github.com/huggingface/text-generation-inference/blob/main/router/Cargo.toml); the repo has been archived since 2026), and `rattler_build_jinja` is "powered by minijinja" ([crates.io](https://crates.io/crates/rattler_build_jinja)). Its 2026 releases are mostly Jinja2-compatibility fixes (dotted integer lookup, chained comparisons, `None`/`True` rendering, `indent` kwargs, round-half-even, floor division semantics; [CHANGELOG](https://github.com/mitsuhiko/minijinja/blob/main/CHANGELOG.md)). Agents write Python-isms such as `x.items()` and `str.upper()`. `minijinja-contrib`'s `pycompat::unknown_method_callback` accepts those ([COMPATIBILITY.md § Python Methods](https://github.com/mitsuhiko/minijinja/blob/main/COMPATIBILITY.md)).
- Tera 2 diverges in ways an agent writing Jinja2 will trip on. Filters and tests take keyword arguments only (`default(value=x)`, while Jinja2 writes `default(x)`; in the spike the Jinja2 form had to be rewritten). There are no macros and no `caller`. `x.0` is gone. Several filters were renamed (`escape` → `escape_html`, `divisibleby` → `divisible_by`). There is new syntax: `{{< />}}`, `?.`, spreads ([MIGRATION.md](https://github.com/Keats/tera/blob/master/MIGRATION.md)). Tera 2.0 shipped on 2026-06-26, which is at or after the training cutoff of current agents. One data point: the agent writing this note (knowledge cutoff June 2026) did not know Tera 2's component syntax before reading the migration guide.

### Maintenance health

- **minijinja**: Armin Ronacher's project; he has 1397 commits and the next contributor has 10 ([contributors API](https://github.com/mitsuhiko/minijinja/graphs/contributors)). It has Python, Go, JS and C bindings in the same repo. 100+ commits since 2026-07-04, the last push on 2026-10-04, 18 open issues and 3 open PRs. Release cadence is roughly monthly. 3.0 is in alpha: serde becomes optional, `State` becomes mutable, and some deprecated APIs are removed ([CHANGELOG 3.0.0-alpha.0](https://github.com/mitsuhiko/minijinja/blob/main/CHANGELOG.md)). MSRV is 1.70.
- **tera**: Vincent Prouillet's project; he has 364 commits and the next contributor has 5. 48 commits since 2026-07-04 and 5 open issues. Tera 2 is new and moving fast: 2.0 → 2.4 in 11 weeks, and minor releases have changed behaviour. 2.2 errors on content in child templates that would have been ignored. 2.3 changed how escaping applies to included templates ([CHANGELOG](https://github.com/Keats/tera/blob/master/CHANGELOG.md)). Zola (same maintainer) is its main consumer.
- Both are effectively single-maintainer, and both are healthy today.

### Hot reload

This does not separate the two. The Engine's dev server already has to watch content, CSS and JS (parity inventory: "dev server with rebuild on change"), and rebuilding either environment is cheap (60 µs vs 146 µs for five templates in the spike).
- minijinja: `path_loader` loads lazily. `Environment::clear_templates()` exists. `minijinja-autoreload` wraps the pattern with a notify watcher (`watch-fs` feature, on by default) ([lib.rs](https://docs.rs/crate/minijinja-autoreload/2.24.0/source/src/lib.rs)).
- Tera: `full_reload()` re-reads the glob passed to `load_from_glob` (`glob_fs` feature) ([tera.rs](https://docs.rs/crate/tera/2.4.0/source/src/tera.rs)). Zola 0.23.5 still "rebuild[s] whole site on any template change in `zola serve`" ([Zola CHANGELOG](https://github.com/getzola/zola/blob/master/CHANGELOG.md)).
- Either way, `check` needs the template dependency graph (extends/include/import) for incremental rebuilds. With minijinja the Engine gets that graph from the AST walk it already does.

## Integration notes if minijinja is adopted

- Pin the exact version (`=2.24.0`), because `unstable_machinery` is semver-exempt. Isolate the AST walker behind one Engine module so a minijinja upgrade touches one place.
- Set `UndefinedBehavior::Strict` and `set_debug(true)`. Install a custom formatter for html-escaper parity and `none` → empty. Enable `loop_controls`. Consider `minijinja-contrib` with `pycompat` so Python-isms from agents work.
- Convert each Collection to a `minijinja::Value` once and share it between pages.
- Document `caller is defined` in the component guide, and pick a props-declaration convention for `check`.

## Open questions

1. Should `check` also validate component (macro) props, and in what syntax: a `{# props #}` header in the template, a sidecar schema, or Rust-side registration? This decides how far the Engine closes the gap with Tera 2's typed components.
2. Can templating be used inside Post bodies (MDX-like components in Markdown/Org)? Zola 0.23 chose this for Tera 2. If yuanme wants it, the Org subset needs a defined escape or embedding syntax.
3. Should the Engine start on minijinja 2.24 or wait for 3.0? 3.0 changes the `Value`/serde integration that the Engine's content model will touch, and it is still alpha (2026-09-23).
4. Escaping parity: should Astro's attribute-context scheme be reproduced with a filter, or written down as a Parity exception? This depends on inventory question 19.
5. Run an Agent eval with a few template tasks (add a component with a slot, add a Collection list page) against minijinja-style docs. It would confirm the familiarity claim with data instead of argument. Running it against Tera 2 as well would settle the runner-up.
