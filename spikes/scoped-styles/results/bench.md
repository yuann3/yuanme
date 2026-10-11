## Template rewrite at load (median of 2001 runs)

| Template | Source B | Tags stamped | Rewrite | Added B |
|---|---|---|---|---|
| `about.html` | 884 | 7 | 3.33 µs | 98 |
| `adversarial.html` | 1172 | 19 | 4.25 µs | 266 |
| `components/base_head.html` | 1387 | 0 | 3.79 µs | 0 |
| `components/formatted_date.html` | 220 | 1 | 0.42 µs | 14 |
| `components/list_row.html` (sidecar) | 1146 | 7 | 2.25 µs | 98 |
| `components/pagination.html` (sidecar) | 347 | 3 | 0.75 µs | 42 |
| `components/panel_header.html` | 708 | 9 | 1.83 µs | 126 |
| `components/photo_pile.html` (sidecar) | 924 | 5 | 1.46 µs | 70 |
| `components/picture.html` | 700 | 4 | 1.00 µs | 56 |
| `components/post_list.html` | 364 | 1 | 0.46 µs | 14 |
| `components/wordmark.html` | 262 | 2 | 0.42 µs | 28 |
| `layouts/post.html` (sidecar) | 1390 | 9 | 2.25 µs | 126 |
| `layouts/shell.html` | 2950 | 36 | 5.96 µs | 504 |
| `partials/copy_btn.html` | 76 | 0 | 0.21 µs | 0 |
| `partials/film_css.html` | 132 | 0 | 0.25 µs | 0 |
| `posts/entry.html` | 132 | 0 | 0.17 µs | 0 |
| `posts/list.html` | 522 | 0 | 0.46 µs | 0 |
| `projects/entry.html` (sidecar) | 1183 | 9 | 2.04 µs | 126 |
| `projects/index.html` (sidecar) | 1193 | 10 | 2.21 µs | 140 |
| `series/term.html` | 513 | 0 | 0.50 µs | 0 |
| `tags/index.html` | 578 | 1 | 0.67 µs | 14 |

sum of medians, all 21 templates: 34.7 µs. One pass over all templates: stamp-all median 33.9 µs / p95 37.8 µs; sidecar-only (the design; others are scanned for nothing here) median 32.3 µs.

## Sidecar CSS (parse + scope + minify + print, Tailwind v4 default targets; median of 501)

| Sidecar | Source B | Unscoped min B / br | Scoped min B / br | Added B / br | Transform | Unscoped transform |
|---|---|---|---|---|---|---|
| `components/list_row.css` | 250 | 151 / 93 | 181 / 107 | +30 / +14 | 5.0 µs | 4.6 µs |
| `components/pagination.css` | 110 | 95 / 73 | 125 / 92 | +30 / +19 | 4.0 µs | 3.6 µs |
| `components/photo_pile.css` | 5863 | 4143 / 1303 | 4888 / 1337 | +745 / +34 | 128.3 µs | 116.6 µs |
| `layouts/post.css` | 256 | 142 / 97 | 197 / 109 | +55 / +12 | 6.4 µs | 5.9 µs |
| `projects/entry.css` | 532 | 391 / 163 | 511 / 179 | +120 / +16 | 12.2 µs | 10.5 µs |
| `projects/index.css` | 1229 | 935 / 326 | 1145 / 355 | +210 / +29 | 25.7 µs | 23.0 µs |
| **all 6, concatenated** | | 5857 / 1644 | 7047 / 1745 | +1190 / +101 | 181.6 µs | |

sidecar validations for all 6 (scan template, collect literal elements, parse CSS, match compounds): 175.6 µs.

## HTML bytes (brotli q11)

| Page | Plain raw / br | Stamped raw / br | Added raw / br | Attributes | Stamp-every-template raw / br (added) |
|---|---|---|---|---|---|
| Post page (`/blog/post-0001/`) | 8309 / 1393 | 8393 / 1406 | +84 / +13 | 6 | 9261 / 1493 (+952 / +100) |
| Post list, 10 rows (`/blog/`) | 13316 / 1662 | 14184 / 1692 | +868 / +30 | 62 | 15192 / 1788 (+1876 / +126) |
| Projects index, 13 rows | 14026 / 1601 | 15678 / 1632 | +1652 / +31 | 118 | 16532 / 1700 (+2506 / +99) |
| About (PhotoPile) | 8463 / 1720 | 8617 / 1739 | +154 / +19 | 11 | 9793 / 1846 (+1330 / +126) |
| Project page | 6302 / 1523 | 6428 / 1546 | +126 / +23 | 9 | 7282 / 1614 (+980 / +91) |

## Render cost (1125 pages, 1,000 fake Posts, one thread, median of 9)

- plain: 26.03 ms, 10372702 bytes, 23.14 µs a page
- stamped (sidecar templates): 26.18 ms, 10561408 bytes, 23.27 µs a page
- stamped (every template): 25.38 ms, 11555464 bytes, 22.56 µs a page
- compile all templates: plain 85.3 µs, stamped 83.7 µs

No per-page pass exists: the attribute is template text. The render delta is the cost of writing the extra bytes.

## Incremental (p95 of 501)

- edit the largest sidecar (`photo_pile.css`): re-scope it, 144.6 µs; no template is reloaded and no page re-renders (the id comes from the path).
- edit the largest template (`layouts/shell.html`): rewrite + minijinja compile, 15.5 µs (pages that use it re-render as they would without scoping).
