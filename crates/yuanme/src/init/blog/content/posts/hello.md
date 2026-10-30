---
title: Hello
date: 2026-01-01
description: A first Post, with an equation, a theorem and some code.
tags: [meta]
---

This Post is Markdown. Its math is LaTeX, which becomes MathML when the Site is built, so no script draws it.

## An equation

The sum of the first $n$ odd numbers is a square:

$$
\begin{equation}
\sum_{k=1}^{n} (2k - 1) = n^2 \label{eq:odd}
\end{equation}
$$

## A theorem

:::theorem[Gauss]{#thm:gauss}
For every $n \ge 1$, $\sum_{k=1}^{n} k = \frac{n(n+1)}{2}$, and so [](#eq:odd) holds.
:::

:::proof
Each $k$ pairs with $n + 1 - k$, and the $n$ pairs, which count the sum twice, each add up to $n + 1$. Then the odd numbers sum to $2 \cdot \frac{n(n+1)}{2} - n = n^2$.
:::

A link with no text to a Label shows its number, as [](#eq:odd) and [](#thm:gauss) do here. In Org, `\eqref` in prose does the same.

## Some code

```rust
fn odd_sum(n: u64) -> u64 {
    (1..=n).map(|k| 2 * k - 1).sum()
}
```

`hello-org.org` says the same in Org.
