//! Whole Sites read from fixture projects: what resolves, and every way an
//! Entry or Data file can be wrong.

use std::collections::BTreeSet;
use std::fmt::Write;

use yuanme_content::config;
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
        "entry/invalid-value",
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
        "org/not-yet-supported",
        "=== content/posts/a.org\n#+title: A\n",
        "content/posts/a.org",
        "",
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
        .filter(|c| !c.starts_with("config/"))
        .chain(["markdown/no-effect"])
        .collect();
    let all: BTreeSet<&str> = codes::ALL
        .iter()
        .map(|c| c.name)
        .filter(|n| {
            ["entry/", "org/", "markdown/", "link/"]
                .iter()
                .any(|a| n.starts_with(a))
        })
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
fn a_title_on_a_plain_directive_warns_and_the_entry_still_builds() {
    let files = "---\ntitle: A\n---\n::::note[Heads up]\nx\n::::\n";
    let (sources, site, diagnostics) = resolve(&fixture(files), false);
    assert!(site.is_some());
    let [d] = diagnostics.as_slice() else {
        panic!("{diagnostics:#?}")
    };
    assert_eq!(
        (d.code.name, d.level),
        ("markdown/no-effect", yuanme_diag::Level::Warning)
    );
    assert_eq!(
        &sources.text(d.primary().file)[d.primary().range.clone()],
        "::::"
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
