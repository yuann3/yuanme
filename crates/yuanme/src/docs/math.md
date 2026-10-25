---
title: Math and Theorem blocks
description: LaTeX math in Markdown and Org, rendered to MathML at build time, with numbered equations, Labels, references and macros.
---

yuanme converts every piece of math in an Entry to MathML when it builds the Site. A Page carries no TeX and no math script: browsers render MathML themselves. Math that can't be converted fails the build with `math/unsupported`; yuanme never falls back to showing the TeX.

yuanme doesn't add a math font to the Site yet, so browsers draw math with their own fonts.

## Math in Markdown

- `$…$` is inline math, and so is ``$`…`$``, whose contents Markdown never touches.
- `$$…$$` is display math, on lines of its own or inside a paragraph.
- A fenced code block whose language is `math` is display math.
- An environment such as `align` goes inside `$$…$$` or a `math` block.
- A `$` followed by a space, or a closing `$` followed by a digit, isn't math, so prices such as `$5 and $6` stay text. `\$` is always a dollar sign.

````md check
---
title: Energy
---

Mass and energy are related by $E = mc^2$, where $c$ is the speed of light.

$$
\int_0^1 x^2 \, dx = \frac{1}{3}
$$

```math
\begin{aligned}
(a + b)^2 &= a^2 + 2ab + b^2 \\
(a - b)^2 &= a^2 - 2ab + b^2
\end{aligned}
```

A coffee costs \$4.
````

Markdown doesn't read LaTeX's own delimiters: `\(…\)` and `\[…\]` are escaped brackets, as on GitHub and in Pandoc, and a bare `\begin{…}` is text. A paragraph with one of them is the warning `math/tex-delimiter`:

```md fail math/tex-delimiter
---
title: Energy
---

The area is \(\pi r^2\).
```

## Math in Org

- `\(…\)` is inline math and `\[…\]` is display math.
- `$…$` and `$$…$$` are math where Org reads them as math; prefer `\(…\)`, which always is.
- An environment on lines of its own, from `\begin{equation}` to `\end{equation}`, is display math. It must be one that stands alone: `equation`, `align`, `gather` or `multline`, starred or not. One that goes inside math, such as `cases` or `pmatrix`, is `math/unsupported` on its own; put it inside `\[…\]`.

```text check content/posts/energy.org
#+TITLE: Energy

Mass and energy are related by \(E = mc^2\).

\[ \int_0^1 x^2 \, dx = \frac{1}{3} \]

\begin{align*}
(a + b)^2 &= a^2 + 2ab + b^2 \\
(a - b)^2 &= a^2 - 2ab + b^2
\end{align*}
```

## Environments

These stand alone as display math, starred or not:

- `equation`
- `align`
- `gather`
- `multline`

These go inside math, in either Source format:

- `aligned` and `gathered`
- `cases`, `dcases`, `rcases` and `drcases`
- `matrix`, `pmatrix`, `bmatrix`, `Bmatrix`, `vmatrix` and `Vmatrix`
- `array`, `darray` and `subarray`

Any other environment is `math/unsupported`, and the help names a replacement: `align` for `eqnarray`, `alignat` and `flalign`, and `aligned` inside `equation` for `split`.

```md fail math/unsupported
---
title: Energy
---

$$
\begin{eqnarray}
a &=& b
\end{eqnarray}
$$
```

## Numbered equations

`equation`, `align`, `gather` and `multline` number their lines, as amsmath does. Numbers start at 1 in each Entry and follow the order the equations are written in.

- A line with `\notag` or `\nonumber` has no number.
- `\tag{*}` gives a line its own tag, `(*)`, in place of a number.
- Starred forms, `$$…$$` and `\[…\]` without a numbered environment are never numbered.

The number is text in the MathML, in the last cell of the equation's row, so it shows in feeds and in a Page read without CSS:

```html
<math display="block"><mtable …><mtr>…<mtd … id="eq:energy"><mtext>(1)</mtext></mtd></mtr></mtable></math>
```

A paragraph that holds only display math is written without its `<p>`.

## Labels and references

An equation is labelled with `\label{…}` inside the math, in Markdown and Org alike, and referred to with `\eqref{…}`, which shows its number in parentheses and links to it. A reference may come before the equation it names.

````md check
---
title: Energy
---

By $\eqref{eq:energy}$, a little mass is a lot of energy.

$$
\begin{equation}
E = mc^2 \label{eq:energy}
\end{equation}
$$

```math
\begin{align}
p &= mv \label{eq:momentum} \\
F &= ma \notag
\end{align}
```

Momentum is $\eqref{eq:momentum}$.
````

In Org, `\eqref{…}` and `\ref{…}` also work in prose, outside math: `\eqref` shows `(1)` and `\ref` shows `1`. Inside math, write `\eqref`; `\ref` there is `math/unsupported`. In Markdown, write `$\eqref{…}$`.

```text check content/posts/energy.org
#+TITLE: Energy

\begin{equation}
E = mc^2 \label{eq:energy}
\end{equation}

Equation \ref{eq:energy} is the famous one, and \eqref{eq:energy} says why.
```

An Org equation takes its Label from `\label`, never from `#+NAME`, so `\eqref` finds it the same way in both Source formats:

```text fail math/named-environment content/posts/energy.org
#+TITLE: Energy

#+NAME: eq:energy
\begin{equation}
E = mc^2
\end{equation}
```

The rules for Labels:

- A `\label` goes on a numbered line. One in `$$…$$` or `\[…\]` alone is `math/unsupported`.
- A Label is letters, digits, `:`, `_`, `.` and `-`, because it becomes an `id` as written. Anything else is `math/invalid-label`, which suggests a valid one.
- Every Label in an Entry shares one namespace with the ids written on its headings (`{#id}` in Markdown, `:CUSTOM_ID:` in Org), and with Org's `#+NAME`s and `<<targets>>`. Two with the same name are `math/duplicate-label`. A prefix that says what it labels, such as `eq:` or `sec:`, keeps them apart. A heading id yuanme makes from a title never takes a Label's name.
- An `\eqref` to a Label no equation in the Entry has, to an equation without a number, or to something that isn't an equation is `math/unknown-label`. When a Label with a similar name exists, the help suggests it.

```md fail math/unknown-label
---
title: Energy
---

$$
\begin{equation}
E = mc^2 \label{eq:energy}
\end{equation}
$$

By $\eqref{eq:enrgy}$, a little mass is a lot of energy.
```

A numbered equation's Label is an id on the Page, so an ordinary link reaches it: `[the energy equation](#eq:energy)` in Markdown, `[[eq:energy][the energy equation]]` in Org, and `[the energy equation](energy.md#eq:energy)` from another Entry. A link with no text to a Label doesn't get the equation's number filled in yet, so give the link text, or use `\eqref`.

## Macros

`[math.macros]` in Site config defines macros for every Entry's math. Each key is a macro's name without its backslash; `#1`, `#2` and so on are its arguments. TOML's single-quoted strings keep backslashes as they are.

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[math.macros]
R = '\mathbb{R}'
norm = '\left\lVert #1 \right\rVert'
rank = '\operatorname{rank}'
```

```md check
---
title: Norms
---

For $v \in \R^n$, $\norm{v} \ge 0$, and $\rank A \le n$.
```

Inside an Entry:

- `\newcommand`, `\renewcommand` and `\def` in any piece of math define a macro for the rest of that Entry.
- `\newcommand` for a command yuanme's math already has is `math/unsupported`; use `\renewcommand` to change it.
- `\DeclareMathOperator` isn't available in math; declare the operator in `[math.macros]` as `\operatorname{…}`, as `rank` is above.

In Org, `#+LATEX_HEADER:` lines in the file define macros for its math: `\newcommand`, `\renewcommand`, `\providecommand`, `\def` and `\DeclareMathOperator`, starred or not. A `\usepackage` of `amsmath`, `amssymb`, `amsfonts`, `amsthm`, `mathtools` or `bm` is accepted, since yuanme's math has what they give. Any other `#+LATEX_HEADER` is the warning `org/no-effect`.

```text check content/posts/energy.org
#+TITLE: Probability
#+LATEX_HEADER: \usepackage{amsmath,amssymb}
#+LATEX_HEADER: \newcommand{\E}{\mathbb{E}}
#+LATEX_HEADER: \DeclareMathOperator{\Var}{Var}

\(\Var X = \E[X^2] - \E[X]^2\) for every \(X\) with \(\E[X^2] < \infty\) in \(\R\).
```

## Diagnostics

- `math/unsupported`: LaTeX yuanme can't render or that isn't valid, such as an unknown command or environment, a group never closed, or a misplaced `\label`.
- `math/unknown-label`: a reference to no numbered equation in the Entry.
- `math/duplicate-label`: two things in one Entry with the same Label.
- `math/invalid-label`: a Label with a character an id can't take as written.
- `math/named-environment`: an Org environment labelled with `#+NAME`.
- `math/tex-delimiter`: a warning for `\(…\)`, `\[…\]` or a bare environment in Markdown.

`yuanme docs CODE` explains each, such as `yuanme docs math/unknown-label`.

## Theorem blocks

yuanme doesn't render Theorem blocks yet: they get no number, title or Label. Until it does, `:::theorem` in Markdown and `#+begin_theorem` in Org are plain blocks, each a `<div class="theorem">`, and a `[title]` on a Markdown directive is the warning `markdown/no-effect`. A `\begin{theorem}` or `\begin{proof}` inside math is `math/unsupported`.
