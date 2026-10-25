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

A Theorem block is a theorem, lemma, definition, proof or another kind, written in the Source format's own block syntax: a block directive in Markdown, a special block in Org. Its title, if it has one, can hold markup and math, and its Label is a `{#…}` in Markdown and a `#+NAME:` in Org, in the same namespace as every other Label in the Entry:

```md check content/posts/primes.md
---
title: Primes
---

:::theorem[Fermat's little theorem]{#thm:fermat}
If $p$ is prime, then $a^p \equiv a \pmod p$.
:::

::::proof
By induction on $a$.

:::lemma[$p$ divides $\binom{p}{k}$]{#lem:binom}
For $0 < k < p$.
:::
::::
```

```text check content/posts/groups.org
#+TITLE: Groups

#+NAME: lem:lagrange
#+begin_lemma Lagrange
The order of a subgroup divides the order of the group.
#+end_lemma

#+begin_proof
Cosets partition the group.
#+end_proof
```

Blocks are numbered in the order they are written, starting again in each Entry. The built-in kinds:

- `theorem`, `lemma`, `proposition`, `corollary`, `conjecture`, `definition`, `example` and `exercise` are numbered on one counter: Theorem 1, Lemma 2, Definition 3.
- `remark` and `note` aren't numbered.
- `proof` isn't numbered and ends with ∎. Its title replaces "Proof": `:::proof[Proof of Theorem 1]`.

`[math.theorems]` in Site config changes a kind or adds one. `label` is what its blocks are called, and `counter` is the counter they're numbered on: `"theorem"`, a name of the kind's own, or `false` for none. Here claims count on their own, and remarks join the shared counter:

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

[math.theorems.claim]
label = "Claim"
counter = "claim"

[math.theorems.remark]
counter = "theorem"
```

A block's name and title run into its first paragraph, and a proof's ∎ ends its last:

```html
<div class="theorem" data-kind="lemma" id="lem:lagrange">
<p><span class="theorem-head"><span class="theorem-name">Lemma 1</span> <span class="theorem-title">(Lagrange)</span>.</span> The order of a subgroup divides …</p>
</div>
<div class="proof">
<p><span class="proof-head">Proof.</span> Cosets partition the group. <span class="qed" role="img" aria-label="End of proof">∎</span></p>
</div>
```

When a block starts with display math or a list, its head is a paragraph of its own, and when a proof ends with one, ∎ is in a `<p class="qed">` after it. A block whose name isn't a kind is a plain `<div class="NAME">`, and a title on it is the warning `markdown/no-effect` or `org/no-effect`. A `\begin{theorem}` or `\begin{proof}` inside math is `math/unsupported`.
