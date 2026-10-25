---
title: The Org subset
description: How an Org file becomes an Entry, which Org yuanme renders, what it rejects or warns about, and where it differs from Emacs.
---

yuanme reads an Org file the way `org-element` in Org 9.8.7 (bundled with Emacs 31.1) reads it. The Org subset is the part of Org it renders. Org outside the subset fails the build at its exact place; yuanme never drops it silently or passes it through. Where yuanme deliberately renders Org differently from Emacs, that is an Org divergence, and every one is listed at the end of this page. Anything else that differs from what Emacs exports is a bug.

## An Org file is an Entry

A file ending in `.org` in a Collection's folder is an Entry, as a Markdown file is. Its fields are the keywords at the top of the file: `#+KEY: value` sets the field `key`, whatever the case of `KEY`.

- `#+TITLE`, `#+DATE`, `#+UPDATED`, `#+SLUG`, `#+DRAFT` and `#+UNLISTED` set the fields every Entry has.
- Each field the Collection declares is a keyword too: a `summary` field is `#+SUMMARY`.
- Each Taxonomy of the Collection is a keyword, such as `#+SERIES`. The `tags` Taxonomy is the exception: its keyword is `#+FILETAGS`, written `:a:b:` as Org writes it.

Here the Collection declares a `summary` field and a `tags` Taxonomy:

```toml check
[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[collections.posts.fields]
summary = { type = "string", optional = true }

[taxonomies.tags]
collections = ["posts"]
route = "/tags/{term}/"
```

```text check content/posts/example.org
#+TITLE: Hello from Org
#+DATE: [2025-12-12 Fri]
#+FILETAGS: :rust:emacs:
#+SUMMARY: The first Post written in Org.
#+DRAFT: false

Org's *bold*, /italic/ and =verbatim=, and a [[https://orgmode.org][link]].
```

How a value is read:

- A string field takes the text after the colon as it is written.
- Any other field takes a YAML value, as front matter does: `true` and `false`, numbers, and lists such as `[a, b]`. Org's `t` and `nil` are `entry/wrong-type` errors.
- A date field takes `2025-12-12` or an Org timestamp, `[2025-12-12 Fri]` or `<2025-12-12 Fri>`. A `datetime` field may give a time, `<2025-12-12 Fri 10:00>`; a time in a `date` field is `entry/invalid-date`. A day name that isn't the date's is the warning `org/wrong-weekday`.

```text fail entry/wrong-type content/posts/example.org
#+TITLE: Hello from Org
#+DRAFT: t
```

Field keywords go in the preamble: before the first headline and before any text. A file-level `:PROPERTIES:` drawer, which may carry an `:ID:`, goes on the very first line, above them. A field keyword further down is `org/misplaced-keyword`:

```text fail org/misplaced-keyword content/posts/example.org
#+TITLE: Hello from Org

Some text.
#+DATE: 2025-12-12
```

Every other keyword is Org's own, and yuanme reads it by what it does:

- Keywords for Emacs alone, such as `#+STARTUP`, `#+TODO`, `#+PROPERTY`, `#+CATEGORY`, `#+LINK` and `#+TBLFM`, are accepted and change nothing. A table shows the values written in it; `#+TBLFM` formulas aren't run.
- Keywords for Org's exporter, such as `#+AUTHOR`, `#+EMAIL`, `#+DESCRIPTION`, `#+SUBTITLE`, `#+LANGUAGE`, `#+TOC` and every `#+HTML_…`, are the warning `org/no-effect`. Its note says what does that job in yuanme: often a field the Collection declares, Site config or a Template.
- `#+OPTIONS` is read option by option; see below.
- `#+LATEX_HEADER` lines that define macros define them for the Entry's math (`yuanme docs math`).
- `#+SETUPFILE` is the error `org/setupfile`: one file is one Entry. `#+INCLUDE` and `#+MACRO` are outside the subset.
- `#+TAGS` lists the tags Emacs offers, so it is an `entry/unknown-key` error that suggests `#+FILETAGS`. ox-hugo's `#+HUGO_…` keywords are errors too, and the help names yuanme's keyword where there is one.
- Any other keyword is `entry/unknown-key`, with the closest keyword as help.

## Markup

- `*bold*` is `<strong>`, `/italic/` is `<em>`, `+strike+` is `<del>`, and `_underline_` is `<span class="underline">`.
- `=verbatim=` and `~code~` are both `<code>`.
- Entities such as `\alpha` and `\to` are their characters. `\\` at the end of a line is a line break.
- `--` is an en dash, `---` an em dash and `...` an ellipsis, as in Org; quotes become curly. `\-` is a soft hyphen.
- `x_{i}` and `x^{2}` are sub- and superscripts, but `snake_case` stays as it is (Org divergence 1).
- Timestamps are `<time class="timestamp">` with a `datetime`, inside `<span class="timestamp-wrapper">`. Statistics cookies such as `[1/3]` are shown as written.
- Math is converted to MathML when the Site is built; `yuanme docs math` covers it.

## Headlines

`*` is `<h2>` and `*****` is `<h6>`, because the Template writes the Page's `<h1>` from the title. A headline deeper than level 5 is outside the subset. Headlines show no TODO keyword, priority or tags, and are never numbered (Org divergence 3).

A heading's id is its `:CUSTOM_ID:` property if it has one, otherwise one made from its title, as a Markdown heading's is. Templates see every heading in `entry.headings`.

Which headlines are shown follows Org's export rules:

- A `COMMENT` headline and a headline tagged `:noexport:`, or a tag in `#+EXCLUDE_TAGS`, are left out with everything under them.
- If any headline is tagged `:export:`, or a tag in `#+SELECT_TAGS`, only those subtrees are shown, without the text before the first headline.
- A headline tagged `:ignore:` is left out, and what is under it is kept.
- An archived headline, tagged `:ARCHIVE:`, is shown without what is under it.

## Lists, tables and blocks

- Unordered and ordered lists, with checkboxes (`[X]`, `[ ]`, `[-]`) and counters such as `[@3]`. A description list, `- term :: details`, is a `<dl>`.
- Tables: the rows before the first rule `|---|` are the head. A row of alignment cookies such as `<r>`, `<l>` or `<c>` sets the columns' alignment and isn't shown.
- `#+begin_quote` is a `<blockquote>`, `#+begin_center` is `<div class="org-center">`, and `#+begin_verse` is `<p class="verse">`, with a `<br>` after each line.
- Any other block, `#+begin_NAME`, is `<div class="NAME">`, such as `#+begin_note`.
- `#+begin_example` and `: ` fixed-width lines are plain code blocks.
- A drawer's contents are shown without the drawer, except a `LOGBOOK`'s. Property drawers, planning lines, clocks, comment lines and `#+begin_comment` blocks are never shown.
- Raw HTML passes through: `#+begin_export html`, `@@html:…@@` and `#+HTML:`. Export blocks and snippets for other back ends, such as `latex`, are dropped, as Emacs's HTML export drops them.

```text check content/posts/example.org
#+TITLE: Hello from Org

* Shopping
- [X] Bread
- [ ] Milk

Where to buy:
- Bakery :: Bread, early.
- Market :: Everything else.

| Item  | Price |
|-------+-------|
| <l>   | <r>   |
| Bread |   2.5 |

#+begin_note
Shops close early on Sundays.
#+end_note

#+begin_quote
Food is the most primitive form of comfort. -- Sheilah Graham
#+end_quote
```

## Source blocks

A `#+begin_src LANG` block is highlighted when the Site is built, as a Markdown code block is. `LANG` is a Shiki language name or alias, an Org name such as `elisp` or `C`, or `jupyter-LANG`. A language no grammar highlights is the warning `entry/unknown-language`; `text` is plain text without one.

yuanme never runs code (Org divergence 2). The block's `:exports` header argument, on its `#+begin_src` line or a `#+HEADER:` line, decides what is shown of the block and of the `#+RESULTS:` already written after it:

- `code`, the default: the block only.
- `results`: the results only.
- `both`: the block, then the results.
- `none`: neither.

```text check content/posts/example.org
#+TITLE: Hello from Org

#+begin_src python :exports both
print(6 * 7)
#+end_src

#+RESULTS:
: 42
```

`-i` keeps a block's indentation. The switches `-n`, `+n`, `-r` and `-l` are the warning `org/no-effect`: lines aren't numbered and labels aren't rewritten.

## Links

- `https:`, `http:`, `mailto:` and `ftp:` links are written as they are. `doi:10.1000/xyz` links to `https://doi.org/10.1000/xyz`.
- `file:` links to another Entry by its path from this file, `.org` or `.md`, and lead to its Page. A search after `::` finds a heading on it: `::*Title` by title, `::#id` by `CUSTOM_ID`, and `::name` by `<<target>>`, `#+NAME` or title. A path from the Site's root, such as `file:/cv.pdf`, is left as it is.
- `[[#id]]`, `[[*Title]]` and `[[name]]` lead to a heading, target or named element in this file. With no description, the link shows the heading's title.
- `[[id:…]]` leads to the Entry or headline whose `:PROPERTIES:` drawer has that `:ID:`, anywhere on the Site. Two with the same `:ID:` are `org/duplicate-id`.
- A link that leads nowhere is `link/unresolved`.
- Markdown Entries link to Org ones the same way: `[intro](post.org#intro)`.

The file-level drawer gives this Entry an `:ID:`:

```text check content/posts/hello.org
:PROPERTIES:
:ID: 0c6a3d2e-hello
:END:
#+TITLE: Hello from Org

* Setup
:PROPERTIES:
:CUSTOM_ID: setup
:END:
Install it first. <<later>>

* Use
Go back to [[#setup]], to [[*Setup][the setup]] or to [[later][the note]].
```

```text check content/posts/other.org
#+TITLE: Another Post

Read [[id:0c6a3d2e-hello][the first Post]], or its [[file:hello.org::*Setup][setup]].
```

A link with no description to an image file (`avif`, `gif`, `jpeg`, `jpg`, `png`, `svg` or `webp`), local or `http(s)`, is an `<img>`, with the file's name as its alt text.

Other link types, such as `elisp:`, `shell:` and `info:`, links to code references, and radio targets are outside the subset.

## Footnotes

`[fn:1]` refers to a footnote, defined at the start of a line as `[fn:1] The note.`, inline as `[fn:name:The note.]`, or anonymously as `[fn::The note.]`. Footnotes are numbered in order and written at the end of the body, as Markdown's are. A reference to a footnote that is never defined is `org/undefined-footnote`.

## Names, captions and attributes

Keywords on the line before an element belong to it:

- `#+NAME: x` gives the element the id `x`, and `[[x]]` links to it. A name is a Label, and shares one namespace with headings and equations (`yuanme docs math`).
- `#+CAPTION:` shows on a table, as its `<caption>`; on an image alone in its paragraph, which becomes a `<figure>` with a `<figcaption>`; and on a source block, which becomes a `<figure>` with the caption first. Several `#+CAPTION:` lines are joined with a space; a short caption, `#+CAPTION[short]:`, is left out.
- `#+ATTR_HTML: :name value` puts attributes on a table, a `#+begin_NAME` block, a captioned source block's `<figure>`, or a paragraph's first link or image. On an image, `:alt` sets the alt text. A `:class` on a block joins the block's own class. A value of `nil` leaves the attribute out. Text that isn't `:name value` is `org/invalid-attribute`.
- A `#+CAPTION` or `#+ATTR_HTML` where it can't show, or with no element after it, is the warning `org/no-effect`.

```text check content/posts/example.org
#+TITLE: Hello from Org

#+CAPTION: A cat on a mat.
#+ATTR_HTML: :width 300 :alt A cat
[[https://example.org/cat.png]]

#+NAME: tab:prices
#+CAPTION: Prices.
#+ATTR_HTML: :class wide
| Item  | Price |
|-------+-------|
| Bread |   2.5 |

See [[tab:prices]].
```

## #+OPTIONS

yuanme reads `#+OPTIONS` one option at a time:

- `^:` is honoured: `^:{}`, the default, makes only `x_{i}` and `x^{i}` scripts; `^:t` makes `x_i` one too; `^:nil` makes none.
- An option set to what yuanme does anyway is accepted, such as `toc:nil`, `num:nil`, `todo:nil`, `tags:nil`, `author:nil`, `date:nil`, `f:t`, `tex:t` or `H:5`.
- An option asking for something yuanme doesn't do, such as `toc:t`, `num:t` or `H:3`, is the warning `org/no-effect`, with a note saying what yuanme does instead.
- An option Org doesn't have is the error `org/unknown-option`.

```text check content/posts/example.org
#+TITLE: Hello from Org
#+OPTIONS: toc:nil num:nil ^:t

A variable_name with a real subscript.
```

## Outside the Org subset

These fail the build with `org/unsupported-construct`, and the help says what to write instead:

- macros, `{{{name}}}` and `#+MACRO`
- `#+INCLUDE`
- code that runs: `#+CALL`, inline `call_name()` and inline `src_lang{…}`
- citations, `[cite:@key]`
- radio targets and links, `<<<target>>>`
- dynamic blocks, `#+BEGIN: … #+END:`
- inline tasks, and headlines deeper than level 5
- `:EXPORT_…:` properties, such as `:EXPORT_FILE_NAME:`: one file is one Entry
- link types yuanme can't follow, and links to code references

```text fail org/unsupported-construct content/posts/example.org
#+TITLE: Hello from Org

Built on {{{date}}}.
```

A paragraph so pathological that parsing it would take too long is `org/out-of-fuel`, and blocks or lists nested thousands deep are `org/too-deep`.

## Org divergences

These are every place yuanme departs from Org 9.8.7 on purpose. The list is closed: one is added only by a written decision.

1. **`^:{}` is the default.** `snake_case` stays literal, and only `x_{i}` and `x^{i}` are sub- and superscripts. `#+OPTIONS: ^:t` restores Org's default.
2. **yuanme never evaluates code.** `:exports` decides which of a source block and the `#+RESULTS:` already in the file are shown. `#+CALL`, `call_x()` and inline `src_x{}` are errors, because they exist to insert evaluated results.
3. **Headlines carry no TODO keyword, priority or tags in the output, and are never numbered.** `*` is `<h2>`, as in Emacs's HTML export, down to `<h6>` at level 5. `H:` and `num:` have no effect, and options that ask for these features are warnings.
4. **Org outside the Org subset is a build error.** Emacs would export it.
5. **A field keyword given twice is an error,** `entry/duplicate-key`. Org joins two `#+TITLE`s into one.
6. **A pathological paragraph fails the build,** with `org/out-of-fuel` at the paragraph, never a plain-text fallback.
