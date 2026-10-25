---
title: Markdown
description: The Markdown yuanme reads: CommonMark with GFM, heading ids and Labels, links between Entries, raw HTML, code blocks and ::: blocks.
---

An Entry in a `.md` file is CommonMark with GitHub Flavored Markdown's extensions, and yuanme writes the HTML Astro's default Markdown pipeline writes, so a Post moved from Astro renders the same.

## Front matter

The file starts with YAML front matter between `---` lines, which sets the Entry's fields. A file without it has no fields, so it is missing its `title`. `yuanme docs collections` covers the fields and the codes for mistakes in them.

## What yuanme reads

CommonMark, plus these extensions:

- **Tables**, with `:` in the rule row to align a column.
- **Task lists**: `- [ ]` and `- [x]` items become disabled checkboxes.
- **Strikethrough**: `~~gone~~`, or `~gone~`, becomes `<del>`.
- **Autolinks**: `https://…`, `www.…` and email addresses become links without `<…>` around them.
- **Footnotes**: `[^name]` in the text and `[^name]: …` anywhere in the file. They are numbered in the order they are first referenced, and listed in a section at the end of the body.
- **Heading attributes**: `{#id .class key=value}` at the end of a heading. See [Heading ids and Labels](#heading-ids-and-labels).
- **Block directives**: `:::name` blocks. See [Block directives](#block-directives).
- **Math**: `$…$`, `$$…$$` and ```` ```math ```` blocks. `yuanme docs math` covers them.

```md check content/posts/features.md
---
title: Features
---

| Command | What it does |
| :------ | -----------: |
| `check` | Reports Diagnostics |
| `build` | Writes the Site |

- [x] Write the Post
- [ ] ~~Tweet it~~

Read more at www.commonmark.org.[^spec]

[^spec]: The CommonMark spec, with GitHub's additions.
```

Other extensions are off, so their syntax stays text: GitHub's alerts (`> [!NOTE]`) are a plain blockquote, and `^x^` is not superscript. Wiki links, emoji shortcodes and description lists aren't read either.

## Heading ids and Labels

Every heading gets an id, which `#fragment` links and the Template's `entry.headings` use. yuanme makes it from the heading's text as Astro does, with github-slugger: lowercase, punctuation and emoji dropped, each space a `-`. A second heading with the same text gets `-1`, a third `-2`.

- `## Getting Started` → `getting-started`
- `## Rust's 2024 Edition!` → `rusts-2024-edition`
- `## Ruskey 🦀` → `ruskey`

To choose the id, end the heading with `{#name}`. That name is a Label: it is the id exactly as written, and a link or a cross-reference can rely on it not changing when the heading's text does. Classes and other attributes go in the same braces:

```md check content/posts/setup.md
---
title: Setup
---

## Install it {#install .wide}

## Run it

Go back to [the install step](#install).
```

This writes `<h2 id="install" class="wide">Install it</h2>` and `<h2 id="run-it">Run it</h2>`. Every Label in an Entry, on headings, equations and other elements, shares one namespace, and a generated id never takes a Label's name. Two Labels with one name are `math/duplicate-label`:

```md fail math/duplicate-label content/posts/setup.md
---
title: Setup
---

## Install it {#install}

## Install it again {#install}
```

A Label is letters, digits, `:`, `_`, `.` and `-`. Braces holding anything else, such as `{#my label}`, aren't read as attributes: they stay in the heading's text.

## Smart punctuation

Prose gets curly quotes and typographic marks, as remark-smartypants makes them:

- `"quoted"` → “quoted”, `'single'` → ‘single’, and `it's` → it’s
- ` ``this'' ` → “this”
- `--` → an em dash, —. `---` stays as it is.
- `...` → …

Code, code blocks, raw HTML, math and image alt text are left as written.

## Links

Link to another Entry by its file's path from the Entry you are writing, and yuanme writes the URL of its Page. A `#fragment` after the path leads to a heading in it, by its id:

```md check content/pages/about.md
---
title: About
---

## Contact

Write to me.
```

```md check content/posts/links.md
---
title: Links
---

See the [About page](../pages/about.md), how to [reach me](../pages/about.md#contact),
and [the setup Post](setup.md#install).
```

These become `/about/`, `/about/#contact` and `/blog/setup/#install`, and change with the Entries' Routes. A path that is no Entry's file, a fragment that is no heading's id, or a link to a Draft that this build leaves out is an error, `link/unresolved`:

```md fail link/unresolved content/posts/links.md
---
title: Links
---

See [my CV](cv.md).
```

A URL with a scheme, such as `https://commonmark.org` or `mailto:`, and a path from the Site's root, such as `/blog/`, are written as they are. Once the Site is built, every link on its own origin is checked against the files it wrote: a link to no file is `link/broken`, and one a host would redirect, such as `/about` for `/about/`, is `link/redirected`. Both are warnings.

Images are written `![alt](path)`; `yuanme docs images` covers them.

## Raw HTML

HTML in the body, as a block or inline, reaches the Page unchanged. There is no setting to turn it off: use it for embeds, `<details>`, `<kbd>` and anything else Markdown has no syntax for.

```md check content/posts/raw.md
---
title: Raw HTML
---

Press <kbd>Ctrl</kbd>+<kbd>C</kbd>.

<details>
<summary>The long answer</summary>

Markdown works again after a blank line.

</details>
```

As CommonMark says, an HTML block runs until a blank line, and Markdown inside it before that line is passed through as HTML, not read. Smart punctuation never touches raw HTML. yuanme doesn't yet warn when raw HTML leaves an element open or closes one it didn't open, so check that your tags balance.

## Code blocks

A fenced code block's language is the first word after the fence. yuanme highlights it at build time with the same TextMate grammars as Shiki, in Catppuccin Latte and Mocha for light and dark, and links a stylesheet for them only on Pages that show code. No script runs in the browser.

````md check content/posts/code.md
---
title: Code
---

```rust
fn main() {
    println!("hello");
}
```

```sh
yuanme check --json
```
````

Names are Shiki's, with its aliases, such as `rust`, `ts`, `sh`, `toml` and `jinja`; then Org's names for Emacs modes, such as `C++` and `elisp`; then the name in lowercase. `text`, `txt`, `plain`, `plaintext` and `none`, a block without a language and an indented block are plain text. A name no grammar has is the warning `entry/unknown-language`, with a close name as a fix when there is one, and the block is written as plain text:

````md fail entry/unknown-language content/posts/code.md
---
title: Code
---

```rustlang
fn main() {}
```
````

Each block is a `<pre class="y-code" data-language="NAME">`, which your stylesheets can style.

## Block directives

A block directive wraps Markdown in a `<div>` whose class is the directive's name:

```md check content/posts/tip.md
---
title: Tip
---

:::tip
Markdown inside, with **emphasis**.
:::
```

This writes `<div class="tip">` around the paragraph. The name is letters, digits, `-` and `_`, right after the colons. A closing `:::` must be at least as long as its opening fence, so an outer block takes more colons than the blocks inside it:

```md check content/posts/tip.md
---
title: Tip
---

::::aside
:::warning
Inside two blocks.
:::
::::
```

After the name, `{…}` gives the block a Label as its id, classes and other attributes, as on a heading: `:::aside{#aside:setup .wide data-x=1}`. A value with spaces goes in quotes.

A directive named after a Theorem kind, such as `:::theorem`, `:::lemma` or `:::proof`, is a Theorem block, numbered and with its kind's name in front. It takes a title in brackets, `:::theorem[Fermat's little theorem]{#thm:fermat}`, and `yuanme docs math` covers it. A title on any other directive, `:::tip[Title]`, has no effect and is the warning `markdown/no-effect`, as is anything else on the line after the name, the title and `{…}`.

A `:::` with no name opens a block that swallows the rest of the file, so it is an error, `markdown/empty-directive`. The usual cause is a closing fence shorter than the one it closes, or a Pandoc `::: {.name}` div:

```md fail markdown/empty-directive content/posts/tip.md
---
title: Tip
---

::: {.tip}
Pandoc's syntax.
:::
```

## Math

`$…$` is inline math and `$$…$$` or a ```` ```math ```` block is display math, rendered to MathML at build time. So in prose, write a price or a shell variable such as `$HOME` in backticks. `yuanme docs math` covers math, equation Labels and cross-references.
