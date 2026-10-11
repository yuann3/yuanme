# Spike: checked templates on minijinja 3.0.0

Throwaway code for [#19](https://github.com/yuann3/yuanme/issues/19), "How do Authors write templates and components, and how is template context typed?". Not Engine code.

## Question

ADR 0002 promises that templates are validated against the content schema at build time, with compile-style diagnostics. Does the provisional #19 design deliver that on real templates, on `minijinja =3.0.0` with `unstable_machinery`?

## Verdict

**Yes, with four gaps.** A 1,400-line AST walker (`src/check.rs`) checks the ported Reference templates without rendering them.

- It caught **45 of 48** seeded mistakes, each as `file:line:col: message`, most with a did-you-mean.
- It raised **0 false positives** on the 18 ported templates, and 1 on 30 extra legitimate idioms.
- Rendering under SemiStrict caught 29 of the 48; under Strict 27 beyond the errors Strict raises on the correct templates.
- `check` takes 2.0 ms as a process. Rendering 1,122 pages for 1,000 fake Posts takes 23 ms on one thread.

The gaps:

1. **`object` props are opaque.** A typo on an Entry field inside a macro body is missed when the prop is `list<object>` (seed 15). The spike added schema-reference types (`entry<posts>`, `term`, `paginator`) and then catches it (seed 14).
2. **Filter arguments are unchecked.** A misspelt keyword (`dateformat(fromat=…)`, seed 32) fails only at render. An invalid `dateformat` format string (`"iso"`, seed 33) never fails: it prints `iso`.
3. **Layouts have undeclared inputs.** `layouts/shell.html` reads `title`, `section` and others that the extending template sets. Nothing in the provisional design declares them, so the spike added a `{# vars: … #}` header.
4. **Narrowing follows `if`, `else`, `and`, inline `if` and loop filters, not control flow.** `{% if not p.updated %}{% continue %}{% endif %}{{ p.updated }}` is flagged although it renders.

## What is here

| Path | What |
|---|---|
| `site/templates/` | Shell, BaseHead, Post layout, Post list, Pagination, ListRow, PostList, tags index, a series Term page and the Project page, ported to Jinja |
| `src/check.rs` | The walker |
| `src/site.rs` | Hand-written schema after #18's renames, fake Entries, context Values, `sort_by`, `attrs`, the Environment |
| `src/seeds.rs` | The 48 seeded mistakes |
| `src/main.rs` | Commands: `check`, `render`, `seeded`, `fp`, `probe`, `sort`, `bench N` |
| `results/` | Raw output of every command |

Run with `cargo run --release -- <command>`. `render` writes pages to `out/` (ignored).

## Results

### Seeded mistakes (`results/seeded.md`)

| | Caught |
|---|---|
| `check` | 45 / 48 |
| Render, SemiStrict | 29 / 48 |
| Render, Strict (errors beyond its baseline) | 27 / 48 |

`check` caught 17 mistakes that a SemiStrict render let through, among them:

- a typo swallowed by `| default`, and one in a branch the Entries never reach;
- a macro body reading `entries` from the importer's context (minijinja leaks it, so the page renders);
- an `int` passed to a `url` prop and a list passed to a `string` prop;
- an override of a block the layout does not have;
- `sort(attribute="-count")`, which minijinja silently treats as an unknown field.

### False positives

- 0 diagnostics on the 18 ported templates. The templates and the walker were written together: one template was edited to get there (`entry.website != entry.github|default("")`), because SemiStrict itself rejects `!=` on an undefined value.
- `results/fp-stress.md`: 30 legitimate idioms in the Post route. The first run flagged 5; else-branch narrowing, loop-filter narrowing and untyped local macros fixed 4. The `continue` early exit remains.

### SemiStrict against Strict (`results/undefined-probe.md`)

- The correct templates contain **13** truthiness tests on a possibly-absent value, such as `{% if entry.hero %}`. SemiStrict accepts all 13. Strict rejects each one whenever the value is absent.
- At render, SemiStrict passes 26 of 26 pages. Strict passes 8 of 26, with 4 distinct errors.
- Under SemiStrict `==` and `!=` on an undefined value are still errors. `check` enforces this; the Project page needs `entry.website != entry.github|default("")`.
- An undefined value passed to a macro argument takes the argument's default.

### `sort_by` (`results/sort_by.txt`)

`sort_by(keys)`: `-` for descending, missing values last, ties by Slug.

| Ordering | Keys | Against a port of the Reference's sort |
|---|---|---|
| Blog list | `["order", "-modified"]` | match (8 hand-written Posts and 1,000 fake ones) |
| Series page | `["modified"]` | match |
| Series page | `["date"]` | **differs** when a Post has `updated`: the Reference sorts on `updatedDate ?? pubDate` |
| Projects | `["order", "-date"]` with `order: 1..13` | match |
| Terms | `["-count"]` | **differs on ties**: the Reference keeps first-seen order, `sort_by` breaks ties by Slug |

The Reference's four tags all have count 1 today, so the tags index order changes unless #23 rules on it. Projects need `order` up to 13; #18's example caps `order` at 5.

### Timing (`results/timing.txt`, release build, Apple silicon, one thread)

| Step | Time | Budget |
|---|---|---|
| `check` process, 18 templates, 5 route kinds (hyperfine) | 2.0 ms | 200 ms |
| of which parse and walk | 0.13 ms | |
| Context construction for 1,000 Posts (Values, sorts, taxonomies, 1,122 route contexts) | 2.7 ms | |
| Compile all templates | 0.1 ms | |
| Render 1,122 pages, 10.2 MB | 22.6 ms (20 µs a page) | 1 s for the whole build |

Context construction is about 11% of construction plus render. Parsing content is not in these numbers.

Template checking walks templates against the schema, never against Entries, so its 2.0 ms is the same at 1,000 Posts. Against the `check` budgets (200 ms for the Reference site, 1 s for 1,000 Posts) it leaves almost everything for content validation, which this spike did not measure.

## Awkward or impossible in minijinja 3.0.0

| Point | Workaround |
|---|---|
| Comments never reach the AST | Scan raw source for `{# props: … #}`; pair with the next top-level `Macro` whose span starts after it with only whitespace and `{%` between. A stray comment is an error. |
| Imported macros see the importer's context | `check` rejects any free variable in a macro body that is not a prop or one of `site`, `data`, `collections`, `taxonomies`, `page`. The runtime does not enforce it. |
| No `kwargs` or `varargs` | An explicit `attrs` map prop and an `attrs` filter. |
| A parameter without a default cannot follow one with a default (syntax error) | Required props first. An optional prop takes `=none` or a real default. |
| A missing required argument has no call-site error at render | `check` reports it at the call site. |
| `{% block %}` inside `{% call %}` or a macro is a syntax error | Layout nesting must be `extends`; a component's default slot cannot hold the content block. |
| A layout's inputs are top-level `set`s in the extending template | A `{# vars: … #}` header on the layout, checked per route. |
| Kwarg names, filter names, test names and `Macro.name` have no span; a nested `GetAttr` span starts at the dot | Columns computed from end offsets and the source text. |
| The filter registry is not listable | A hand-kept name list in the walker. It can drift from the Environment. |
| Escaping is not context-aware | `| safe` inside `<style>`; a custom formatter for html-escaper output and `none` as empty. |
| Macro output keeps its own indentation; `trim_blocks` leaves blank lines | Accepted, since Parity is on the normalized DOM. |
| Detecting whether a macro takes a default slot | The spike greps the body's `Debug` output for `caller`. The Engine needs a proper walk. |

## Recommended changes to the design

1. Keep **SemiStrict**. Strict rejects the dominant idiom of the Reference templates.
2. Let props name schema types: `entry<posts>`, `term`, `paginator`. Plain `object` stays as the unchecked escape hatch.
3. Add a **`{# vars: … #}` header for layouts**, with the same grammar as `props`.
4. Give the Engine registry **signatures**, not just names, so filter keyword arguments are checked. Wrap `dateformat` to reject an unknown format.
5. Make `check` reject Python method calls (`x.lower()`), `==` on an optional value, and `sort` with a `-` key.
6. Document the guard forms `check` understands, and that an early `continue` is not one.
7. Decide the Term tie-break and whether series pages sort on `date` or `modified`.

## Not verified

- Parity with the Reference site's output. The render was read by eye for structure only.
- Home, about, résumé, archive, 404, feeds, `llms.txt` and the projects index were not ported.
- The `check` budget against real content validation; only template checking was timed.
- `set` inside one `if` branch, recursive macros, include fallback lists and `import` cycles.
- Whether the hand-kept filter and test lists match minijinja's built-ins exactly.
- minijinja 2.24: everything ran on `=3.0.0`.
