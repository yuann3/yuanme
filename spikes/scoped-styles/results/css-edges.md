### atrules.css

```css
@layer components {
  .card { color: red; }
}
@media (min-width: 600px) { .card .title { color: blue; } }
@container side (min-width: 300px) { .card { color: green; } }
@supports (display: grid) { .card { display: grid; } }
@keyframes spin { from { rotate: 0deg } to { rotate: 360deg } }
.card { animation: spin 1s linear; container-name: side; }
@font-face { font-family: X; src: url(x.woff2); }
@property --p { syntax: "<color>"; inherits: true; initial-value: red; }
```
no targets (nesting kept):
```css
@layer components {
  .card[data-y-H] {
    color: red;
  }
}

@media (width >= 600px) {
  .card[data-y-H] .title[data-y-H] {
    color: #00f;
  }
}

@container side (width >= 300px) {
  .card[data-y-H] {
    color: green;
  }
}

@supports (display: grid) {
  .card[data-y-H] {
    display: grid;
  }
}

@keyframes spin {
  from {
    rotate: 0deg;
  }

  to {
    rotate: 360deg;
  }
}

.card[data-y-H] {
  animation: 1s linear spin;
  container-name: side;
}

@font-face {
  font-family: X;
  src: url("x.woff2");
}

@property --p {
  syntax: "<color>";
  inherits: true;
  initial-value: red;
}
```
- keyframes left global: ["spin"]

### bad_global_bare.css

```css
.a :global .b { color: red; }
```
no targets (nesting kept): **error** `bad_global_bare.css:1:5: Ambiguous CSS module class not supported`

### bad_global_empty.css

```css
.a :global() { color: red; }
```
no targets (nesting kept): **error** `bad_global_empty.css:1:12: Invalid empty selector`

### bad_global_list.css

```css
.a :global(.b, .c) { color: red; }
```
no targets (nesting kept): **error** `bad_global_list.css:1:14: Unexpected token Comma`

### bad_local.css

```css
:local(.a) .b { color: red; }
```
no targets (nesting kept):
```css
.a[data-y-H] .b[data-y-H] {
  color: red;
}
```
- diagnostic `bad_local.css:1:1: `.a .b`: :local() is not supported; sidecar selectors are scoped by default`

### bad_syntax.css

```css
.a { color: red;
.b :global(.c { color: blue; }
```
no targets (nesting kept): **error** `bad_syntax.css:3:1: Unexpected end of input`

### global.css

```css
.body > :global(h1:first-of-type) { display: none; }
article :global(em), article :global(i) { font-synthesis: style; }
:global(.dark) .card { color: white; }
:global(.prose a:hover) { color: red; }
.photo :global(img)::after { content: ""; }
:global(body) { margin: 0; }
.card:global(.is-open) { color: red; }
```
no targets (nesting kept):
```css
.body[data-y-H] > h1:first-of-type {
  display: none;
}

article[data-y-H] em, article[data-y-H] i {
  font-synthesis: style;
}

.dark .card[data-y-H] {
  color: #fff;
}

.prose a:hover {
  color: red;
}

.photo[data-y-H] img:after {
  content: "";
}

body {
  margin: 0;
}

.card.is-open[data-y-H] {
  color: red;
}
```
- diagnostic `global.css:7:1: `.card.is-open`: :global() inside a scoped compound does nothing (scoping is by attribute, not by class); drop it or make it its own compound`

### nesting.css

```css
.card {
  color: red;
  .title { color: blue; }
  &:hover { color: green; }
  & > .body { margin: 0; }
  .dark & { color: white; }
  @media (min-width: 600px) { padding: 1px; .title { padding: 2px; } }
  :global(.prose) { color: gray; }
}
```
no targets (nesting kept):
```css
.card[data-y-H] {
  color: red;

  & .title[data-y-H] {
    color: #00f;
  }

  &:hover {
    color: green;
  }

  & > .body[data-y-H] {
    margin: 0;
  }

  .dark[data-y-H] & {
    color: #fff;
  }

  @media (width >= 600px) {
    padding: 1px;

    & .title[data-y-H] {
      padding: 2px;
    }
  }

  & .prose {
    color: gray;
  }
}
```
Tailwind v4 default targets (nesting lowered):
```css
.card[data-y-H] {
  color: red;
}

.card[data-y-H] .title[data-y-H] {
  color: #00f;
}

.card[data-y-H]:hover {
  color: green;
}

.card[data-y-H] > .body[data-y-H] {
  margin: 0;
}

.dark[data-y-H] .card[data-y-H] {
  color: #fff;
}

@media (width >= 600px) {
  .card[data-y-H] {
    padding: 1px;
  }

  .card[data-y-H] .title[data-y-H] {
    padding: 2px;
  }
}

.card[data-y-H] .prose {
  color: gray;
}
```

### pseudo.css

```css
:hover { color: red; }
* { box-sizing: border-box; }
.a > * + * { margin-top: 1em; }
.a::before, .a:hover::after, ::selection { content: ""; }
.a:is(.b, .c) .d { color: red; }
:is(.a, .b) > :where(.c, .d) { color: red; }
.a:not(.b):not([hidden]) { color: red; }
.a:has(> img, .x) + .b { color: red; }
:where(.a) :not(.b) { color: red; }
li:nth-child(2n + 1 of .x) { color: red; }
a:focus-visible, a[href^="http"]:not(:hover) { color: red; }
:root { --x: 1; }
html, body { margin: 0; }
```
no targets (nesting kept):
```css
:hover[data-y-H] {
  color: red;
}

[data-y-H] {
  box-sizing: border-box;
}

.a[data-y-H] > [data-y-H] + [data-y-H] {
  margin-top: 1em;
}

.a[data-y-H]:before, .a:hover[data-y-H]:after, [data-y-H]::selection {
  content: "";
}

.a:is(.b, .c)[data-y-H] .d[data-y-H] {
  color: red;
}

:is(.a, .b)[data-y-H] > :where(.c, .d)[data-y-H] {
  color: red;
}

.a:not(.b):not([hidden])[data-y-H] {
  color: red;
}

.a:has( > img, .x)[data-y-H] + .b[data-y-H] {
  color: red;
}

:where(.a)[data-y-H] :not(.b)[data-y-H] {
  color: red;
}

li:nth-child(odd of .x)[data-y-H] {
  color: red;
}

a:focus-visible[data-y-H], a[href^="http"]:not(:hover)[data-y-H] {
  color: red;
}

:root {
  --x: 1;
}

html[data-y-H], body[data-y-H] {
  margin: 0;
}
```

