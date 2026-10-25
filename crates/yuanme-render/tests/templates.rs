//! Sites rendered through their Templates (#19): every kind of Route, the
//! Engine's filters, the feed and the sitemap, and every Template error.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::Path;

use yuanme_content::config::{self, Config};
use yuanme_content::site::{self, Project, Site};
use yuanme_diag::{Diagnostic, FileId, Files, codes};
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
    let project = resolve(written);
    let (pages, diagnostics) = template::render(
        &project.config,
        &project.site,
        &project.files,
        &project.templates,
        &project.statics,
    );
    (project.files, pages, diagnostics)
}

struct Resolved {
    files: Files,
    config: Config,
    site: Site,
    templates: BTreeMap<String, FileId>,
    statics: BTreeSet<String>,
}

fn resolve(written: &[(String, String)]) -> Resolved {
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
    Resolved {
        files,
        config,
        site,
        templates,
        statics,
    }
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

/// The schemas `yuanme schema context` prints describe what Pages really
/// see: each Template prints its whole context as JSON, which must match
/// its kind's schema.
#[test]
fn every_page_sees_what_its_context_schema_says() {
    let mut written = site("blog");
    for (template, vars) in [
        ("posts/entry.html", "entry"),
        ("pages/entry.html", "entry"),
        ("posts/list.html", "collection entries paginator"),
        ("tags/term.html", "taxonomy term entries paginator"),
        ("term.html", "taxonomy term entries paginator"),
        ("tags/index.html", "taxonomy terms"),
        ("series/index.html", "taxonomy terms"),
        ("home.html", ""),
        ("404.html", ""),
        ("llms.txt", ""),
        ("rss.xml", "feed entries"),
    ] {
        let names = "site data collections taxonomies page ".to_string() + vars;
        let pairs: Vec<String> = names
            .split_whitespace()
            .map(|n| format!("'{n}': {n}"))
            .collect();
        let path = format!("templates/{template}");
        written.retain(|(p, _)| *p != path);
        written.push((
            path,
            format!("{{{{ {{{}}} | tojson }}}}\n", pairs.join(", ")),
        ));
    }
    let project = resolve(&written);
    let schemas = template::context_schemas(&project.config, &project.site);
    assert_eq!(
        schemas.keys().copied().collect::<Vec<_>>(),
        ["entry", "feed", "index", "list", "template", "term"]
    );
    let (pages, diagnostics) = template::render(
        &project.config,
        &project.site,
        &project.files,
        &project.templates,
        &project.statics,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let mut kinds = BTreeSet::new();
    for page in pages.iter().filter(|p| !p.output.starts_with("sitemap")) {
        let context: serde_json::Value = serde_json::from_str(&page.text)
            .unwrap_or_else(|e| panic!("{}: {e}\n{}", page.output, page.text));
        let kind = context["page"]["kind"].as_str().unwrap().to_string();
        let validator = jsonschema::validator_for(&schemas[kind.as_str()]).unwrap();
        let errors: Vec<String> = validator
            .iter_errors(&context)
            .map(|e| format!("{} at {}", e, e.instance_path()))
            .collect();
        assert!(errors.is_empty(), "{}: {errors:#?}", page.output);
        kinds.insert(kind);
    }
    assert_eq!(kinds.len(), schemas.len(), "a Page of every kind");

    // The schema is closed: a field no Template sees fails it.
    let mut context: serde_json::Value =
        serde_json::from_str(&page(&pages, "index.html").text).unwrap();
    context["site"]["titel"] = "x".into();
    assert!(!jsonschema::is_valid(&schemas["template"], &context));
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
            r#"{% set a = collections.posts|selectattr("slug", "eq", "a")|first %}{% if a.at and a.date %}{{ a.at }} {{ a.at|dateformat(format="%Y-%m-%d %H:%M") }} {{ a.date|dateformat(format="%Y-%m-%d %H:%M %:z") }}{% endif %}"#,
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
            r#"<a{{ {"href": "/a?b=1&c=\"2\"", "hidden": true, "open": false, "x": none, "y": collections.posts[0].hero}|attrs }}>"#,
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
        (
            r#"{{ "/hi/ -- [[https://o.org][x]]"|org }}|{{ "/hi/"|org(inline=true) }}"#,
            "<p><em>hi</em> – <a href=\"https://o.org\">x</a></p>|<em>hi</em>",
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
        "\"partials/nav.html\"",
    ),
    (
        "template/syntax",
        "=== templates/home.html\n{% if true %}open",
        "templates/home.html",
        "",
    ),
    // What `check` can't see: an index past the end, in the entry Template
    // every Post uses. One Diagnostic, not one for each Page.
    (
        "template/render",
        "=== templates/posts/entry.html\n<h2>{{ entry.headings[0].text }}</h2>",
        "templates/posts/entry.html",
        "text",
    ),
    // Undefined, as minijinja's own filters would find it: `map` loses the
    // field's type.
    (
        "template/render",
        "=== templates/home.html\n{% for at in collections.posts|map(attribute=\"at\") %}{{ at|dateformat(format=\"%Y\") }}{% endfor %}",
        "templates/home.html",
        "dateformat",
    ),
    // `check`, before any Page renders.
    (
        "template/unknown-field",
        "=== templates/posts/entry.html\n{% if entry.order %}{{ entry.titel }}{% endif %}",
        "templates/posts/entry.html",
        "titel",
    ),
    (
        "template/unguarded-optional",
        "=== templates/posts/entry.html\n<img src=\"{{ entry.hero.src }}\">",
        "templates/posts/entry.html",
        "entry.hero",
    ),
    (
        "template/compare-optional",
        "=== templates/posts/entry.html\n{% if entry.order == 1 %}first{% endif %}",
        "templates/posts/entry.html",
        "entry.order",
    ),
    (
        "template/wrong-type",
        "=== templates/home.html\n{% for c in site.url %}{{ c }}{% endfor %}",
        "templates/home.html",
        "site.url",
    ),
    (
        "template/non-constant-name",
        "=== templates/home.html\n{% include \"partials/\" ~ page.kind ~ \".html\" %}",
        "templates/home.html",
        "\"partials/\" ~ page.kind ~ \".html\"",
    ),
    (
        "template/undeclared-input",
        "=== templates/components/count.html\n{# props: label: string #}\n{% macro count(label) %}{{ label }}: {{ collections.posts|length }} of {{ entries|length }}{% endmacro %}",
        "templates/components/count.html",
        "entries",
    ),
    (
        "template/bad-prop",
        "=== templates/components/count.html\n{# props: label: string #}\n{% macro count(label) %}{{ label }}{% endmacro %}\n=== templates/home.html\n{% from \"components/count.html\" import count %}{{ count(label=site.url, lable=\"x\") }}",
        "templates/home.html",
        "lable",
    ),
    (
        "template/bad-header",
        "=== templates/components/count.html\n{# props: label: strng #}\n{% macro count(label) %}{{ label }}{% endmacro %}",
        "templates/components/count.html",
        "strng",
    ),
    (
        "template/bad-var",
        "=== templates/base.html\n{# vars: title: string #}<title>{{ title }}</title>{% block main %}{% endblock %}\n=== templates/home.html\n{% extends \"base.html\" %}{% block main %}Home{% endblock %}",
        "templates/home.html",
        "\"base.html\"",
    ),
    (
        "template/unknown-block",
        "=== templates/base.html\n{% block main %}{% endblock %}\n=== templates/home.html\n{% extends \"base.html\" %}{% block mian %}Home{% endblock %}",
        "templates/home.html",
        "mian",
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
        "\"iso\"",
    ),
    (
        "template/bad-filter-argument",
        "=== templates/home.html\n{{ \"2025-01-01\"|dateformat(fromat=\"%Y\") }}",
        "templates/home.html",
        "fromat",
    ),
    (
        "template/unknown-url",
        "=== templates/home.html\n<a href=\"{{ url('/blog/z/') }}\">",
        "templates/home.html",
        "url('/blog/z/')",
    ),
    // Recursion that never ends stops at minijinja's limit, before the
    // render thread's stack runs out and aborts the process.
    (
        "template/render",
        "=== templates/components/c.html\n{# props: #}\n{% macro c() %}{{ self.main() }}{% endmacro %}\n=== templates/home.html\n{% from \"components/c.html\" import c %}{% block main %}{{ c() }}{% endblock %}",
        "templates/home.html",
        "c()",
    ),
    // Org the Engine reports as an error fails its filter.
    (
        "template/render",
        "=== templates/home.html\n{{ \"#+SETUPFILE: x.org\"|org }}",
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
    let files = "=== templates/posts/entry.html\n<h1>{{ entry.title }}</h1>\n<h2>{{ entry.headings[0].text }}</h2>\n";
    let (sources, _, diagnostics) = render(&fixture(BASE, files));
    insta::assert_snapshot!(diagnostics[0].render(&sources, false));
}

#[test]
fn rendered_check_error() {
    let files = "=== templates/posts/entry.html\n<h1>{{ entry.titel }}</h1>\n";
    let (sources, _, diagnostics) = render(&fixture(BASE, files));
    insta::assert_snapshot!(diagnostics[0].render(&sources, false));
}

/// The 48 mistakes spike/template-check seeded into the Reference
/// Templates (#19), each one edit: (file, find, replace, the code, and
/// the text its primary span covers in the file it names). `§§` separates
/// two edits in one file. The spike's seed 30 called `assets()`; with
/// `asset()` withdrawn, it misspells `absolute_url` instead.
#[rustfmt::skip]
const SEEDS: &[(&str, &str, &str, &str, &str, &str)] = &[
    ("layouts/post.html", "{% set title = entry.title %}", "{% set title = entry.titel %}", "template/unknown-field", "layouts/post.html", "titel"),
    ("layouts/post.html", "entry.hero.src", "entry.hero.scr", "template/unknown-field", "layouts/post.html", "scr"),
    ("layouts/post.html", "entry.hero.alt|default('')", "entry.hero.atl|default('')", "template/unknown-field", "layouts/post.html", "atl"),
    ("layouts/shell.html", "{{ s.title }}", "{{ s.titel }}", "template/unknown-field", "layouts/shell.html", "titel"),
    ("layouts/shell.html", "site.extra.greeting", "site.extra.greting", "template/unknown-field", "layouts/shell.html", "greting"),
    ("layouts/post.html", "entry.modified", "entry.modifed", "template/unknown-field", "layouts/post.html", "modifed"),
    ("partials/post_meta.html", "entry.reading_time", "entry.reading_tim", "template/unknown-field", "partials/post_meta.html", "reading_tim"),
    ("posts/list.html", "prev=paginator.prev", "prev=paginator.previous", "template/unknown-field", "posts/list.html", "previous"),
    ("projects/entry.html", "{{ entry.content }}\n    {% endcall", "{{ entry.contents }}\n    {% endcall", "template/unknown-field", "projects/entry.html", "contents"),
    ("projects/entry.html", "hero.url", "hero.src", "template/unknown-field", "projects/entry.html", "src"),
    ("partials/post_meta.html", "{% if entry.draft %}", "{% if entry.draft and entry.sereis %}", "template/unknown-field", "partials/post_meta.html", "sereis"),
    ("components/list_row.html", "data-row-name>{{ title }}", "data-row-name>{{ titel }}", "template/undeclared-input", "components/list_row.html", "titel"),
    ("components/post_list.html", "{% if posts %}", "{% if entries %}", "template/undeclared-input", "components/post_list.html", "entries"),
    ("components/post_list.html", "post.description", "post.descripton", "template/unknown-field", "components/post_list.html", "descripton"),
    // `object` is unchecked by design (#19), so this one is missed.
    ("components/post_list.html", "list<entry<posts>>§§post.description", "list<object>§§post.descripton", "", "", ""),
    ("posts/list.html", "panel_header(title=\"Writing\")", "panel_header(titel=\"Writing\")", "template/bad-prop", "posts/list.html", "titel"),
    ("tags/index.html", "list_row(href=t.url, title=", "list_row(title=", "template/bad-prop", "tags/index.html", "list_row"),
    ("posts/list.html", "prev=paginator.prev", "prev=paginator.number", "template/bad-prop", "posts/list.html", "paginator.number"),
    ("series/term.html", "panel_header(title=term.name)", "panel_header(title=entries)", "template/bad-prop", "series/term.html", "entries"),
    ("partials/post_meta.html", "formatted_date(date=entry.date)", "formatted_date(date=\"yesterday\")", "template/bad-prop", "partials/post_meta.html", "\"yesterday\""),
    ("partials/post_meta.html", "formatted_date(date=entry.date)", "formatted_date(date=entry.updated)", "template/bad-prop", "partials/post_meta.html", "entry.updated"),
    ("components/prose.html", "  {{ caller() }}\n", "  \n", "template/bad-prop", "projects/entry.html", "ui.prose"),
    ("components/list_row.html", "meta?: string #}", "metta?: string #}", "template/bad-header", "components/list_row.html", "metta"),
    ("components/pagination.html", "prev?: url", "prev?: uri", "template/bad-header", "components/pagination.html", "uri"),
    ("components/wordmark.html", "#}\n{% macro wordmark", "#}\n<!-- wordmark -->\n{% macro wordmark", "template/bad-header", "components/wordmark.html", "props:"),
    ("posts/list.html", "import pagination %}", "import paginator_nav %}", "template/unknown-filter", "posts/list.html", "paginator_nav"),
    ("projects/entry.html", "ui.prose(", "ui.prosa(", "template/unknown-filter", "projects/entry.html", "prosa"),
    ("projects/entry.html", "entry.title|lower", "entry.title|lowercase", "template/unknown-filter", "projects/entry.html", "lowercase"),
    ("layouts/shell.html", "section is defined else 'idle'", "section is definded else 'idle'", "template/unknown-filter", "layouts/shell.html", "definded"),
    ("components/base_head.html", "{{ absolute_url(image) }}", "{{ absolute_uri(image) }}", "template/unknown-filter", "components/base_head.html", "absolute_uri"),
    ("projects/entry.html", "entry.title|lower", "entry.title.lower()", "template/unknown-filter", "projects/entry.html", "lower"),
    ("components/formatted_date.html", "<time datetime=\"{{ date|dateformat(format=", "<time datetime=\"{{ date|dateformat(fromat=", "template/bad-filter-argument", "components/formatted_date.html", "fromat"),
    ("components/base_head.html", "dateformat(format=\"%Y-%m-%d\")", "dateformat(format=\"iso\")", "template/bad-filter-argument", "components/base_head.html", "\"iso\""),
    ("layouts/post.html", "{% include \"partials/post_meta.html\" %}", "{% include \"partials/\" ~ page.kind ~ \"_meta.html\" %}", "template/non-constant-name", "layouts/post.html", "\"partials/\" ~ page.kind"),
    ("tags/index.html", "{% extends \"layouts/shell.html\" %}", "{% extends \"layouts/\" ~ page.kind ~ \".html\" %}", "template/non-constant-name", "tags/index.html", "\"layouts/\" ~ page.kind"),
    ("posts/entry.html", "partials/copy_btn.html", "partials/copy_button.html", "template/not-found", "posts/entry.html", "copy_button"),
    ("posts/entry.html", "{% block after_article %}", "{% block after_artcle %}", "template/unknown-block", "posts/entry.html", "after_artcle"),
    ("tags/index.html", "{% set title = \"Tags\" %}\n", "", "template/bad-var", "tags/index.html", "\"layouts/shell.html\""),
    ("posts/list.html", "{% set section = \"writing\" %}", "{% set section = \"writting\" %}", "template/bad-var", "posts/list.html", "\"writting\""),
    ("partials/post_meta.html", "<span>{{ entry.reading_time }} min read</span>", "<span>{{ entry.series }}</span>", "template/unguarded-optional", "partials/post_meta.html", "entry.series"),
    ("layouts/post.html", "{% if entry.hero %}<img", "{% if entry.title %}<img", "template/unguarded-optional", "layouts/post.html", "entry.hero"),
    ("projects/entry.html", "entry.github|default(\"\") %}", "entry.github %}", "template/compare-optional", "projects/entry.html", "entry.github"),
    ("partials/post_meta.html", "for tag in entry.tags", "for tag in entry.title", "template/wrong-type", "partials/post_meta.html", "entry.title"),
    ("components/base_head.html", "<meta name=\"author\" content=\"{{ site.author }}\"", "<meta name=\"author\" content=\"{{ site.author.name }}\"", "template/unknown-field", "components/base_head.html", "name"),
    ("tags/index.html", "{% if terms %}", "{% if entries %}", "template/unknown-field", "tags/index.html", "entries"),
    ("tags/index.html", "terms|sort_by([\"-count\"])", "terms|sort(attribute=\"-count\")", "template/bad-filter-argument", "tags/index.html", "\"-count\""),
    ("series/term.html", "sort_by([\"date\"])", "sort_by([\"dat\"])", "template/unknown-field", "series/term.html", "\"dat\""),
    ("components/pagination.html", "{% endif %}\n{% endmacro %}", "{% endmacro %}", "template/syntax", "components/pagination.html", ""),
];

#[test]
fn the_spikes_seeded_mistakes_are_caught_where_they_are() {
    let base = site("eyuan");
    let mut missed = Vec::new();
    for (n, &(file, find, replace, code, at, covered)) in SEEDS.iter().enumerate() {
        let seed = n + 1;
        let mut written = base.clone();
        let (_, text) = written
            .iter_mut()
            .find(|(p, _)| *p == format!("templates/{file}"))
            .unwrap_or_else(|| panic!("seed {seed}: no {file}"));
        for (f, r) in find.split("§§").zip(replace.split("§§")) {
            assert!(text.contains(f), "seed {seed}: {f:?} isn't in {file}");
            *text = text.replacen(f, r, 1);
        }
        let (sources, _, diagnostics) = render(&written);
        if code.is_empty() {
            assert!(diagnostics.is_empty(), "seed {seed}: {diagnostics:#?}");
            missed.push(seed);
            continue;
        }
        // Before any Page renders: rendering alone caught 29 (#19).
        let found = diagnostics.iter().any(|d| {
            let span = d.primary();
            let rendered = d
                .children
                .iter()
                .any(|c| c.message.starts_with("while rendering"));
            !rendered
                && d.code.name == code
                && sources.path(span.file) == format!("templates/{at}")
                && sources.text(span.file)[span.range.clone()].contains(covered)
        });
        assert!(
            found,
            "seed {seed}: no {code} on {covered:?} in {at}: {diagnostics:#?}"
        );
    }
    // #55 is done at 45 of 48.
    assert_eq!(missed, [15]);
}

#[test]
fn check_accepts_the_idioms_the_spike_found_legitimate() {
    // Spike #19's 30 idioms, as the Template of a Post, but its 12th: an
    // early `{% continue %}` is no guard to `check`.
    #[rustfmt::skip]
    let idioms = [
        "{% if not entry.hero %}none{% else %}{{ entry.hero.src }}{% endif %}",
        "{% if entry.hero is undefined %}none{% else %}{{ entry.hero.src }}{% endif %}",
        "{{ entry.hero.src if entry.hero is defined else \"\" }}",
        "{% if entry.hero and entry.hero.alt %}{{ entry.hero.alt }}{% endif %}",
        "{% if entry.hero %}{% set h = entry.hero %}{{ h.src }}{% endif %}",
        "{% set h = entry.hero|default(none) %}{% if h %}{{ h.src }}{% endif %}",
        "{{ entry.updated or entry.date }}",
        "{{ entry.series|default(\"none\")|upper }}",
        "{% if entry.updated is defined and entry.updated != entry.date %}u{% endif %}",
        "{% if entry.series is defined %}{% set sname = entry.series %}{% else %}{% set sname = \"none\" %}{% endif %}{{ sname }}",
        "{% for p in collections.posts if p.series is defined %}{{ p.series }}{% endfor %}",
        "{% for p in collections.posts %}{{ loop.index }}{{ loop.previtem.title if loop.previtem else \"\" }}{% endfor %}",
        "{% set ns = namespace(n=0) %}{% for t in entry.tags %}{% set ns.n = ns.n + 1 %}{% endfor %}{{ ns.n }}",
        "{{ entry.tags|map(\"upper\")|join(\",\") }}",
        "{% for k, v in data.nav[0]|items %}{{ k }}={{ v }}{% endfor %}",
        "{{ entry[\"title\"] }} {{ collections[\"posts\"]|length }}",
        "{% set key = \"title\" %}{{ entry[key] }}",
        "{% for g in collections.posts|groupby(\"date\") %}{{ g.grouper }}:{{ g.list|length }} {% endfor %}",
        "{{ (collections.posts|first).title }}",
        "{{ entry.tags[0] if entry.tags else \"\" }}",
        "{% macro local(x) %}{{ x }}{% endmacro %}{{ local(entry.title) }}",
        "{{ entry.title ~ \" \" ~ entry.reading_time }}",
        "{% filter upper %}{{ entry.title }}{% endfilter %}",
        "{{ taxonomies.tags.terms|selectattr(\"count\", \"gt\", 1)|map(attribute=\"name\")|join(\" \") }}",
        "{% for i in range(entry.tags|length) %}{{ entry.tags[i] }}{% endfor %}",
        "{{ entry.word_count // 200 }} {{ entry.title|length > 3 }}",
        "{% with d = entry.updated|default(entry.date) %}{{ d|dateformat(format=\"%Y\") }}{% endwith %}",
        "{% for p in collections.posts|sort_by([\"-date\"]) %}{{ p.title }}{% break %}{% endfor %}",
        "{% set cls = [\"a\", \"b\" if entry.draft else \"\"]|select|join(\" \") %}{{ cls }}",
    ];
    for idiom in idioms {
        let mut written = site("eyuan");
        let (_, text) = written
            .iter_mut()
            .find(|(p, _)| p == "templates/posts/entry.html")
            .unwrap();
        *text = idiom.to_string();
        let (_, _, diagnostics) = render(&written);
        assert!(diagnostics.is_empty(), "{idiom}: {diagnostics:#?}");
    }
}
