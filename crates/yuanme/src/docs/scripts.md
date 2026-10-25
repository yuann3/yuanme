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

yuanme strips the types from `.ts` and `.mts` files and doesn't check them. It doesn't read `tsconfig.json`, so its `paths` don't apply to imports. yuanme doesn't type-check Scripts yet.

yuanme doesn't compile JSX either. JSX in a Script is `asset/jsx`; build the element with the DOM, or write it in a Template.

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
for (const el of document.querySelectorAll(".clock")) {
  el.textContent = new Date().toLocaleTimeString();
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

You don't load a Sidecar script. Each Page that rendered the Template gets it as a `<script type="module">`, just after the Page's last `<link rel="stylesheet">`, or else just before `</head>`; a Page with neither gets none. A Page that didn't render the Template doesn't load it.

A Sidecar script may export `mount`, and its exports survive bundling, but yuanme doesn't call `mount` yet. Until it does, do the work at the module's top level, as above, which runs on every page load.

## The Router

yuanme doesn't have the Router yet: every move between Pages is a full page load.

A Script can already import `navigate` from `yuanme:router`, a module yuanme provides. For now it loads the URL in full; `history: "replace"` replaces the current history entry instead of adding one:

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

`yuanme docs CODE` explains each one.
