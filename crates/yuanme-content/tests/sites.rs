//! Whole Sites read from fixture projects: what resolves, and every way an
//! Entry or Data file can be wrong.

use std::collections::BTreeSet;
use std::fmt::Write;

use yuanme_content::config;
use yuanme_content::document::{Document, Kind};
use yuanme_content::fields::{Image, Value};
use yuanme_content::site::{self, Project, RouteKind, Site};
use yuanme_diag::{Diagnostic, Files, codes};

/// A project written as `=== path` lines, each followed by the file's text.
/// A path ending in `/` is an empty folder.
fn project(fixture: &str) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = Vec::new();
    for line in fixture.split_inclusive('\n') {
        match line.strip_prefix("=== ") {
            Some(path) => files.push((path.trim_end().to_string(), String::new())),
            None => files
                .last_mut()
                .expect("a fixture starts with `=== path`")
                .1
                .push_str(line),
        }
    }
    files
}

fn resolve(fixture: &str, drafts: bool) -> (Files, Option<Site>, Vec<Diagnostic>) {
    let written = project(fixture);
    let mut files = Files::default();
    let config_text = &written
        .iter()
        .find(|(p, _)| p == "yuanme.toml")
        .expect("a yuanme.toml")
        .1;
    let config_file = files.add("yuanme.toml", config_text.as_str());
    let (config, diagnostics) = config::parse(&files, config_file, "1.0.0");
    let config =
        config.unwrap_or_else(|| panic!("the fixture's config is wrong: {diagnostics:#?}"));

    let paths: Vec<&str> = written
        .iter()
        .map(|(p, _)| p.as_str())
        .filter(|p| !p.ends_with('/'))
        .collect();
    let (entries, data) = site::sources(&config, paths.iter().copied());
    let mut id = |path: &str| {
        let text = &written.iter().find(|(p, _)| p == path).expect("listed").1;
        files.add(path, text.as_str())
    };
    let entries: Vec<_> = entries.into_iter().map(|(c, p)| (c, id(p))).collect();
    let data: Vec<_> = data.into_iter().map(&mut id).collect();

    let is_file = |p: &str| paths.contains(&p);
    let is_dir = |d: &str| {
        written
            .iter()
            .any(|(p, _)| p.starts_with(d) && p[d.len()..].starts_with('/'))
    };
    let (site, diagnostics) = {
        let project = Project {
            files: &files,
            entries,
            data,
            is_file: &is_file,
            is_dir: &is_dir,
        };
        site::resolve(&config, &project, drafts)
    };
    (files, site, diagnostics)
}

const BLOG: &str = r#"=== yuanme.toml
[site]
url = "https://example.com"
timezone = "Asia/Singapore"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"
paginate = 2
sort = ["-date"]

[collections.posts.fields]
cover = { type = "image", optional = true }
published = { type = "datetime", optional = true }
kind = { type = "enum", values = ["essay", "note"], default = "essay" }

[taxonomies.tags]
collections = ["posts"]
route = "/tags/{term}/"
slugs = { "C#" = "c-sharp" }

[taxonomies.series]
collections = ["posts"]
route = "/series/{term}/"
single = true

[data.nav]
type = "list"
items = { type = "object", fields = { name = { type = "string" }, href = { type = "string" } } }

[routes.home]
path = "/"
template = "home.html"

[routes.not_found]
path = "/404.html"
template = "404.html"

[feeds.rss]
collections = ["posts", "pages"]
path = "/rss.xml"
=== content/posts/hello-world.md
---
title: Hello, World
date: 2025-03-01
published: 2025-03-01T09:30:00
cover: ./cover.png
tags: [Rust, C++]
series: Intro
---
Hi. Read [the second](second/index.md#how-it-ends), [this](#hello),
[this again](hello-world.md#hello), [the top](#),
[About](../pages/about.md) and [the site](https://example.com/x).

## Hello
=== content/posts/cover.png
=== content/posts/second/index.md
---
title: Second
date: 2025-04-01
tags: [rust, "C#"]
series: Intro
---
## How it ends
=== content/posts/undated.md
---
title: Undated
---
=== content/posts/custom.md
---
title: Custom
date: 2025-02-01
slug: 2024/custom-name
kind: note
---
## Taken {#one}

## One

See [the second one](#one-1).
=== content/posts/wip.md
---
title: Work in progress
date: 2025-05-01
draft: true
---
=== content/posts/secret.md
---
title: Secret
date: 2025-06-01
unlisted: true
tags: [Rust, Hush]
---
=== content/posts/notes.txt
not content
=== content/pages/about.md
---
title: About
---
=== data/nav.yaml
- name: Blog
  href: /blog/
=== data/social.json
{"github": "yuann3", "followers": 12}
"#;

fn show(v: &Value) -> String {
    match v {
        Value::String(s) | Value::Url(s) => format!("{s:?}"),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Date(d) => d.to_string(),
        Value::Datetime(z) => z.to_string(),
        Value::Image(Image::Local(p)) => format!("local {p}"),
        Value::Image(Image::Remote(u)) => format!("remote {u}"),
        Value::List(items) => format!(
            "[{}]",
            items.iter().map(show).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, v)| format!("{k}: {}", show(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// The Site as a reader checks it: Routes, then what each one lists.
fn summary(site: &Site) -> String {
    let slugs = |ids: &[usize]| {
        ids.iter()
            .map(|&i| site.entries[i].slug.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut out = String::from("routes:\n");
    for r in &site.routes {
        let what = match &r.kind {
            RouteKind::Entry(i) => format!("entry {}", site.entries[*i].path),
            RouteKind::List {
                collection,
                page,
                entries,
            } => format!("{collection} page {page}: {}", slugs(entries)),
            RouteKind::Term {
                taxonomy,
                term,
                page,
                entries,
            } => {
                format!(
                    "{taxonomy} `{}` page {page}: {}",
                    site.taxonomies[taxonomy][*term].name,
                    slugs(entries)
                )
            }
            RouteKind::Terms { taxonomy } => format!("{taxonomy} index"),
            RouteKind::Template(name) => format!("template route {name}"),
            RouteKind::Feed { name, entries } => format!("feed {name}: {}", slugs(entries)),
        };
        writeln!(out, "  {} -> {}  {what}", r.url, r.output).unwrap();
    }
    out.push_str("entries:\n");
    for e in &site.entries {
        let date = e.date.map_or("no date".into(), |d| d.to_string());
        let flags = [(e.draft, " draft"), (e.unlisted, " unlisted")]
            .iter()
            .filter(|f| f.0)
            .map(|f| f.1)
            .collect::<String>();
        writeln!(
            out,
            "  {}/{} {:?} {date}{flags}, {} words",
            e.collection, e.slug, e.title, e.word_count
        )
        .unwrap();
        for (k, v) in &e.fields {
            writeln!(out, "    {k} = {}", show(v)).unwrap();
        }
        for (taxonomy, terms) in &e.terms {
            let terms: Vec<_> = terms.iter().map(|(t, _)| t.as_str()).collect();
            writeln!(out, "    {taxonomy}: {}", terms.join(", ")).unwrap();
        }
    }
    for (i, links) in site.links.iter().enumerate() {
        for href in links.values() {
            writeln!(out, "  {} links to {href}", site.entries[i].slug).unwrap();
        }
    }
    out.push_str("lists:\n");
    for (name, ids) in &site.collections {
        writeln!(out, "  {name}: {}", slugs(ids)).unwrap();
    }
    out.push_str("data:\n");
    for (name, v) in &site.data {
        writeln!(out, "  {name} = {}", show(v)).unwrap();
    }
    out
}

#[test]
fn a_blog_resolves_into_routes_lists_and_terms() {
    let (_, site, diagnostics) = resolve(BLOG, false);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    insta::assert_snapshot!(summary(&site.expect("no errors")));
}

/// An Org Entry's keywords are its fields, and links between Org and
/// Markdown Entries reach headings by title, `CUSTOM_ID` and `:ID:` (#16).
#[test]
fn org_keywords_fill_fields_and_links_cross_source_formats() {
    let files = r#"=== content/posts/org.org
:PROPERTIES:
:ID: the-org-post
:END:
#+TITLE: Org -- a post
#+date: [2025-12-12 Fri]
#+FILETAGS: :Rust:wasm:
#+SERIES: Intro
#+N: 3
#+KIND: note
#+AT: <2025-12-12 Fri 10:00>
#+DRAFT: false
#+STARTUP: overview

* Intro
See [[file:md.md::*Second part][the second part]], [[id:head-2][a heading]],
[[file:other.org::#cid]], [[file:other.org::Heading two]] and [[file:md.md]].
=== content/posts/md.md
---
title: Md
---
## Second part

Back to [the Org post](org.org#intro) and [its heading](other.org#cid).
=== content/posts/other.org
#+TITLE: Other
* Heading two
:PROPERTIES:
:ID: head-2
:CUSTOM_ID: cid
:END:
See [[id:the-org-post][the Org post]].
"#;
    let (_, site, diagnostics) = resolve(&fixture(files), false);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let site = site.expect("no errors");
    let mut out = summary(&site);
    for e in &site.entries {
        for h in e.document.headings() {
            writeln!(out, "  {} has h{} #{} {:?}", e.slug, h.level, h.id, h.text).unwrap();
        }
    }
    insta::assert_snapshot!(out);
}

#[test]
fn org_keywords_for_emacs_alone_warn_and_the_entry_still_builds() {
    let files = "=== content/posts/a.org\n#+TITLE: A\n#+AUTHOR: Yuan\n#+DATE: [2025-12-12 Mon]\n#+OPTIONS: toc:nil num:t\n";
    let (sources, site, diagnostics) = resolve(&fixture(files), false);
    assert!(site.is_some(), "{diagnostics:#?}");
    let got: Vec<_> = diagnostics
        .iter()
        .map(|d| {
            let at = d.primary();
            (
                d.code.name,
                d.level,
                &sources.text(at.file)[at.range.clone()],
            )
        })
        .collect();
    use yuanme_diag::Level::Warning;
    assert_eq!(
        got,
        [
            ("org/no-effect", Warning, "AUTHOR"),
            ("org/wrong-weekday", Warning, "Mon"),
            ("org/no-effect", Warning, "num:t"),
        ]
    );
}

/// Lists nested this deep stop the parse; how they stop depends on the
/// build's frame sizes, so either code is right.
#[test]
fn org_nested_too_deeply_fails_at_its_opener() {
    let list: String = (0..200)
        .map(|i| format!("{}- a\n", " ".repeat(i)))
        .collect();
    let files = format!("=== content/posts/a.org\n#+TITLE: A\n\n{list}");
    let (sources, site, diagnostics) = resolve(&fixture(&files), false);
    assert!(site.is_none());
    let [d] = diagnostics.as_slice() else {
        panic!("{diagnostics:#?}")
    };
    assert!(["org/too-deep", "org/out-of-fuel"].contains(&d.code.name));
    let at = d.primary();
    assert!(sources.text(at.file)[at.range.clone()].starts_with("- a\n - a\n"));
}

#[test]
fn drafts_are_listed_only_when_shown() {
    let (_, site, _) = resolve(BLOG, true);
    let site = site.expect("no errors");
    let wip = site
        .entries
        .iter()
        .position(|e| e.slug == "wip")
        .expect("a Draft is an Entry when shown");
    // Newest first: the Draft is dated after every listed post.
    assert_eq!(site.collections["posts"].first(), Some(&wip));
    assert!(site.routes.iter().any(|r| r.url == "/blog/wip/"));
}

/// Config for the cases below, which add their own files.
const BASE: &str = r#"=== yuanme.toml
[site]
url = "https://e.com"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[collections.posts.fields]
n = { type = "int", min = 1, max = 5, optional = true }
kind = { type = "enum", values = ["essay", "note"], optional = true }
cover = { type = "image", optional = true }
at = { type = "datetime", optional = true }

[taxonomies.tags]
collections = ["posts"]
route = "/tags/{term}/"

[taxonomies.series]
collections = ["posts"]
route = "/series/{term}/"
single = true
=== content/posts/
"#;

const ONE_POST: &str = "=== content/posts/a.md\n";

/// `$s` 256 times, for the input that runs a parse out of fuel: a quadratic
/// one needs a few hundred lines.
macro_rules! x256 {
    ($s:literal) => {
        x4!(x4!(x4!(x4!($s))))
    };
}
macro_rules! x4 {
    ($s:expr) => {
        concat!($s, $s, $s, $s)
    };
}

/// (code, files, the file and text its primary span must cover). Files
/// without a `yuanme.toml` are added to `BASE`, and text without a
/// `=== path` line is `content/posts/a.md`.
const CASES: &[(&str, &str, &str, &str)] = &[
    (
        "entry/syntax",
        "---\ntitle: [a\n---\n",
        "content/posts/a.md",
        "",
    ),
    (
        "entry/syntax",
        "---\ntitle: A\n",
        "content/posts/a.md",
        "---",
    ),
    (
        "entry/syntax",
        "---\ntitle: !!int 3\n---\n",
        "content/posts/a.md",
        "3",
    ),
    (
        "entry/toml-front-matter",
        "+++\ntitle = \"A\"\n+++\n",
        "content/posts/a.md",
        "+++",
    ),
    (
        "entry/duplicate-key",
        "---\ntitle: A\ndate: 2025-01-01\ntitle: B\n---\n",
        "content/posts/a.md",
        "title",
    ),
    (
        "entry/unknown-key",
        "---\ntitle: A\nknd: note\n---\n",
        "content/posts/a.md",
        "knd",
    ),
    // A Taxonomy's field is unknown in a Collection it doesn't cover.
    (
        "entry/unknown-key",
        "=== content/pages/a.md\n---\ntitle: A\ntags: [x]\n---\n",
        "content/pages/a.md",
        "tags",
    ),
    (
        "entry/missing-field",
        "---\ndate: 2025-01-01\n---\nBody\n",
        "content/posts/a.md",
        "---\ndate: 2025-01-01\n---",
    ),
    (
        "entry/missing-field",
        "Body without front matter\n",
        "content/posts/a.md",
        "",
    ),
    (
        "entry/wrong-type",
        "---\ntitle: 12\n---\n",
        "content/posts/a.md",
        "12",
    ),
    (
        "entry/wrong-type",
        "---\ntitle: A\ndraft: yes\n---\n",
        "content/posts/a.md",
        "yes",
    ),
    (
        "entry/wrong-type",
        "---\ntitle: A\nn: 2.5\n---\n",
        "content/posts/a.md",
        "2.5",
    ),
    (
        "entry/wrong-type",
        "---\n- title\n---\n",
        "content/posts/a.md",
        "- title\n",
    ),
    (
        "entry/wrong-type",
        "---\ntitle: A\ntags: Rust\n---\n",
        "content/posts/a.md",
        "Rust",
    ),
    (
        "entry/wrong-type",
        "---\ntitle: A\nseries: [a, b]\n---\n",
        "content/posts/a.md",
        "[a, b]",
    ),
    (
        "entry/out-of-range",
        "---\ntitle: A\nn: 9\n---\n",
        "content/posts/a.md",
        "9",
    ),
    (
        "entry/invalid-value",
        "---\ntitle: A\nkind: eassy\n---\n",
        "content/posts/a.md",
        "eassy",
    ),
    (
        "image/not-found",
        "---\ntitle: A\ncover: ./missing.png\n---\n",
        "content/posts/a.md",
        "./missing.png",
    ),
    (
        "entry/invalid-value",
        "---\ntitle: A\ntags: [ok, \"!!!\"]\n---\n",
        "content/posts/a.md",
        "\"!!!\"",
    ),
    (
        "entry/invalid-date",
        "---\ntitle: A\ndate: March 1 2025\n---\n",
        "content/posts/a.md",
        "March 1 2025",
    ),
    (
        "entry/invalid-date",
        "---\ntitle: A\ndate: 2025-02-30\n---\n",
        "content/posts/a.md",
        "2025-02-30",
    ),
    (
        "entry/invalid-date",
        "---\ntitle: A\nat: 2025-01-01\n---\n",
        "content/posts/a.md",
        "2025-01-01",
    ),
    (
        "entry/invalid-slug",
        "---\ntitle: A\nslug: My Post\n---\n",
        "content/posts/a.md",
        "My Post",
    ),
    (
        "entry/duplicate-slug",
        "=== content/posts/a.md\n---\ntitle: A\n---\n=== content/posts/b.md\n---\ntitle: B\nslug: a\n---\n",
        "content/posts/b.md",
        "a",
    ),
    // Drafts take their Slug even when they aren't shown.
    (
        "entry/duplicate-slug",
        "=== content/posts/a.md\n---\ntitle: A\ndraft: true\n---\n=== content/posts/b.md\n---\ntitle: B\nslug: a\n---\n",
        "content/posts/b.md",
        "a",
    ),
    (
        "entry/term-collision",
        "=== yuanme.toml\n[site]\nurl = \"https://e.com\"\n[collections.posts]\nroute = \"/blog/{slug}/\"\n[taxonomies.tags]\ncollections = [\"posts\"]\nroute = \"/tags/{term}/\"\nslugs = { \"C#\" = \"rust\" }\n=== content/posts/a.md\n---\ntitle: A\ntags: [C#, Rust]\n---\n",
        "content/posts/a.md",
        "Rust",
    ),
    (
        "entry/output-collision",
        "=== content/posts/x.md\n---\ntitle: X\n---\n=== content/pages/blog.md\n---\ntitle: Blog\n---\n",
        "yuanme.toml",
        "posts",
    ),
    // A Route a Template makes alone collides like any other.
    (
        "entry/output-collision",
        "=== yuanme.toml\n[site]\nurl = \"https://e.com\"\n[routes.about]\npath = \"/about/\"\ntemplate = \"about.html\"\n=== content/pages/about.md\n---\ntitle: About\n---\n",
        "yuanme.toml",
        "\"about.html\"",
    ),
    (
        "entry/duplicate-data-file",
        "=== data/nav.yaml\na: 1\n=== data/nav.json\n{\"a\": 1}\n",
        "data/nav.json",
        "",
    ),
    (
        "entry/syntax",
        "=== data/nav.toml\na = \n",
        "data/nav.toml",
        "",
    ),
    (
        "entry/wrong-type",
        "=== data/nav.yaml\na:\n",
        "data/nav.yaml",
        "",
    ),
    (
        "entry/wrong-type",
        "=== yuanme.toml\n[site]\nurl = \"https://e.com\"\n[data.nav]\ntype = \"list\"\nitems = { type = \"string\" }\n=== data/nav.toml\na = 1\n",
        "data/nav.toml",
        "a = 1\n",
    ),
    (
        "org/unsupported-construct",
        "=== content/posts/a.org\n#+TITLE: A\n\nBuilt {{{date}}}.\n",
        "content/posts/a.org",
        "{{{date}}}",
    ),
    (
        "org/setupfile",
        "=== content/posts/a.org\n#+TITLE: A\n#+SETUPFILE: setup.org\n",
        "content/posts/a.org",
        "#+SETUPFILE: setup.org",
    ),
    // Upstream organic's fuel runs out on a few hundred empty drawers
    // (#49), and that is an error at the first, not a hang.
    (
        "org/out-of-fuel",
        concat!("=== content/posts/a.org\n#+TITLE: A\n\n", x256!(":X:\n\n")),
        "content/posts/a.org",
        ":X:",
    ),
    (
        "org/undefined-footnote",
        "=== content/posts/a.org\n#+TITLE: A\n\nA note[fn:1].\n",
        "content/posts/a.org",
        "[fn:1]",
    ),
    (
        "org/invalid-attribute",
        "=== content/posts/a.org\n#+TITLE: A\n\n#+ATTR_HTML: class=x\n[[file:a.png]]\n",
        "content/posts/a.org",
        "class=x",
    ),
    (
        "org/duplicate-id",
        "=== content/posts/a.org\n:PROPERTIES:\n:ID: 1\n:END:\n#+TITLE: A\n=== content/posts/b.org\n#+TITLE: B\n* B\n:PROPERTIES:\n:ID: 1\n:END:\n",
        "content/posts/b.org",
        ":ID: 1",
    ),
    (
        "org/misplaced-keyword",
        "=== content/posts/a.org\n#+TITLE: A\n\nText.\n#+DATE: 2025-01-01\n",
        "content/posts/a.org",
        "DATE",
    ),
    (
        "org/unknown-option",
        "=== content/posts/a.org\n#+TITLE: A\n#+OPTIONS: tco:nil\n",
        "content/posts/a.org",
        "tco:nil",
    ),
    // Org would join them (Org divergence 5).
    (
        "entry/duplicate-key",
        "=== content/posts/a.org\n#+TITLE: A\n#+title: B\n",
        "content/posts/a.org",
        "title",
    ),
    (
        "entry/unknown-key",
        "=== content/posts/a.org\n#+TITLE: A\n#+TITEL: B\n",
        "content/posts/a.org",
        "TITEL",
    ),
    (
        "entry/unknown-key",
        "=== content/posts/a.org\n#+TITLE: A\n#+TAGS: a b\n",
        "content/posts/a.org",
        "TAGS",
    ),
    (
        "entry/unknown-key",
        "=== content/posts/a.org\n#+TITLE: A\n#+HUGO_TAGS: a\n",
        "content/posts/a.org",
        "HUGO_TAGS",
    ),
    (
        "entry/wrong-type",
        "=== content/posts/a.org\n#+TITLE: A\n#+DRAFT: t\n",
        "content/posts/a.org",
        "t",
    ),
    (
        "entry/invalid-date",
        "=== content/posts/a.org\n#+TITLE: A\n#+DATE: <2025-12-12 Fri 10:00>\n",
        "content/posts/a.org",
        "<2025-12-12 Fri 10:00>",
    ),
    (
        "entry/missing-field",
        "=== content/posts/a.org\nText.\n",
        "content/posts/a.org",
        "",
    ),
    (
        "link/unresolved",
        "=== content/posts/a.org\n#+TITLE: A\n\nSee [[file:b.org::*Nope]].\n=== content/posts/b.org\n#+TITLE: B\n* Yes\n",
        "content/posts/a.org",
        "[[file:b.org::*Nope]]",
    ),
    (
        "link/unresolved",
        "=== content/posts/a.org\n#+TITLE: A\n\nSee [[id:nope]] and [[*Yes]].\n* Yes\n",
        "content/posts/a.org",
        "[[id:nope]]",
    ),
    (
        "link/unresolved",
        "=== content/posts/a.org\n#+TITLE: A\n\nSee [[nowhere]].\n",
        "content/posts/a.org",
        "[[nowhere]]",
    ),
    (
        "markdown/empty-directive",
        "---\ntitle: A\n---\n:::\nx\n",
        "content/posts/a.md",
        ":::",
    ),
    // Pandoc's fenced div, which yuanme doesn't read (#17).
    (
        "markdown/empty-directive",
        "---\ntitle: A\n---\n::: {#thm-a}\nx\n:::\n",
        "content/posts/a.md",
        ":::",
    ),
    // Block directives nested and never closed: each line is matched
    // against every one open around it.
    (
        "markdown/out-of-fuel",
        concat!("---\ntitle: A\n---\nProse.\n\n", x256!(":::a\n")),
        "content/posts/a.md",
        ":::a",
    ),
    (
        "link/unresolved",
        "---\ntitle: A\n---\nSee [b](b.md).\n",
        "content/posts/a.md",
        "[b](b.md)",
    ),
    // `..` and `%20` resolve as a path does, and the file is still missing.
    (
        "link/unresolved",
        "---\ntitle: A\n---\nSee [b](../posts/my%20b.md).\n",
        "content/posts/a.md",
        "[b](../posts/my%20b.md)",
    ),
    (
        "link/unresolved",
        "=== content/posts/a.md\n---\ntitle: A\n---\nSee [b](b.md#nope).\n=== content/posts/b.md\n---\ntitle: B\n---\n## Yes\n",
        "content/posts/a.md",
        "[b](b.md#nope)",
    ),
    (
        "link/unresolved",
        "---\ntitle: A\n---\n## One\nSee [one](#two).\n",
        "content/posts/a.md",
        "[one](#two)",
    ),
    // A Draft has no Route in a build.
    (
        "link/unresolved",
        "=== content/posts/a.md\n---\ntitle: A\n---\nSee [b](b.md).\n=== content/posts/b.md\n---\ntitle: B\ndraft: true\n---\n",
        "content/posts/a.md",
        "[b](b.md)",
    ),
    // Math: LaTeX yuanme can't render, at the place in the file math-core
    // found it, in a Markdown and an Org Entry.
    (
        "math/unsupported",
        "---\ntitle: A\n---\nSo:\n\n$$\n\\begin{eqnarray} a &=& b \\end{eqnarray}\n$$\n",
        "content/posts/a.md",
        "{eqnarray}",
    ),
    (
        "math/unsupported",
        "=== content/posts/a.org\n#+TITLE: A\n\nSo \\(\\DeclareMathOperator{\\tr}{tr} \\tr A\\).\n",
        "content/posts/a.org",
        "\\DeclareMathOperator",
    ),
    (
        "math/unsupported",
        "=== content/posts/a.org\n#+TITLE: A\n\n\\begin{eqnarray}\na &=& b\n\\end{eqnarray}\n",
        "content/posts/a.org",
        "{eqnarray}",
    ),
    (
        "math/unsupported",
        "---\ntitle: A\n---\n$$ E = mc^2 \\label{eq:e} $$\n",
        "content/posts/a.md",
        "\\label",
    ),
    // An `\eqref` to a Label no equation has, an Org `\ref` too, and one
    // to an equation without a number.
    (
        "math/unknown-label",
        "---\ntitle: A\n---\n$$\\begin{equation} E \\label{eq:gauss} \\end{equation}$$\n\nBy $\\eqref{eq:gaus}$.\n",
        "content/posts/a.md",
        "eq:gaus",
    ),
    (
        "math/unknown-label",
        "=== content/posts/a.org\n#+TITLE: A\n\nBy \\ref{eq:none}.\n",
        "content/posts/a.org",
        "eq:none",
    ),
    (
        "math/unknown-label",
        "---\ntitle: A\n---\n$$\\begin{equation*} E \\label{eq:e} \\end{equation*}$$\n\nBy $\\eqref{eq:e}$.\n",
        "content/posts/a.md",
        "eq:e",
    ),
    // One namespace: a heading's `{#id}` and an equation's `\label`, two
    // `{#id}`s, and Org's `#+NAME` and `<<target>>`.
    (
        "math/duplicate-label",
        "---\ntitle: A\n---\n## Energy {#energy}\n\n$$\\begin{equation} E \\label{energy} \\end{equation}$$\n",
        "content/posts/a.md",
        "energy",
    ),
    (
        "math/duplicate-label",
        "---\ntitle: A\n---\n## One {#x}\n\n## Two {#x}\n",
        "content/posts/a.md",
        "x",
    ),
    (
        "math/duplicate-label",
        "=== content/posts/a.org\n#+TITLE: A\n\n#+NAME: tbl\n| a |\n\nSee <<tbl>>.\n",
        "content/posts/a.org",
        "tbl",
    ),
    // A Theorem block's Label is in the same namespace, in both formats.
    (
        "math/duplicate-label",
        "---\ntitle: A\n---\n## Main {#thm:main}\n\n:::theorem[Main]{#thm:main}\nx\n:::\n",
        "content/posts/a.md",
        "thm:main",
    ),
    (
        "math/duplicate-label",
        "=== content/posts/a.org\n#+TITLE: A\n\n#+NAME: thm:a\n#+begin_theorem\nx\n#+end_theorem\n\n\\begin{equation}\nE \\label{thm:a}\n\\end{equation}\n",
        "content/posts/a.org",
        "thm:a",
    ),
    (
        "math/invalid-label",
        "---\ntitle: A\n---\n$$\\begin{equation} E \\label{eq/e} \\end{equation}$$\n",
        "content/posts/a.md",
        "eq/e",
    ),
    (
        "math/invalid-label",
        "---\ntitle: A\n---\n:::lemma[thm/a]{#thm/a}\nx\n:::\n",
        "content/posts/a.md",
        "thm/a",
    ),
    (
        "math/invalid-label",
        "=== content/posts/a.org\n#+TITLE: A\n\n#+NAME: thm a\n#+begin_theorem\nx\n#+end_theorem\n",
        "content/posts/a.org",
        "thm a",
    ),
    // An empty-text link to a Label nothing has.
    (
        "link/unresolved",
        "---\ntitle: A\n---\n:::theorem{#thm:a}\nx\n:::\n\nBy [](#thm:b).\n",
        "content/posts/a.md",
        "[](#thm:b)",
    ),
    (
        "link/unresolved",
        "=== content/posts/a.org\n#+TITLE: A\n\n#+NAME: thm:a\n#+begin_theorem\nx\n#+end_theorem\n\nBy [[#thm:b]].\n",
        "content/posts/a.org",
        "[[#thm:b]]",
    ),
    (
        "math/named-environment",
        "=== content/posts/a.org\n#+TITLE: A\n\n#+NAME: eq:e\n\\begin{equation}\nE\n\\end{equation}\n",
        "content/posts/a.org",
        "eq:e",
    ),
    // Config the content contradicts.
    (
        "config/invalid-value",
        "=== yuanme.toml\n[site]\nurl = \"https://e.com\"\n[collections.notes]\ndir = \"content/note\"\nroute = \"/{slug}/\"\n=== content/notes/a.md\n---\ntitle: A\n---\n",
        "yuanme.toml",
        "\"content/note\"",
    ),
    (
        "config/invalid-value",
        "=== yuanme.toml\n[site]\nurl = \"https://e.com\"\n[data.nav]\ntype = \"string\"\n",
        "yuanme.toml",
        "nav",
    ),
];

fn fixture(files: &str) -> String {
    let files = if files.starts_with("=== ") {
        files.to_string()
    } else {
        format!("{ONE_POST}{files}")
    };
    if files.contains("=== yuanme.toml") {
        files
    } else {
        format!("{BASE}{files}")
    }
}

#[test]
fn each_mistake_is_one_diagnostic_with_its_code_on_the_right_text() {
    let wrong: Vec<String> = CASES
        .iter()
        .filter_map(|&(code, files, file, under)| {
            let (sources, site, diagnostics) = resolve(&fixture(files), false);
            let got: Vec<_> = diagnostics
                .iter()
                .map(|d| {
                    let at = d.primary();
                    (
                        d.code.name,
                        sources.path(at.file),
                        &sources.text(at.file)[at.range.clone()],
                        d.message.as_str(),
                    )
                })
                .collect();
            let ok = site.is_none()
                && got.len() == 1
                && (got[0].0, got[0].1, got[0].2) == (code, file, under);
            (!ok).then(|| format!("{files}\n  want {code} in {file} on {under:?}\n  got {got:?}\n"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn every_entry_code_has_a_case_here() {
    let tested: BTreeSet<&str> = CASES
        .iter()
        .map(|c| c.0)
        // The Engine has more of `image/`, for body images, and tests them.
        .filter(|c| !c.starts_with("config/") && !c.starts_with("image/"))
        .chain([
            "markdown/no-effect",
            "math/tex-delimiter",
            "org/no-effect",
            "org/wrong-weekday",
            "org/too-deep",
            // The Engine's, where code is highlighted (#53).
            "entry/unknown-language",
        ])
        .collect();
    let all: BTreeSet<&str> = codes::ALL
        .iter()
        .map(|c| c.name)
        .filter(|n| {
            ["entry/", "org/", "markdown/", "math/", "link/"]
                .iter()
                .any(|a| n.starts_with(a))
        })
        // The Engine checks the built Site's links (#15), and tests them.
        .filter(|n| !["link/broken", "link/redirected"].contains(n))
        .collect();
    assert_eq!(tested, all);
}

#[test]
fn every_mistake_across_the_site_is_reported() {
    let files = "=== content/posts/a.md\n---\ntitle: 1\nn: 0\n---\n=== content/posts/b.md\n---\ndate: 2025-1-1\n---\n";
    let (sources, site, diagnostics) = resolve(&fixture(files), false);
    assert!(site.is_none());
    let got: Vec<_> = diagnostics
        .iter()
        .map(|d| (sources.path(d.primary().file), d.code.name))
        .collect();
    assert_eq!(
        got,
        [
            ("content/posts/a.md", "entry/wrong-type"),
            ("content/posts/a.md", "entry/out-of-range"),
            ("content/posts/b.md", "entry/missing-field"),
            ("content/posts/b.md", "entry/invalid-date"),
        ]
    );
}

#[test]
fn latex_delimiters_in_markdown_warn_and_the_entry_still_builds() {
    let files = "---\ntitle: A\n---\nThe area is \\(\\pi r^2\\).\n\nNot `\\(x\\)`, $[a, b]$ or \\[1].\n\n\\begin{equation}\nx\n\\end{equation}\n";
    let (sources, site, diagnostics) = resolve(&fixture(files), false);
    assert!(site.is_some());
    let got: Vec<_> = diagnostics
        .iter()
        .map(|d| {
            let at = d.primary();
            (d.code.name, &sources.text(at.file)[at.range.clone()])
        })
        .collect();
    assert_eq!(
        got,
        [
            ("math/tex-delimiter", "\\("),
            ("math/tex-delimiter", "\\begin{equation}"),
        ]
    );
}

#[test]
fn a_title_on_a_plain_directive_warns_and_the_entry_still_builds() {
    for (files, code, under) in [
        (
            "---\ntitle: A\n---\n::::tip[Heads up]\nx\n::::\n",
            "markdown/no-effect",
            "::::",
        ),
        (
            "=== content/posts/a.org\n#+TITLE: A\n\n#+begin_tip Heads up\nx\n#+end_tip\n",
            "org/no-effect",
            "Heads up",
        ),
        // What a directive's opening line has that yuanme doesn't read.
        (
            "---\ntitle: A\n---\n:::theorem[Main]{#thm:a} more\nx\n:::\n",
            "markdown/no-effect",
            "more",
        ),
        (
            "---\ntitle: A\n---\n:::theorem[Main\nx\n:::\n",
            "markdown/no-effect",
            "[Main",
        ),
    ] {
        let (sources, site, diagnostics) = resolve(&fixture(files), false);
        assert!(site.is_some(), "{diagnostics:#?}");
        let [d] = diagnostics.as_slice() else {
            panic!("{diagnostics:#?}")
        };
        assert_eq!((d.code.name, d.level), (code, yuanme_diag::Level::Warning));
        assert_eq!(
            &sources.text(d.primary().file)[d.primary().range.clone()],
            under
        );
    }
}

/// Kinds of the Site's own, each numbered on the counter it names.
#[test]
fn a_site_numbers_its_own_theorem_kinds() {
    let files = "=== yuanme.toml\n[site]\nurl = \"https://e.com\"\n[collections.posts]\nroute = \"/{slug}/\"\n[math.theorems.claim]\ncounter = \"claim\"\n[math.theorems.lemma]\nlabel = \"Lemmata\"\n[math.theorems.theorem]\ncounter = false\n=== content/posts/a.md\n---\ntitle: A\n---\n:::claim\nx\n:::\n\n:::theorem\nx\n:::\n\n:::lemma\nx\n:::\n\n:::claim\nx\n:::\n\n:::definition\nx\n:::\n";
    let (_, site, diagnostics) = resolve(files, false);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let doc = &site.expect("no errors").entries[0].document;
    let names: Vec<_> = doc
        .descendants(Document::ROOT)
        .into_iter()
        .filter_map(|id| match &doc.node(id).kind {
            Kind::Theorem { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        names,
        ["Claim 1", "Theorem", "Lemmata 1", "Claim 2", "Definition 2"]
    );
}

#[test]
fn front_matter_after_a_byte_order_mark_is_read() {
    let (_, site, diagnostics) = resolve(&fixture("\u{FEFF}---\ntitle: A\n---\nBody.\n"), false);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert_eq!(site.expect("no errors").entries[0].title, "A");
}

#[test]
fn spans_after_a_lone_carriage_return_point_at_the_right_text() {
    // comrak ends a line at `\r`, `\n` or `\r\n`, as CommonMark does.
    let files = "---\ntitle: A\n---\nOne\r\rSee [b](b.md).\r\n";
    let (sources, _, diagnostics) = resolve(&fixture(files), false);
    let [d] = diagnostics.as_slice() else {
        panic!("{diagnostics:#?}")
    };
    assert_eq!(
        &sources.text(d.primary().file)[d.primary().range.clone()],
        "[b](b.md)"
    );
}

/// Front matter as the JSON a schema validates.
fn front_matter(text: &str) -> serde_json::Value {
    use yuanme_content::yaml::{self, Node, Yaml};
    fn json(n: &Node) -> serde_json::Value {
        match &n.value {
            Yaml::Null => serde_json::Value::Null,
            Yaml::Bool(b) => (*b).into(),
            Yaml::Int(i) => (*i).into(),
            Yaml::Float(f) => (*f).into(),
            Yaml::Str(s) => s.as_str().into(),
            Yaml::Seq(items) => items.iter().map(json).collect(),
            Yaml::Map(entries) => entries
                .iter()
                .map(|(k, v)| (k.name.clone(), json(v)))
                .collect(),
        }
    }
    let yaml = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("---\n"))
        .map_or("", |(yaml, _)| yaml);
    json(&yaml::parse(yaml, 0).unwrap())
}

/// `yuanme schema entry` and `schema data` agree with what a build
/// accepts: every Entry and Data file that resolves validates, and front
/// matter the Engine rejects for its fields fails.
#[test]
fn the_front_matter_and_data_schemas_accept_what_resolves() {
    let config_of = |fixture: &str| {
        let mut files = Files::default();
        let text = &project(fixture)[0].1;
        let file = files.add("yuanme.toml", text.as_str());
        config::parse(&files, file, "1.0.0").0.unwrap()
    };
    let validator = |config: &config::Config, collection: &str| {
        let collections = site::collections(config);
        let schema = config::entry_schema(config, collection, &collections[collection]);
        jsonschema::validator_for(&schema).unwrap()
    };

    let (_, site, diagnostics) = resolve(BLOG, true);
    assert!(site.is_some(), "{diagnostics:#?}");
    let config = config_of(BLOG);
    for (path, text) in project(BLOG) {
        let Some(rest) = path.strip_prefix("content/") else {
            continue;
        };
        let Some((collection, _)) = rest.split_once('/').filter(|_| path.ends_with(".md")) else {
            continue;
        };
        let errors: Vec<String> = validator(&config, collection)
            .iter_errors(&front_matter(&text))
            .map(|e| e.to_string())
            .collect();
        assert!(errors.is_empty(), "{path}: {errors:?}");
    }
    let nav = config::data_schema("nav", &config.data["nav"]);
    assert!(jsonschema::is_valid(
        &nav,
        &serde_json::json!([{ "name": "Blog", "href": "/blog/" }])
    ));
    assert!(!jsonschema::is_valid(
        &nav,
        &serde_json::json!([{ "name": "Blog", "url": "/blog/" }])
    ));

    let config = config_of(BASE);
    let schema = validator(&config, "posts");
    for wrong in [
        "---\ntitle: Hi\nn: 0\n---\n",
        "---\ntitle: Hi\nn: 2.5\n---\n",
        "---\ntitle: Hi\nkind: poem\n---\n",
        "---\ntitel: Hi\n---\n",
        "---\ntitle: [Hi]\n---\n",
        "---\ntitle: Hi\ntags: rust\n---\n",
        "---\ntitle: Hi\nseries: [a, b]\n---\n",
        "---\ntitle: Hi\ndraft: yes\n---\n",
    ] {
        let (_, site, _) = resolve(&fixture(wrong), false);
        assert!(site.is_none(), "the Engine accepts {wrong:?}");
        assert!(
            !schema.is_valid(&front_matter(wrong)),
            "the schema accepts {wrong:?}"
        );
    }
}
