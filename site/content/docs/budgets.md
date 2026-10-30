---
title: "Budgets"
description: "The Site budgets yuanme build measures every Page against, how to change them, strict mode, and the build report."
---

A Budget is a numeric limit on one measurable property of a Site or of a build, such as JS bytes per Page. `yuanme build` measures every HTML Page against the Site's budgets and writes each number in the build report. A Page over one is a warning, not an error, unless you turn on strict mode: how fast a Site must be is yours to decide.

## The Site budgets

Bytes are counted as Brotli at quality 11, the smallest a host could send, so a live transfer is 10–30% larger. 1 kB is 1,000 bytes. Every Site gets these numbers, the Starters', unless its Site config changes them:

| Budget | Limit | What it counts |
|---|---:|---|
| `html` | 14 kB | The Page's first flight: the whole document, inline CSS and JS included. An Entry's Page leaves out its body, which the report lists apart. |
| `css` | 6 kB | Each stylesheet the Page links or preloads, and each `<style>` element. |
| `js` | 3 kB | Each module Script the Page loads, the chunks they import, which yuanme preloads, classic scripts from the Site's own origin, and inline `<script>`s. A chunk loaded with `import()`, a script from another origin and a data block, such as JSON-LD, don't count. |
| `fonts` | 30 kB | Each font file the Page's stylesheets name in `url()`, or the Page preloads, but the math font, whether or not the Page's text uses it. |
| `math_font` | 55 kB | The math font, on a Page that shows math. |
| `requests` | 4 | The Page itself, and each stylesheet, script, chunk, font and `<link rel="preload">` it loads, from any origin. Preload the image a visitor sees first and it counts too. |

A file counts once per Page, however many times the Page names it. `yuanme check` measures nothing: Budgets need the built Site.

Quality 11 is slow, about a second per MB on one core, so a number under half its limit is estimated at quality 5 instead, which takes about 3% of the time. An estimate is about 15% more than quality 11 would be: on yuanme's test Sites, 13–15% more on HTML and 5–20% more on Entries' bodies, and for one file from 11% less to 31% more. Half the limit leaves room for that, so an estimate is never over a limit, and every number at or over half of one is exact. An Entry's body is estimated when its Page's HTML is.

## Changing them

`[budgets]` in Site config sets any number, in kB, or for `requests` a count. A number left out keeps the Starters':

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[budgets]
css = 11
js = 13
requests = 6
```

`budgets = false`, at the top of `yuanme.toml`, turns them all off: the build measures nothing and warns of nothing.

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
- Each HTML Page's number for every Site budget, an Entry's body apart, and each Budget a Page is over, acknowledged or not.

A build with errors writes nothing into the output directory, but still writes the report: it says the build failed, with its counts and stage times, and lists no Pages. When strict mode failed it, the Budgets are still there.

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

and its Post:

```md check content/posts/hello.md
---
title: Hello
date: 2025-12-12
---

My first Post, with some *emphasis* and a [link](https://commonmark.org).
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

## Budgets

Bytes are Brotli at quality 11. The limits: 14.0 kB of HTML, 6.0 kB of CSS, 3.0 kB of JS, 30.0 kB of text fonts, 55.0 kB of math font, 4 requests. An Entry's body isn't part of its Page's HTML.

| Page | HTML | Body | CSS | JS | Text fonts | Math font | Requests |
|---|---:|---:|---:|---:|---:|---:|---:|
| `/blog/hello/` | 90 B | 83 B | 0 B | 71 B | 0 B | 0 B | 2 |
| `/blog/` | 76 B |  | 0 B | 0 B | 0 B | 0 B | 1 |
```

The 6 files are the two Pages, the bundle, the sitemap's two files and the cloudflare adapter's `_headers`. `report.json` has the same numbers, its keys sorted:

```json
{
  "budgets": {
    "limits": { "css": 6000, "fonts": 30000, "html": 14000, "js": 3000, "math_font": 55000, "requests": 4 },
    "over": [],
    "pages": [
      { "body": 83, "css": 0, "fonts": 0, "html": 90, "js": 71, "math_font": 0, "requests": 2, "url": "/blog/hello/" },
      { "body": null, "css": 0, "fonts": 0, "html": 76, "js": 0, "math_font": 0, "requests": 1, "url": "/blog/" }
    ]
  },
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
- `budgets` has each limit in bytes, or requests; each HTML Page's numbers, where `body` is `null` except on an Entry's Page; and `over`, each Budget a Page is over, with its `value`, `limit`, `allowance` from an Acknowledged overage, and whether that `acknowledged` it. It is `null` when nothing was measured: with `budgets = false`, or when the build failed before it got there.
- After a failed build, `ok` is `false`, `pages` is empty, and `written`, `deleted` and `unknown` are `null`.

Read the report after a change to see what it cost: a Page that grew, a Budget it's now close to, or a module that a new import pulled into a bundle.

## A Page over a budget

With the Site from `yuanme docs getting-started` and the Script from the example above, `/blog/hello/` loads 72 bytes of JS. A budget of 0.05 kB, 50 bytes, is too little:

```toml fail budget/js
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[budgets]
js = 0.05
```

`yuanme build` reports it at the limit, and its help names the Acknowledged overage that would allow it:

```
warning[budget/js]: `/blog/hello/` loads 0.072 kB of JS, over its budget of 0.05 kB
  --> yuanme.toml:10:6
   |
10 | js = 0.05
   |      ^^^^
   |
   = help: make it smaller, raise `budgets.js`, or acknowledge it: `[budgets.overages."/blog/hello/"] js = 0.1`
```

Each Budget has its code, `budget/html`, `budget/css`, `budget/js`, `budget/fonts`, `budget/math-font` and `budget/requests`, and `yuanme docs` explains each with ways to get under it. With the Starters' numbers, the warning is at the start of `yuanme.toml`.

With `--json`, the build prints a `budget` event for each, after the Diagnostics:

```json
{"acknowledged":false,"limit":50,"name":"js","page":"/blog/hello/","passed":false,"type":"budget","value":72}
```

## Acknowledged overages

A Page that is right as it is, though over a budget, can be allowed to go over by a named amount, in the same units:

```toml
[budgets.overages."/blog/hello/"]
js = 0.1
```

That silences the warning while the Page stays within the budget plus 0.1 kB. Over that, it warns again, at the overage. The build report and the `budget` events still list it, as acknowledged. An overage for a URL that is no HTML Page of the Site is `budget/unknown-page`.

## Strict mode

`yuanme build --strict-budgets`, or `strict = true` in `[budgets]`, makes every warning about a Site budget an error: the build exits with `1` and writes nothing into the output directory, but its report still lists each Page's numbers. `--strict` makes every warning an error, Budgets included. Each such Diagnostic gets a note that says which made it an error.

Use it where a slow Page should stop a deploy, such as in CI.
