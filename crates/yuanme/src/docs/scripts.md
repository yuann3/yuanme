---
title: Scripts
description: Load TypeScript or JavaScript modules from a Template; yuanme bundles them, with packages from node_modules, into minified and hashed chunks.
---

A Script is a JS or TS module that runs in the visitor's browser. A Template loads one with `<script type="module" src>` naming a file of the project, and yuanme bundles it with everything it imports, minifies it, writes it under `/assets/` with a hash in its name, and rewrites the `src`. There is no Node.js to install: yuanme compiles TypeScript itself.

## Load a Script

Keep Scripts anywhere in the project outside `static/`, such as `scripts/`. A Script is a `.ts`, `.js`, `.mts` or `.mjs` file:

```ts check scripts/site.ts
const year: number = new Date().getFullYear();
document.querySelector("footer")?.append(` ${year}`);
```

Name it from a Template by its path from the project root, with a leading `/`:

```jinja check templates/posts/entry.html
<!doctype html>
<html lang="en">
<head>
<title>{{ entry.title }}</title>
<script type="module" src="/scripts/site.ts"></script>
</head>
<h1>{{ entry.title }}</h1>
{{ entry.content }}
<footer>Example</footer>
</html>
```

```md check content/posts/hello.md
---
title: Hello
---

Some text.
```

`dist/blog/hello/index.html` then loads `/assets/site-0123456789abcdef.js`. A `src` follows the same rules as a stylesheet's `href`: a path from `/` names a project file or a Static file, other origins are left as written, and a `?query` is kept. `yuanme docs styles` has them all.

What yuanme leaves alone:

- A `<script src>` of a Static file or of another origin, which loads as written, module or not.
- An inline `<script>`, whose text is not bundled, so it can't import project files.
- A data block, such as `<script type="application/ld+json">`.

A `<script src>` of a project file without `type="module"` is `asset/classic-script`, because yuanme bundles every Script as a module:

```jinja fail asset/classic-script templates/posts/entry.html
<!doctype html>
<html lang="en">
<head>
<title>{{ entry.title }}</title>
<script src="/scripts/site.ts"></script>
</head>
{{ entry.content }}
</html>
```

A module is deferred and runs in strict mode. A file that must stay a classic script goes in `static/`.

## TypeScript

yuanme strips the types from `.ts` and `.mts` files when it bundles them. The bundler doesn't read `tsconfig.json`, so its `paths` don't apply to imports.

yuanme doesn't compile JSX either. JSX in a Script is `asset/jsx`; build the element with the DOM, or write it in a Template.

### Check types

Turn on `[typescript] check` in Site config, and `yuanme check` and `yuanme build` check the types of every TypeScript Script with TypeScript 7's `tsc`. Each type error is `asset/type-error`, which fails the build, at the name or value TypeScript puts it on, with TypeScript's message after its own code:

```toml
[typescript]
check = true
```

```text
scripts/site.ts:6:7: error[asset/type-error]: TS2322: Type 'string' is not assignable to type 'number'.
```

What is checked is what Pages load: each Script and Sidecar script, what they import, and every `.d.ts` file of the project, which may declare globals. A `.ts` file that no Page loads isn't checked.

The first check downloads `tsc` 7.0.2 with `curl` from TypeScript's GitHub release, about 9 MB, or from the npm registry if that fails, and checks its SHA-256. No Node.js is needed. It is kept in the same cache folder as the Tailwind CLI (`yuanme docs styles`); in CI, cache that folder. To check offline, set `YUANME_TSC_BIN` to the `tsc` of TypeScript's 7.0.2 release, with the `lib.*.d.ts` files beside it as the release has them.

The result is kept in `.yuanme/cache` under every TS, JS and JSON file of the project, so checking a project that hasn't changed doesn't run `tsc`. `package.json` stands for `node_modules`, which isn't read.

### tsconfig.json

Every yuanme command writes `.yuanme/types/yuanme.d.ts`, the types of `yuanme:router`, the Router's events, `mount`'s `ctx` and Data file imports. A `tsconfig.json` at the project root that includes it gives your editor the types the check uses:

```json
{
  "compilerOptions": {
    "allowJs": true,
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "skipLibCheck": true,
    "noEmit": true,
    "paths": { "/*": ["./*"] }
  },
  "include": ["**/*.ts", ".yuanme/types/yuanme.d.ts"],
  "exclude": ["dist", "static", "node_modules"]
}
```

`paths` makes an import from the project root, such as `/data/nav.yaml`, name a project file, as it does for the bundler. A Site without a `tsconfig.json` is checked with these options; with one, with its `compilerOptions`. Either way the check sets its own files, and `noEmit`. TypeScript 7 is strict by default. A Data file's import has the type `any`.

## Imports

A Script imports other project files by a relative path, or by a path from the project root starting with `/`. As in TypeScript, `./util` finds `util.ts` or `util.js`, `./util.js` also finds `util.ts`, and `./lib` finds `lib/index.ts`. TS and JS files import each other freely:

```js check scripts/greet.js
export function greet(who) {
  console.log(`Hello, ${who}`);
}
```

```ts check scripts/site.ts
import { greet } from "./greet";

greet(document.title);
```

### JSON and Data files

A Script may import a `.json` file of the project, and any Data file, as its value. A Data file gives the same value Templates see as `data.NAME`, with its schema's defaults, and dates as ISO text:

```yaml check data/nav.yaml
- title: Blog
  url: /blog/
```

```ts check scripts/site.ts
import { greet } from "./greet";
import nav from "/data/nav.yaml";

greet(`${nav.length} links`);
```

### Packages

An import of a bare name, such as `nanoid`, is a package in the project's `node_modules`, resolved for the browser, so a package's `browser` build is the one used:

```ts
import { nanoid } from "nanoid";
```

Add packages with Bun, which writes `package.json` and `node_modules`:

```sh
bun add nanoid
bun install
```

yuanme reads `node_modules` and never runs a package manager. Only the project's own `node_modules` counts, not one in a parent folder. A project that has a `package.json` but no `node_modules` gets `asset/dependencies-not-installed` for each import of a package:

```json check package.json
{
  "dependencies": {
    "nanoid": "^5.0.0"
  }
}
```

```ts fail asset/dependencies-not-installed scripts/site.ts
import { nanoid } from "nanoid";

console.log(nanoid());
```

An import of another origin, such as `https://esm.sh/…`, is left in the bundle as written.

### Stylesheets

A Script can't import CSS: yuanme bundles the stylesheets that Pages link, so an imported one would never reach a Page. Import it from a linked stylesheet instead (`yuanme docs styles`):

```css check styles/site.css
body { margin: 0 }
```

```ts fail asset/css-in-script scripts/site.ts
import "/styles/site.css";
```

## Bundling and shared chunks

Every Script of the Site is bundled in one build:

- Each Script a Page loads becomes its own file, keeping its exports.
- Code that two Scripts import goes into a shared chunk, which both import, so a visitor downloads it once.
- A module loaded with `import()` gets a chunk of its own, fetched when the code runs.
- Each Page gets a `<link rel="modulepreload">` for every chunk its Scripts import, directly or through other chunks, so the browser fetches them at once. They go after the Page's stylesheets.

The code is minified for the browsers in `[build.targets]` (`yuanme docs styles`), and licence comments stay. A chunk's name is its name with a hash of its code and of every chunk it imports, so it changes whenever any of them does.

## Sidecar scripts

A Sidecar script is a Script with the same name as a Template, beside it, such as `templates/partials/clock.ts` beside `templates/partials/clock.html`:

```jinja check templates/partials/clock.html
<time class="clock"></time>
```

```ts check templates/partials/clock.ts
export function mount() {
  const tick = () => {
    for (const el of document.querySelectorAll(".clock")) {
      el.textContent = new Date().toLocaleTimeString();
    }
  };
  tick();
  const timer = setInterval(tick, 1000);
  return () => clearInterval(timer);
}
```

```jinja check templates/posts/entry.html
<!doctype html>
<html lang="en">
<head>
<title>{{ entry.title }}</title>
<link rel="stylesheet" href="/styles/site.css">
<script type="module" src="/scripts/site.ts"></script>
</head>
<h1>{{ entry.title }}</h1>
{% include "partials/clock.html" %}
{{ entry.content }}
</html>
```

You don't load a Sidecar script. Each Page that rendered the Template loads it through a module yuanme writes for the Page, `/assets/page-….js`, with a `<script type="module">` just after the Page's last `<link rel="stylesheet">`, or else just before `</head>`; a Page with neither gets none. Pages that rendered the same Templates share that module, and a Page that didn't render the Template doesn't load its Sidecar script.

### mount

A Sidecar script's top level runs once per document. Do a Page's work in an exported `mount` instead, which yuanme calls on every Page load that rendered the Template: once after a full page load, and again each time the Router swaps in another Page that rendered it. `mount` gets one argument, `ctx`:

- `ctx.url`: the Page's URL, as a `URL`.
- `ctx.signal`: an `AbortSignal` that aborts before the Router swaps the Page out. A listener added with `{ signal: ctx.signal }` goes away with the Page.
- `ctx.data`: an object of the Sidecar script's own, which lasts from Page to Page until the next full load.

`mount` may return a function, which runs when the signal aborts: the clock above stops its timer there. In TypeScript, `MountContext` from `yuanme:mount` types `ctx`, and takes the type of `ctx.data`:

```ts check templates/partials/clock.ts
import type { MountContext } from "yuanme:mount";

export function mount(ctx: MountContext<{ ticks?: number }>) {
  const tick = () => {
    ctx.data.ticks = (ctx.data.ticks ?? 0) + 1;
    for (const el of document.querySelectorAll(".clock")) {
      el.textContent = new Date().toLocaleTimeString();
    }
  };
  tick();
  const timer = setInterval(tick, 1000);
  return () => clearInterval(timer);
}
```

With the Router off, each move between Pages is a full page load, so `mount` runs once per load and its cleanup never runs. A `mount` that throws is reported in the browser's console, and the other Sidecar scripts still mount.

## The Router

`[router] mode` in Site config picks how a visitor moves between Pages:

- `"off"`, the default: every move is a full page load.
- `"native"`: full page loads, which the browser animates with a cross-document View Transition. yuanme adds `<style>@view-transition{navigation:auto}</style>` to every Page, and no JS. Style the animation with `::view-transition-old()` and `::view-transition-new()` in a stylesheet.
- `"swap"`: the Router on every Page.

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[router]
mode = "swap"

[budgets]
requests = 5
```

The Router is a Script and a chunk of its own, so with a stylesheet and a Script of the Site's, each Page makes 5 requests on its render path, one more than the Starters' budget: `yuanme docs budgets` explains it.

With `"swap"`, the Router catches each move to another Page of the Site, fetches the Page, and swaps it into the current document, so what the Site keeps between Pages survives. It:

- replaces `<body>`, `<html>`'s attributes and `<head>`, keeping the stylesheets both Pages link, and loading new ones before the swap;
- runs the new Page's Scripts that haven't run in this document, including inline ones, and mounts its Sidecar scripts;
- scrolls to the top, or to the element the URL's `#fragment` names, and Back and Forward restore where the Page was scrolled;
- announces the new Page's title to screen readers.

Browsers without the Navigation API, older than Chrome 105, Firefox 147 and Safari 26.2, get full page loads. So does a move to another origin, to a file such as `/feed.xml`, a download, a form, a reload, a page that doesn't run the Router, such as an HTML Static file, and a fetch that fails. The Router itself is at most 2 KB brotli-compressed.

### Keep an element

An element with `data-yuanme-persist` moves into the next Page in place of the element with the same value there, instead of being replaced, so it keeps its state: a `<canvas>` keeps its drawing and WebGL context, and a focused link stays focused. A Page without that value gets its own element:

```jinja check templates/posts/entry.html
<!doctype html>
<html lang="en">
<head>
<title>{{ entry.title }}</title>
<link rel="stylesheet" href="/styles/site.css">
<script type="module" src="/scripts/site.ts"></script>
</head>
<canvas data-yuanme-persist="background"></canvas>
<h1>{{ entry.title }}</h1>
{% include "partials/clock.html" %}
{{ entry.content }}
</html>
```

### Full page loads and prefetching

`data-yuanme-reload` on a link, or on an element around links, makes them full page loads.

After 80 ms of hover or focus on a link, or at once on touch, the Router fetches its Page and keeps it for 30 seconds, so the click shows it sooner. It doesn't when the browser asks to save data or on a 2G connection. `data-yuanme-prefetch="false"` on a link, or around it, turns prefetching off for it.

### Events

With the Router on, it fires three events on `document`, which a Site-level Script listens for:

- `yuanme:before-swap`, before the swap. The event has `from` and `to`, the two URLs, `navigationType`, which is `"push"`, `"replace"` or `"traverse"`, `newDocument`, the parsed Page, and `swap`, the function that swaps it in. Replace `swap` to change how the Page goes in.
- `yuanme:after-swap`, after the swap and before the new Page's Scripts run.
- `yuanme:page-load`, once the Page's Scripts have run and its Sidecar scripts are mounted: after the first load, and after each swap.

```ts check scripts/site.ts
document.addEventListener("yuanme:before-swap", (event) => {
  const swap = event.swap;
  event.swap = () => {
    document.documentElement.dataset.leaving = event.from.pathname;
    swap();
  };
});
```

### navigate

A Script moves to another Page with `navigate` from `yuanme:router`: through the Router when it is on, or as a full page load. `history: "replace"` replaces the current history entry instead of adding one:

```ts check scripts/site.ts
import { navigate } from "yuanme:router";
import { greet } from "./greet";
import nav from "/data/nav.yaml";

greet(`${nav.length} links`);
document.querySelector("h1")?.addEventListener("click", () => {
  navigate("/blog/", { history: "replace" });
});
```

## The build report

`yuanme build` writes `.yuanme/report.md` and `.yuanme/report.json`. Under `## Scripts`, the report lists every file it wrote for Scripts, with its size and each module in it, with that module's size in the minified code:

```md
`assets/site-bf5be843e220a556.js`, 255 B

| Module | Size |
|---|---:|
| `yuanme:router` | 152 B |
| `data/nav.yaml` | 77 B |
| `scripts/greet.js` | 67 B |
| `scripts/site.ts` | 149 B |
```

`report.json` has the same under `scripts`, as `file`, `bytes` and `modules`. Read it to find what makes a Script large.

## When something is wrong

Code that can't be parsed is `asset/invalid-script`, at the place in the file:

```js fail asset/invalid-script scripts/greet.js
export const x = ;
```

JSX is `asset/jsx`:

```js fail asset/jsx scripts/greet.js
export const greeting = <p>Hello</p>;
```

The bundler's warnings about code that works but is likely a mistake are `asset/script-warning`. A warning doesn't fail the build unless you pass `--strict`:

```js fail asset/script-warning scripts/greet.js
export const greet = (code) => eval(code);
```

The codes for Scripts:

- `asset/not-found`: a `<script src>` or an `import` names no file of the project, no Static file and no installed package.
- `asset/classic-script`: a `<script src>` of a project file without `type="module"`.
- `asset/css-in-script`: a Script imports a stylesheet.
- `asset/jsx`: a Script has JSX in it.
- `asset/invalid-script`: a Script isn't valid JavaScript, TypeScript or UTF-8, or a `<script type="module">` names a file that is no Script.
- `asset/script-warning`: the bundler warns about a Script or a package it imports.
- `asset/dependencies-not-installed`: an import of a package, with a `package.json` and no `node_modules`.
- `asset/type-error`: with `[typescript] check = true`, `tsc` finds a type error.
- `asset/tool-unavailable`: with `[typescript] check = true`, yuanme can't download `tsc`, or `YUANME_TSC_BIN` names no file.

`yuanme docs CODE` explains each one.
