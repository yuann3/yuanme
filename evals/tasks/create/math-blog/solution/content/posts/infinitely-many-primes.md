---
title: There are infinitely many primes
date: 2026-02-01
description: Euclid's proof, in a few lines.
tags: [number-theory]
---

:::theorem[Euclid]{#thm:euclid}
There are infinitely many prime numbers.
:::

:::proof
Suppose $p_1, \dots, p_n$ were all the primes, and let $N = p_1 p_2 \cdots p_n + 1$. No $p_i$ divides $N$, since each leaves remainder $1$. But $N > 1$ has a prime factor, which is then not in the list.
:::

[](#thm:euclid) is the oldest proof most of us still teach.
