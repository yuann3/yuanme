//! Sites rendered through their Templates (#19): every kind of Route, the
//! Engine's filters, the feed and the sitemap, and every Template error.

use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::Path;

use yuanme_content::config;
use yuanme_content::site::{self, Project};
use yuanme_diag::{Diagnostic, Files, codes};
use yuanme_render::template::{self, Page};

/// Every file under `dir`, by its path from `dir`.
fn read(dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let relative = path.strip_prefix(dir).unwrap().to_str().unwrap();
                out.push((
                    relative.to_string(),
                    std::fs::read_to_string(&path).unwrap(),
                ));
            }
        }
    }
    out.sort();
    out
}

fn site(name: &str) -> Vec<(String, String)> {
    read(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/sites")
            .join(name),
    )
}

/// Files written as `=== path` lines, each followed by the file's text,
/// over `base`: a file given again replaces the base's.
fn fixture(base: &str, files: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for text in [base, files] {
        let mut current: Option<usize> = None;
        for line in text.split_inclusive('\n') {
            match line.strip_prefix("=== ") {
                Some(path) => {
                    let path = path.trim_end().to_string();
                    out.retain(|(p, _)| *p != path);
                    out.push((path, String::new()));
                    current = Some(out.len() - 1);
                }
                None => out[current.expect("starts with `=== path`")]
                    .1
                    .push_str(line),
            }
        }
    }
    out
}

/// Resolves the project and renders it. The project's content must be
/// right; only its Templates may fail.
fn render(written: &[(String, String)]) -> (Files, Vec<Page>, Vec<Diagnostic>) {
    let mut files = Files::default();
    let text = |path: &str| &written.iter().find(|(p, _)| p == path).expect("listed").1;
    let config_file = files.add("yuanme.toml", text("yuanme.toml").as_str());
    let (config, diagnostics) = config::parse(&files, config_file, "1.0.0");
    let config = config.unwrap_or_else(|| panic!("{diagnostics:#?}"));
    let paths: Vec<&str> = written.iter().map(|(p, _)| p.as_str()).collect();
    let (entries, data) = site::sources(&config, paths.iter().copied());
    let entries: Vec<_> = entries
        .into_iter()
        .map(|(c, p)| (c, files.add(p, text(p).as_str())))
        .collect();
    let data: Vec<_> = data
        .into_iter()
        .map(|p| files.add(p, text(p).as_str()))
        .collect();
    let templates = written
        .iter()
        .filter_map(|(p, t)| Some((p.strip_prefix("templates/")?.to_string(), t)))
        .map(|(name, t)| {
            (
                name.clone(),
                files.add(format!("templates/{name}"), t.as_str()),
            )
        })
        .collect();
    let statics: BTreeSet<String> = paths
        .iter()
        .filter_map(|p| Some(format!("/{}", p.strip_prefix("static/")?)))
        .collect();
    let is_file = |p: &str| paths.contains(&p);
    let is_dir = |d: &str| paths.iter().any(|p| p.starts_with(&format!("{d}/")));
    let (site, diagnostics) = site::resolve(
        &config,
        &Project {
            files: &files,
            entries,
            data,
            is_file: &is_file,
            is_dir: &is_dir,
        },
        false,
    );
    let site = site.unwrap_or_else(|| panic!("{diagnostics:#?}"));
    let (pages, diagnostics) = template::render(&config, &site, &files, &templates, &statics);
    (files, pages, diagnostics)
}

fn page<'p>(pages: &'p [Page], output: &str) -> &'p Page {
    pages
        .iter()
        .find(|p| p.output == output)
        .unwrap_or_else(|| panic!("no {output}"))
}

fn show(pages: &[Page]) -> String {
    let mut out = String::new();
    for p in pages {
        writeln!(out, "=== {}\n{}", p.output, p.text).unwrap();
    }
    out
}

#[test]
fn a_site_renders_every_kind_of_route() {
    let (_, pages, diagnostics) = render(&site("blog"));
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    insta::assert_snapshot!(show(&pages));
}

#[test]
fn each_page_records_the_templates_it_used() {
    let (_, pages, _) = render(&site("blog"));
    // Its own, then what it extends, includes and imports, at any depth.
    assert_eq!(
        page(&pages, "blog/2/index.html").templates,
        [
            "posts/list.html",
            "layouts/base.html",
            "components/row.html",
            "components/pagination.html",
            "partials/nav.html"
        ]
    );
    // A Collection's `template` override.
    assert_eq!(
        page(&pages, "notes/quick/index.html").templates,
        ["posts/entry.html", "layouts/base.html", "partials/nav.html"]
    );
    assert_eq!(page(&pages, "404.html").templates, ["404.html"]);
    assert_eq!(page(&pages, "rss.xml").templates, ["rss.xml"]);
}

#[test]
fn the_reference_templates_from_the_spike_render() {
    let written = site("eyuan");
    let (_, pages, diagnostics) = render(&written);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    // The 18 that spike/template-check ported. Two Templates it lacked,
    // `tags/term.html` and `series/index.html`, were added to the fixture.
    let used: BTreeSet<&str> = pages
        .iter()
        .flat_map(|p| p.templates.iter().map(String::as_str))
        .collect();
    let ported: Vec<&str> = written
        .iter()
        .filter_map(|(p, _)| p.strip_prefix("templates/"))
        .filter(|p| !["tags/term.html", "series/index.html"].contains(p))
        .collect();
    assert_eq!(ported.len(), 18);
    for t in ported {
        assert!(used.contains(t), "{t} rendered no Page");
    }
    let outputs: Vec<&str> = pages.iter().map(|p| p.output.as_str()).collect();
    let shown: Vec<Page> = [
        "blog/first/index.html",
        "projects/lrle/index.html",
        "tags/index.html",
    ]
    .iter()
    .map(|o| {
        let p = page(&pages, o);
        Page {
            output: p.output.clone(),
            text: p.text.clone(),
            templates: Vec::new(),
        }
    })
    .collect();
    insta::assert_snapshot!(format!("{outputs:#?}\n{}", show(&shown)));
}

/// A Site whose home page is the case's Template.
const BASE: &str = r#"=== yuanme.toml
[site]
url = "https://e.com"
timezone = "Asia/Singapore"

[collections.posts]
route = "/blog/{slug}/"

[collections.posts.fields]
order = { type = "int", optional = true }
at = { type = "datetime", optional = true }
hero = { type = "object", optional = true, fields = { src = { type = "string" } } }

[routes.home]
path = "/"
template = "home.html"
=== content/posts/a.md
---
title: A
date: 2025-01-02
order: 2
at: 2025-01-01T20:00:00Z
---
=== content/posts/b.md
---
title: B
date: 2025-01-02
---
=== content/posts/c.md
---
title: C
date: 2024-05-06
order: 1
---
=== templates/posts/entry.html
{{ entry.title }}
=== templates/home.html
=== static/logo.svg
"#;

#[test]
fn the_engine_filters_and_escaping() {
    // (the home page's Template, what it renders)
    let cases: &[(&str, &str)] = &[
        // Missing values last whichever the direction, ties by Slug.
        (
            r#"{% for e in collections.posts|sort_by(["-order"]) %}{{ e.slug }}{% endfor %}"#,
            "acb",
        ),
        (
            r#"{% for e in collections.posts|sort_by(["order"]) %}{{ e.slug }}{% endfor %}"#,
            "cab",
        ),
        (
            r#"{% for e in collections.posts|sort_by(["-date", "title"]) %}{{ e.slug }}{% endfor %}"#,
            "abc",
        ),
        // Jinja's own sort puts the missing value first.
        (
            r#"{% for e in collections.posts|sort(attribute="order") %}{{ e.slug }}{% endfor %}"#,
            "bca",
        ),
        // A datetime in `site.timezone`; a date stays its calendar day.
        (
            r#"{% set a = collections.posts|selectattr("slug", "eq", "a")|first %}{{ a.at }} {{ a.at|dateformat(format="%Y-%m-%d %H:%M") }} {{ a.date|dateformat(format="%Y-%m-%d %H:%M %:z") }}"#,
            "2025-01-01T20:00:00+00:00 2025-01-02 04:00 2025-01-02 00:00 +00:00",
        ),
        (
            r#"{{ "2024-07-01"|dateformat(format="rfc2822") }} {{ "2024-07-01"|dateformat(format="rfc3339") }}"#,
            "Mon, 01 Jul 2024 00:00:00 GMT 2024-07-01T00:00:00+00:00",
        ),
        (
            r#"{{ url("/blog/a/") }} {{ url("/blog/a/#top") }} {{ url("/logo.svg") }} {{ absolute_url("/x/") }} {{ absolute_url("https://o.org/") }}"#,
            "/blog/a/ /blog/a/#top /logo.svg https://e.com/x/ https://o.org/",
        ),
        (
            r#"{{ 1|pluralize }} {{ 2|pluralize }} {{ 1|pluralize("y", "ies") }} {{ 0|pluralize("y", "ies") }}"#,
            " s y ies",
        ),
        (
            r#"<a{{ {"href": "/a?b=1&c=\"2\"", "hidden": true, "open": false, "x": none, "y": undefined_name}|attrs }}>"#,
            r#"<a hidden href="/a?b=1&amp;c=&quot;2&quot;">"#,
        ),
        // The standard five, and not `/`; a null prints as nothing.
        (
            r#"{{ "</a> & 'b' \"c\"" }}|{{ none }}|{{ true }}|{{ "<i>"|safe }}|{{ "a/b"|e }}"#,
            "&lt;/a&gt; &amp; &#39;b&#39; &quot;c&quot;||true|<i>|a/b",
        ),
        (
            r#"{{ "*hi* -- [x](/blog/a/)"|markdown }}|{{ "*hi*"|markdown(inline=true) }}"#,
            "<p><em>hi</em> — <a href=\"/blog/a/\">x</a></p>|<em>hi</em>",
        ),
        // An absent optional field is false, and `default` covers it.
        (
            r#"{% set b = collections.posts|selectattr("slug", "eq", "b")|first %}{% if b.hero %}hero{% else %}none{% endif %} {{ b.order|default(0) }} {{ b.hero is defined }}"#,
            "none 0 false",
        ),
        (
            "{{ page.url }} {{ page.kind }} {{ page.canonical }}",
            "/ template https://e.com/",
        ),
    ];
    for (source, expected) in cases {
        let files = format!("=== templates/home.html\n{source}");
        let (_, pages, diagnostics) = render(&fixture(BASE, &files));
        assert!(diagnostics.is_empty(), "{source}: {diagnostics:#?}");
        assert_eq!(page(&pages, "index.html").text, *expected, "{source}");
    }
    // A Template that isn't HTML or XML isn't escaped.
    let files = "=== templates/home.txt\n{{ \"<&>\" }}\n=== yuanme.toml\n[site]\nurl = \"https://e.com\"\n[routes.home]\npath = \"/home.txt\"\ntemplate = \"home.txt\"\n";
    let (_, pages, _) = render(&fixture("", files));
    assert_eq!(page(&pages, "home.txt").text, "<&>");
}

#[test]
fn a_feed_holds_published_listed_entries_newest_first_and_a_file_replaces_its_template() {
    let config = r#"=== yuanme.toml
[site]
url = "https://e.com"
title = "E's"

[collections.posts]
route = "/blog/{slug}/"
fields = { order = { type = "int", optional = true }, at = { type = "datetime", optional = true } }

[feeds.rss]
collections = ["posts"]
path = "/rss.xml"
=== content/posts/d.md
---
title: Draft
date: 2026-01-01
draft: true
---
=== content/posts/u.md
---
title: Unlisted
date: 2026-01-01
unlisted: true
---
"#;
    let (_, pages, diagnostics) = render(&fixture(BASE, config));
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    insta::assert_snapshot!(page(&pages, "rss.xml").text);
    // The sitemap lists Pages, not feeds, and no Unlisted Entry.
    insta::assert_snapshot!(show(
        &pages
            .into_iter()
            .filter(|p| p.output.starts_with("sitemap"))
            .collect::<Vec<_>>()
    ));

    let replaced = format!(
        "{config}=== templates/rss.xml\n{{% for e in entries %}}{{{{ e.slug }}}}{{% endfor %}}"
    );
    let (_, pages, _) = render(&fixture(BASE, &replaced));
    assert_eq!(page(&pages, "rss.xml").text, "abc");
}

/// (code, files over `BASE`, the file and text its primary span covers)
const ERRORS: &[(&str, &str, &str, &str)] = &[
    // A Route's Template is chosen in Site config, so the error is there.
    (
        "template/not-found",
        "=== templates/posts/entry.html\n=== yuanme.toml\n[site]\nurl = \"https://e.com\"\n[collections.posts]\nroute = \"/blog/{slug}/\"\ntemplate = \"post.html\"\n[collections.posts.fields]\norder = { type = \"int\", optional = true }\nat = { type = \"datetime\", optional = true }\n",
        "yuanme.toml",
        "\"post.html\"",
    ),
    (
        "template/not-found",
        "=== templates/home.html\n<p>{% include \"partials/nav.html\" %}</p>",
        "templates/home.html",
        "include \"partials/nav.html\"",
    ),
    (
        "template/syntax",
        "=== templates/home.html\n{% if true %}open",
        "templates/home.html",
        "",
    ),
    // Printing an absent field, in the entry Template every Post uses: one
    // Diagnostic, not one for each Page.
    (
        "template/render",
        "=== templates/posts/entry.html\n<img src=\"{{ entry.hero }}\">",
        "templates/posts/entry.html",
        "entry.hero",
    ),
    (
        "template/render",
        "=== templates/home.html\n{{ collections.posts[0].hero.src }}",
        "templates/home.html",
        "hero.src",
    ),
    // Undefined, as minijinja's own filters would find it.
    (
        "template/render",
        "=== templates/home.html\n{{ collections.posts[1].at|dateformat(format=\"%Y\") }}",
        "templates/home.html",
        "dateformat",
    ),
    (
        "template/unknown-filter",
        "=== templates/home.html\n{{ \"x\"|date_format }}",
        "templates/home.html",
        "date_format",
    ),
    (
        "template/bad-filter-argument",
        "=== templates/home.html\n{{ \"2025-01-01\"|dateformat(format=\"iso\") }}",
        "templates/home.html",
        "dateformat(format=\"iso\")",
    ),
    (
        "template/bad-filter-argument",
        "=== templates/home.html\n{{ \"2025-01-01\"|dateformat(fromat=\"%Y\") }}",
        "templates/home.html",
        "dateformat(fromat=\"%Y\")",
    ),
    (
        "template/unknown-url",
        "=== templates/home.html\n<a href=\"{{ url('/blog/z/') }}\">",
        "templates/home.html",
        "url('/blog/z/')",
    ),
    (
        "org/not-yet-supported",
        "=== templates/home.html\n{{ \"* Heading\"|org }}",
        "templates/home.html",
        "org",
    ),
];

#[test]
fn each_template_mistake_is_one_diagnostic_with_its_code_on_the_right_text() {
    for (code, files, file, covered) in ERRORS {
        let (sources, pages, diagnostics) = render(&fixture(BASE, files));
        assert!(pages.is_empty(), "{code}: a failed render writes nothing");
        let [d] = diagnostics.as_slice() else {
            panic!("{code} in {files}: {diagnostics:#?}");
        };
        assert_eq!(d.code.name, *code, "{d:#?}");
        let at = d.primary();
        assert_eq!(sources.path(at.file), *file, "{d:#?}");
        let text = &sources.text(at.file)[at.range.clone()];
        assert!(
            text.contains(covered),
            "{code}: {text:?} doesn't cover {covered:?}"
        );
    }
}

#[test]
fn every_template_code_has_a_case_here() {
    let tested: BTreeSet<&str> = ERRORS
        .iter()
        .map(|c| c.0)
        .filter(|c| c.starts_with("template/"))
        .collect();
    let all: BTreeSet<&str> = codes::ALL
        .iter()
        .map(|c| c.name)
        .filter(|n| n.starts_with("template/"))
        .collect();
    assert_eq!(tested, all);
}

#[test]
fn rendered_render_error() {
    let files = "=== templates/posts/entry.html\n<h1>{{ entry.title }}</h1>\n<img src=\"{{ entry.hero }}\">\n";
    let (sources, _, diagnostics) = render(&fixture(BASE, files));
    insta::assert_snapshot!(diagnostics[0].render(&sources, false));
}
