---
title: "Budgets"
description: "The build report yuanme writes after every build, what it lists, and the state of Budgets."
---

A Budget is a numeric limit on one measurable property of a Site or of a build, such as JS bytes per Page. yuanme doesn't check Site budgets yet: it measures no Budget against a limit, takes no Acknowledged overages, and has no Site config for either. `yuanme schema events` lists a `budget` event, which no command prints yet.

What yuanme does now is measure each build and write the numbers down, in the build report.

## The build report

Every `yuanme build` writes two files in `.yuanme/`, replacing the previous build's:

- `report.md`, for a person to read.
- `report.json`, the same numbers for a script or an agent.

`yuanme check` writes no report.

The report lists:

- Whether the build succeeded, and its counts of errors and warnings.
- How long each stage took, in milliseconds: `load` reads the project, `build` renders the Site in memory, and `write` syncs it into the output directory.
- How many files the Site has, and their bytes in all, the Host adapter's files included.
- How many files the build wrote and deleted, and the files in the output directory that yuanme didn't write and left as they are.
- Each Page's URL and the size of its file, largest first. Every Route counts, feeds included.
- Each Script bundle in `assets/`, its size, and the modules in it, in the order they run, with each module's size before minifying, so the modules can add up to more than the bundle. A package from `node_modules/` shows up here by its path.

A build with errors writes nothing into the output directory, but still writes the report: it says the build failed, with its counts and stage times, and lists no Pages.

## An example

With the Site from `yuanme docs getting-started`, a Template that loads a Script:

```jinja check templates/posts/entry.html
<!doctype html>
<html lang="en">
<title>{{ entry.title }}</title>
<h1>{{ entry.title }}</h1>
{{ entry.content }}
<script type="module" src="/scripts/site.ts"></script>
</html>
```

```ts check scripts/site.ts
import { greet } from "./greet.ts";

document.querySelector("h1")?.append(greet("reader"));
```

```ts check scripts/greet.ts
export function greet(name: string): string {
  return `, hello ${name}`;
}
```

`yuanme build` then writes a `report.md` like this, with your own sizes and times:

```markdown
# Build report

2 Pages and 6 files, 1.0 kB in all. 0 errors, 0 warnings.

| Stage | Time |
|---|---:|
| load | 1 ms |
| build | 6 ms |
| write | 0 ms |

6 files written, 0 deleted.

## Pages

| Page | Size |
|---|---:|
| `/blog/hello/` | 246 B |
| `/blog/` | 115 B |

## Scripts

`assets/site-a11d77f55f068aa0.js`, 85 B

| Module | Size |
|---|---:|
| `scripts/greet.ts` | 72 B |
| `scripts/site.ts` | 74 B |
```

The 6 files are the two Pages, the bundle, the sitemap's two files and the cloudflare adapter's `_headers`. `report.json` has the same numbers, its keys sorted:

```json
{
  "bytes": 1015,
  "deleted": 0,
  "errors": 0,
  "files": 6,
  "ok": true,
  "pages": [
    { "bytes": 246, "url": "/blog/hello/" },
    { "bytes": 115, "url": "/blog/" }
  ],
  "scripts": [
    {
      "bytes": 85,
      "file": "assets/site-a11d77f55f068aa0.js",
      "modules": [
        { "bytes": 72, "module": "scripts/greet.ts" },
        { "bytes": 74, "module": "scripts/site.ts" }
      ]
    }
  ],
  "stages": { "build_ms": 6, "load_ms": 1, "write_ms": 0 },
  "unknown": [],
  "warnings": 0,
  "written": 6
}
```

- `pages`, `scripts` and `unknown` are lists; `written` and `deleted` are counts.
- After a failed build, `ok` is `false`, `pages` is empty, and `written`, `deleted` and `unknown` are `null`.

Read the report after a change to see what it cost: a Page that grew, or a module that a new import pulled into a bundle.

## Strict mode

`yuanme check --strict` and `yuanme build --strict` make every warning an error, so the command exits with `1` on any Diagnostic, and `build` writes nothing. Each such Diagnostic gets a note that `--strict` made it an error. It applies to every warning, such as `link/broken` or `host/unsupported`; there is no strict mode for Budgets alone, since yuanme doesn't check them yet.
