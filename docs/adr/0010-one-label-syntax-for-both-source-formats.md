# Cross-references use LaTeX Labels and empty-text links in both Source formats

Equations are labelled with `\label{…}` inside the math, in Markdown and in Org alike, and cited with `\eqref{…}` inside math. Theorem blocks and other elements take a Label through the Source format's own attribute syntax (`{#thm:x}` on a Markdown directive, `#+NAME:` in Org). A link with no text to a Label, such as `[](#thm:x)` in Markdown or `[[thm:x]]` in Org, gets the target's number filled in by the Engine: "Theorem 3" or "(2)". In Org, `\ref{…}` and `\eqref{…}` written in prose work too, because Org parses them as LaTeX. Every Label in an Entry shares one namespace with its heading ids, so two of them with the same name are a build error. A reference to a Label that does not exist is also a build error.

Authors who write mathematics already know `\label` and `\eqref`, and in Org they are the convention: the corpus has `\label` in 12 Org files and `\eqref`/`\ref` in 11. Markdown has no shared convention. Pandoc and Quarto use `{#eq-x}` and `@eq-x`, MyST uses roles, and in our corpus only 5–8 Markdown files use any of them. One syntax, written the same way in both Source formats, is what an agent can learn from one page of docs. The empty-text link is MyST's convention, and it is valid CommonMark that degrades to an ordinary link. Decided in [How do Authors write math, theorems and cross-references in each Source format?](https://github.com/yuann3/yuanme/issues/17).

## Considered Options

- **Pandoc/Quarto syntax in Markdown** (`$$ … $$ {#eq-x}`, `@eq-x`) (rejected): it gives Markdown a second syntax that Org cannot share, and `@word` collides with handles and email-like prose.
- **CSS counters for the numbers** (rejected): the numbers would not be in the markup, could not be linked to or read in feeds, and restart per Page. math-core already writes the numbers as text, and the charter settled that math needs no client JS.
- **Numbering across a series or Collection** (rejected): a Post read on its own would start at (7). Numbers restart in each Entry, as they do in each LaTeX document.

## Consequences

- The Engine needs the Label-to-number map that math-core keeps private, and `\ref`, which math-core lacks. Both go upstream, and the Engine carries a pinned patch until they land.
- Links to another Entry's Labels (`[](other.md#thm:x)`) work through the Site-wide link index from [#16](https://github.com/yuann3/yuanme/issues/16) and fill in the target's number.
- Labels become ids exactly as written, so they are limited to letters, digits, `:`, `_`, `.` and `-`. Anything else is an error that suggests a valid Label.
