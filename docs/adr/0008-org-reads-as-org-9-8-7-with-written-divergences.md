# Org reads as Org 9.8.7's org-element does, with written Org divergences

The Engine reads an Org file the way `org-element` in Org 9.8.7 (bundled with Emacs 31.1) reads it. Where the Org syntax spec and `org-element` disagree, the Engine follows `org-element`, because the spec says it describes that parser. The parser is the organic fork ([#30](https://github.com/yuann3/yuanme/issues/30)). It recognises every element and object type, with spans, so constructs outside the Org subset fail the build at their exact position. Its differential fixtures are regenerated from Org 9.8.7, replacing organic's 2023 pin. Moving to a newer Org release later is a deliberate change that regenerates those fixtures.

Every place the Engine departs from Org 9.8.7 is an Org divergence, and the list is closed. Anything else that differs from Emacs is a bug. Authors write Org in Emacs and check it there, so what they see in Emacs is what the Engine must produce, except where this list says otherwise. Decided in [What is the Org subset, and how do Org and Markdown share one metadata model?](https://github.com/yuann3/yuanme/issues/16).

## The Org divergences

1. **`^:{}` is the default.** `snake_case` stays literal, and only `x_{i}` and `x^{i}` are sub- and superscripts. `#+OPTIONS: ^:t` restores Org's default. Authors who set the option at all mostly turn it off: the corpus has `^:{}` 13 times and `^:nil` 9 times.
2. **The Engine never evaluates code.** `:exports` decides which of a source block and the `#+RESULTS:` already in the file are shown. `#+CALL`, `call_x()` and inline `src_x{}` blocks are errors, because by default they exist to insert evaluated results.
3. **Headlines carry no TODO keyword, priority or tags in the output, and are never numbered.** `*` is `<h2>`, as in ox-html, down to `<h6>` at level 5. `H:` and `num:` have no effect, and options that ask for these features produce a warning.
4. **Org outside the Org subset is a build error.** Emacs would export it.
5. **A field keyword given twice is an error.** Org concatenates two `#+TITLE`s into one.
6. **A pathological paragraph fails the build.** The fork bounds parse time with a fuel limit, and running out is an error with the paragraph's span, never a plain-text fallback.

## Considered Options

- **Follow the spec text where it disagrees with `org-element`** (rejected): `$n$-th` would be math on the Site and plain text in Emacs. The Engine warns on such `$…$` instead and suggests `\(n\)-th`.
- **Track Org's latest release automatically** (rejected): every Org release could change how existing Posts render, without anyone deciding it.
- **Keep Org's defaults everywhere** (rejected): `snake_case` subscripting and TODO keywords in headings are the most common surprises when Org is published as a blog.

## Consequences

- An Org divergence is added only through an ADR or a decision ticket, and the docs list every one of them.
- Org's own dash rules hold: `--` is an en dash and `---` an em dash. The shared smartypants runs on Org in that mode, which corrects [#18](https://github.com/yuann3/yuanme/issues/18)'s assumption that Org gets the same dash rules as Markdown.
- Emacs stays a fixture oracle only and never runs at build time (ADR 0003).
