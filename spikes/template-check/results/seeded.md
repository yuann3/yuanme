## Unseeded templates

check diagnostics on the correct templates (false positives): 0


Truthiness tests on a possibly-undefined value (legal under SemiStrict, an error under Strict when the value is absent): 13 sites

- `components/base_head.html:18:7`
- `components/list_row.html:11:13`
- `components/list_row.html:8:11`
- `components/pagination.html:3:7`
- `components/pagination.html:5:9`
- `components/pagination.html:6:9`
- `layouts/post.html:13:13`
- `partials/post_meta.html:4:9`
- `partials/post_meta.html:7:9`
- `projects/entry.html:14:13`
- `projects/entry.html:15:13`
- `projects/entry.html:19:13`
- `projects/entry.html:9:26`

Render under SemiStrict: 26/26 pages OK, 0 distinct errors

Render under Strict: 8/26 pages OK, 4 distinct errors
- `partials/post_meta.html:4:9: UndefinedError: `entry.updated` is undefined`
- `projects/entry.html:9:26: UndefinedError: `entry.github` is undefined`
- `projects/entry.html:15:13: UndefinedError: `entry.website` is undefined`
- `projects/entry.html:19:13: UndefinedError: `entry.hero` is undefined (from line 18)`

## Seeded mistakes

| # | Mistake | check | check diagnostic (first relevant) | SemiStrict render | Strict render |
|---|---|---|---|---|---|

check caught 0/0; missed seeds: []. Render caught: SemiStrict 0/0, Strict 0/0 (render only sees executed paths of the fake Entries).
