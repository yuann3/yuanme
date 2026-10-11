check on original sources: 0 diagnostics; check on rewritten sources (every template stamped): 0 diagnostics

seed in `layouts/post.html` (two stamped tags before the error, same line):
```html
      <span>ok</span> <span>{{ entry.reading_tme.x }} min read</span>
```
true column of `entry.reading_tme` in the Author's file: 32

| Path | Diagnostic |
|---|---|
| `check` on the original source | `layouts/post.html:15:38: unknown field `reading_tme` on `entry` (Post); did you mean `reading_time`?` |
| `check` on the rewritten source, unmapped (wrong) | `layouts/post.html:15:66: unknown field `reading_tme` on `entry` (Post); did you mean `reading_time`?` |
| render of the rewritten source, unmapped (wrong) | `layouts/post.html:15:65: UndefinedError: `entry.reading_tme` is undefined` |
| render of the rewritten source, through the offset map | `layouts/post.html:15:37: UndefinedError: `entry.reading_tme` is undefined` |
| render of the original source (reference) | `layouts/post.html:15:37: UndefinedError: `entry.reading_tme` is undefined` |

offset map for `layouts/post.html`: 10 insertions of 14 bytes; inserted text never contains a newline, so line numbers are unchanged and only same-line columns move.
mapped column == reference column: true
round trip original -> rewritten -> original holds for all 16804 byte offsets of the 21 templates.
