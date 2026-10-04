# Excluded candidates

| Candidate | Reason |
|---|---|
| quarto-dev/quarto-web (Quarto docs: cross-references, theorems) | No content licence: `license.qmd` covers only the CLI and editor, and the repo has no LICENSE. Used quarto-cli test `.qmd` files (MIT) instead. |
| rstudio/bookdown `inst/examples` (bookdown book, theorem envs, `\@ref`) | The book text is CC BY-NC-SA 4.0 (NonCommercial), even though the repo is GPL-3.0. |
| mentat-collective/fdg-book (Functional Differential Geometry in Org) | CC BY-NC-SA 4.0. |
| ncordon/math-notes (Org math notes with theorem blocks) | Repo LICENSE says GPL-3.0, but every note embeds a `by-nc-sa.png` badge. The licences conflict. |
| iliayar/ITMO (Russian lecture notes, about 120 Org files with theorem blocks) | GLWTS licence. It is non-standard and asks reusers to leave no trace of the author, which conflicts with recording the source. |
| jethrokuan/braindump, wugouzi/notes (Org math notes) | No licence. |
| jkitchin/jkitchin.github.com (Kitchin blog in Org, math) | The LICENSE file is an HTML page. No clear content licence. |
| sachac/sachac.github.io, jethrokuan/blog | No licence file in the repo. |
| opsxcq/blog, invenia/blog, yilinmo/yilinmo.github.io, stanford-cs324/winter2022 | The MIT LICENSE names the theme or template author (Steve Francia, Barry Clark, Mark Otto, Kevin Lin), not the post authors. |
| high-dimensional-statistics.github.io | MIT, but whether it covers the exercise posts is unclear. Skipped. |
| leanprover-community/blog | No licence. |
| include-yy/egh0bww1 `republish/` and `tr-*` posts, `projecteuler/` | Republished or translated works by others. Project Euler problem texts are CC BY-NC-SA. Only the author's own `posts/` were taken. |
| b40yd/b40yd.github.io | GPL-3.0, but probably inherited from the theme. Enough other Org blogs. |
| Worg `org-configs/org-customization-survey*.org` | Generated and large (1 MB / 220 KB). |
| opsxcq `bitcoin-nonce-reuse-attack.org` (900 KB), `tj-kaczynski-manifesto.org` | Too big, and not the author's work. The repo is excluded anyway. |
| eigenric/prove-it | Fetch failed (Unicode path normalisation). Not retried. |
