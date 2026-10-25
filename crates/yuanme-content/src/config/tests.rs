use std::collections::BTreeSet;

use yuanme_diag::{Applicability, Diagnostic, FileId, Files, apply_fixes, codes};

use super::*;

/// The first pass alone, against `keys`.
fn check_table<'a>(
    files: &'a Files,
    file: FileId,
    keys: &[Key],
) -> (Option<DeTable<'a>>, Vec<Diagnostic>) {
    let mut checker = Checker::new(file);
    let mut table = parse_toml(files.text(file), &mut checker).expect("valid TOML");
    let keys: Vec<&Key> = keys.iter().collect();
    checker.table(&keys, &mut table, 0..0, "");
    let ok = !checker.failed();
    (ok.then_some(table), checker.diagnostics)
}

const MINIMAL: &str = "[site]\nurl = \"https://example.com\"\n";

fn check_with(text: &str, engine: &str) -> (Files, Option<Config>, Vec<Diagnostic>) {
    let mut files = Files::default();
    let file = files.add("yuanme.toml", text);
    let (config, diagnostics) = parse(&files, file, engine);
    (files, config, diagnostics)
}

fn check(text: &str) -> (Option<Config>, Vec<Diagnostic>) {
    let (_, config, diagnostics) = check_with(text, "1.4.2");
    (config, diagnostics)
}

/// The one error `text` must produce.
fn only_error(text: &str) -> Diagnostic {
    let (config, diagnostics) = check(text);
    assert!(config.is_none(), "an error must fail the config");
    let [d] = diagnostics.as_slice() else {
        panic!("expected one Diagnostic, got {diagnostics:#?}");
    };
    d.clone()
}

#[test]
fn a_full_config_reads_into_typed_values_with_defaults_filled_in() {
    let text = r##"#:schema ./.yuanme/schemas/yuanme.toml.json
yuanme = "1.2"

[site]
url = "https://eyuan.me"
title = "eyuan"
language = "en"

[site.extra]
github = "yuann3"
socials = [{ name = "x", url = "https://x.com" }]

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"
paginate = 10
sort = ["order", "-modified"]

[collections.posts.fields]
description = { type = "string", max = 250 }
order = { type = "int", min = 1, max = 5, optional = true }
kind = { type = "enum", values = ["essay", "note"], default = "essay" }
date = { type = "date" }
links = { type = "list", items = { type = "url" }, unique = true }
cover = { type = "object", fields = { src = { type = "image" }, alt = { type = "string", optional = true } } }

[collections.projects]
dir = "content/work/"
route = "/projects/{slug}/"
template = "layouts/project.html"

[taxonomies.tags]
collections = ["posts"]
route = "/tags/{term}/"
lowercase = true
slugs = { "C++" = "cplusplus" }

[routes.home]
path = "/"
template = "home.html"

[feeds.rss]
collections = ["posts", "pages"]
path = "/rss.xml"

[feeds.atom]
collections = ["projects"]
path = "/atom.xml"
template = "feeds/atom.xml"

[data.nav]
type = "list"
items = { type = "object", fields = { label = { type = "string" }, href = { type = "string" } } }

[math.macros]
R = '\mathbb{R}'
norm = '\left\lVert #1 \right\rVert'

[math.theorems.claim]
label = "Claim"
counter = "claim"

[math.theorems.remark]
counter = "theorem"

[math.theorems.definition]
label = "Def."
counter = false

[math.theorems.axiom]

[build]
targets = { chrome = "120", safari = "16.4.1" }

[host]
adapter = "netlify"

[[host.headers]]
path = "/fonts/*"
headers = { "Cache-Control" = "public, max-age=3600", "Access-Control-Allow-Origin" = "*" }

[[host.redirects]]
from = "/old-post/"
to = "/blog/new-post/"

[[host.redirects]]
from = "/talk"
to = "https://example.com/talk?id=1"
status = 302

[images]
widths = [1200, 640, 1200]
sizes = "(min-width: 768px) 720px, 100vw"
formats = ["webp", "avif"]
avif_speed = 4
quality = { jpeg = 90 }
"##;
    let (config, diagnostics) = check(text);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let config = config.unwrap();

    assert_eq!(config.site.url, "https://eyuan.me");
    assert_eq!(config.site.title.as_deref(), Some("eyuan"));
    assert_eq!(config.site.timezone, "UTC");
    assert_eq!(config.site.description, None);
    assert_eq!(
        config.site.extra["socials"].as_array().unwrap()[0]["name"].as_str(),
        Some("x")
    );

    let posts = &config.collections["posts"];
    assert_eq!(posts.dir, "content/posts");
    assert_eq!(posts.list_route.as_deref(), Some("/blog/"));
    assert_eq!(posts.paginate, Some(10));
    assert_eq!(
        posts.sort,
        [
            SortKey {
                field: "order".into(),
                descending: false
            },
            SortKey {
                field: "modified".into(),
                descending: true
            },
        ]
    );
    assert_eq!(
        posts.fields["description"].kind,
        FieldType::String {
            min: None,
            max: Some(250)
        }
    );
    let order = &posts.fields["order"];
    assert_eq!(
        order.kind,
        FieldType::Int {
            min: Some(1),
            max: Some(5)
        }
    );
    assert!(order.optional);
    let kind = &posts.fields["kind"];
    assert_eq!(
        kind.kind,
        FieldType::Enum(vec!["essay".into(), "note".into()])
    );
    assert_eq!(kind.default, Some(toml::Value::String("essay".into())));
    assert!(!posts.fields["date"].optional);
    assert_eq!(
        posts.fields["links"].kind,
        FieldType::List {
            items: Box::new(FieldType::Url),
            unique: true
        }
    );
    let FieldType::Object(cover) = &posts.fields["cover"].kind else {
        panic!("cover is an object");
    };
    assert_eq!(cover["src"].kind, FieldType::Image);
    assert!(cover["alt"].optional);

    let projects = &config.collections["projects"];
    assert_eq!(projects.dir, "content/work");
    assert_eq!(
        projects.sort,
        [SortKey {
            field: "date".into(),
            descending: true
        }]
    );

    assert_eq!(posts.template, None);
    assert_eq!(projects.template.as_deref(), Some("layouts/project.html"));
    assert_eq!(
        &text[projects.template_at.clone()],
        "\"layouts/project.html\""
    );

    let tags = &config.taxonomies["tags"];
    assert_eq!(tags.collections, ["posts"]);
    assert!(tags.lowercase && !tags.single);
    assert_eq!(tags.slugs["C++"], "cplusplus");
    assert_eq!(tags.template, None);

    let home = &config.routes["home"];
    assert_eq!(
        (home.path.as_str(), home.template.as_str()),
        ("/", "home.html")
    );
    let rss = &config.feeds["rss"];
    assert_eq!(rss.collections, ["posts", "pages"]);
    assert_eq!(rss.path, "/rss.xml");
    // The built-in feed Template, which a file in `templates/` can replace.
    assert_eq!(rss.template, "rss.xml");
    assert_eq!(config.feeds["atom"].template, "feeds/atom.xml");

    let FieldType::List { items, .. } = &config.data["nav"].kind else {
        panic!("nav is a list");
    };
    assert!(matches!(**items, FieldType::Object(_)));

    assert_eq!(
        config.math.macros,
        [
            ("R".to_string(), r"\mathbb{R}".to_string()),
            (
                "norm".to_string(),
                r"\left\lVert #1 \right\rVert".to_string()
            ),
        ]
    );
    // Kinds of the Author's own, and changes to the built-in ones, which
    // keep what isn't changed.
    let kind = |name: &str| {
        let k = &config.math.theorems[name];
        (k.label.as_str(), k.counter.as_deref())
    };
    assert_eq!(kind("claim"), ("Claim", Some("claim")));
    assert_eq!(kind("remark"), ("Remark", Some("theorem")));
    assert_eq!(kind("definition"), ("Def.", None));
    assert_eq!(kind("axiom"), ("Axiom", Some("theorem")));
    assert_eq!(kind("lemma"), ("Lemma", Some("theorem")));
    assert_eq!(kind("proof"), ("Proof", None));
    assert_eq!(config.math.theorems.len(), 13);

    // Only the browsers listed: one left out is not supported.
    let targets: Vec<_> = config.build.targets.iter().collect();
    assert_eq!(
        targets,
        [
            (&"chrome".to_string(), &[120, 0, 0]),
            (&"safari".to_string(), &[16, 4, 1])
        ]
    );

    let host = &config.host;
    assert_eq!(host.adapter, Adapter::Netlify);
    assert_eq!(&text[host.adapter_at.clone()], "\"netlify\"");
    let [fonts] = host.headers.as_slice() else {
        panic!("{:?}", host.headers)
    };
    assert_eq!(fonts.path, "/fonts/*");
    assert_eq!(&text[fonts.path_at.clone()], "\"/fonts/*\"");
    let mut headers = fonts.headers.clone();
    headers.sort();
    assert_eq!(
        headers,
        [
            ("Access-Control-Allow-Origin".into(), "*".into()),
            ("Cache-Control".into(), "public, max-age=3600".into())
        ]
    );
    let redirects: Vec<_> = host
        .redirects
        .iter()
        .map(|r| (r.from.as_str(), r.to.as_str(), r.status))
        .collect();
    // A redirect is permanent and keeps its method unless it says otherwise.
    assert_eq!(
        redirects,
        [
            ("/old-post/", "/blog/new-post/", 308),
            ("/talk", "https://example.com/talk?id=1", 302)
        ]
    );

    // Widths ascending and each once; formats in the order written.
    assert_eq!(
        config.images,
        Images {
            widths: vec![640, 1200],
            sizes: "(min-width: 768px) 720px, 100vw".into(),
            formats: vec!["webp".into(), "avif".into()],
            avif_speed: 4,
            avif_quality: 72,
            jpeg_quality: 90,
            webp_quality: 80,
        }
    );

    let (config, _) = check(MINIMAL);
    let config = config.unwrap();
    // #28's defaults: AVIF before a JPEG fallback, at each encoder's own
    // quality.
    assert_eq!(
        config.images,
        Images {
            widths: vec![480, 800, 1200, 1600, 2400],
            sizes: "100vw".into(),
            formats: vec!["avif".into()],
            avif_speed: 6,
            avif_quality: 72,
            jpeg_quality: 80,
            webp_quality: 80,
        }
    );
    // Workers static assets is the default host (#25).
    assert_eq!(config.host.adapter, Adapter::Cloudflare);
    assert!(config.host.headers.is_empty() && config.host.redirects.is_empty());
    let targets: Vec<_> = config.build.targets.into_iter().collect();
    assert_eq!(
        targets,
        [
            ("chrome".to_string(), [111, 0, 0]),
            ("firefox".to_string(), [128, 0, 0]),
            ("safari".to_string(), [16, 4, 0])
        ]
    );
}

#[test]
fn every_config_code_has_a_test_here() {
    let tested: BTreeSet<&str> = CASES
        .iter()
        .map(|(code, _, _)| *code)
        .chain(["config/engine-version", "config/deprecated"])
        .collect();
    let all: BTreeSet<&str> = codes::ALL
        .iter()
        .map(|c| c.name)
        .filter(|n| n.starts_with("config/"))
        .collect();
    assert_eq!(tested, all);
}

/// (code, config, the text its primary span must cover)
const CASES: &[(&str, &str, &str)] = &[
    (
        "config/syntax",
        "[site]\nurl = https://example.com\n",
        "https://example.com",
    ),
    ("config/syntax", "[site]\nurl = \"a\"\nurl = \"b\"\n", "url"),
    // The key itself, so the fix replaces only the typo.
    (
        "config/unknown-key",
        "[site]\nurl = \"https://e.com\"\ntitel = \"x\"\n",
        "titel",
    ),
    (
        "config/unknown-key",
        "[sites]\nurl = \"https://e.com\"\n",
        "sites",
    ),
    (
        "config/unknown-key",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nfields = { a = { type = \"string\", maximum = 3 } }\n",
        "maximum",
    ),
    // A key `int` takes, on a `bool`: what a field allows depends on its type.
    (
        "config/unknown-key",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nfields = { a = { type = \"bool\", min = 1 } }\n",
        "min",
    ),
    // List items and Data files can't be optional.
    (
        "config/unknown-key",
        "[site]\nurl = \"https://e.com\"\n[data.nav]\ntype = \"string\"\noptional = true\n",
        "optional",
    ),
    ("config/missing-key", "[site]\ntitle = \"x\"\n", "site"),
    ("config/missing-key", "#:schema ./x.json\n", ""),
    (
        "config/missing-key",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\ndir = \"x\"\n",
        "p",
    ),
    (
        "config/missing-key",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nfields = { a = { max = 3 } }\n",
        "a",
    ),
    ("config/wrong-type", "[site]\nurl = 3\n", "3"),
    // A number would read 16.10 as 16.1.
    (
        "config/wrong-type",
        "[site]\nurl = \"https://e.com\"\n[build]\ntargets = { safari = 16.4 }\n",
        "16.4",
    ),
    (
        "config/unknown-key",
        "[site]\nurl = \"https://e.com\"\n[build]\ntargets = { chorme = \"111\" }\n",
        "chorme",
    ),
    ("config/wrong-type", "site = \"x\"\n", "\"x\""),
    // A kind is numbered on a named counter or not at all.
    (
        "config/wrong-type",
        "[site]\nurl = \"https://e.com\"\n[math.theorems.claim]\ncounter = true\n",
        "true",
    ),
    (
        "config/wrong-type",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nsort = [\"a\", 2]\n",
        "2",
    ),
    (
        "config/out-of-range",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nlist_route = \"/p/\"\npaginate = 0\n",
        "0",
    ),
    (
        "config/out-of-range",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nfields = { a = { type = \"string\", max = -1 } }\n",
        "-1",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"example.com\"\n",
        "\"example.com\"",
    ),
    // A macro with a name LaTeX can't call, and one using a command
    // nothing defines.
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[math.macros]\nR = '\\mathbb{R}'\n\"my R\" = '\\mathbf{R}'\n",
        "'\\mathbf{R}'",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[math.macros]\nnorm = '\\lVert #1 \\rVert'\nN = '\\norm{\\Nat}'\n",
        "'\\norm{\\Nat}'",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/blog/\"\n",
        "\"/blog/\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"blog/{slug}/\"\n",
        "\"blog/{slug}/\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[routes.home]\npath = \"{slug}\"\ntemplate = \"home.html\"\n",
        "\"{slug}\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.posts]\nroute = \"/{slug}/\"\n[feeds.rss]\ncollections = [\"psts\"]\npath = \"/rss.xml\"\n",
        "\"psts\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{year}/{slug}/\"\n",
        "\"/{year}/{slug}/\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[taxonomies.tags]\ncollections = []\nroute = \"/tags/\"\n",
        "\"/tags/\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nfields = { a = { type = \"strnig\" } }\n",
        "\"strnig\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nsort = [\"-dat\"]\n",
        "\"-dat\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nsort = [\"-\"]\n",
        "\"-\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.posts]\nroute = \"/{slug}/\"\n[taxonomies.tags]\ncollections = [\"post\"]\nroute = \"/tags/{term}/\"\n",
        "\"post\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nfields = { word_count = { type = \"int\" } }\n",
        "word_count",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nfields = { content = { type = \"string\" } }\n",
        "content",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nfields = { title = { type = \"string\" } }\n",
        "title",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nfields = { date = { type = \"string\" } }\n",
        "\"string\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nfields = { tags = { type = \"string\" } }\n[taxonomies.tags]\ncollections = [\"p\"]\nroute = \"/tags/{term}/\"\n",
        "tags",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nfields = { a = { type = \"int\", min = 5, max = 1 } }\n",
        "5",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nfields = { a = { type = \"enum\", values = [] } }\n",
        "[]",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nfields = { a = { type = \"enum\", values = [\"x\"], default = \"y\" } }\n",
        "\"y\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nfields = { a = { type = \"int\", max = 3, default = 4 } }\n",
        "4",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nfields = { a = { type = \"list\", items = { type = \"int\" }, default = [1, \"2\"] } }\n",
        "\"2\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\npaginate = 5\n",
        "5",
    ),
    (
        "config/invalid-value",
        "yuanme = \"1\"\n[site]\nurl = \"https://e.com\"\n",
        "\"1\"",
    ),
    // No browserslist queries (#20): their results change with caniuse data.
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[build]\ntargets = { chrome = \">= 111\" }\n",
        "\">= 111\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[build]\ntargets = { safari = \"16.256\" }\n",
        "\"16.256\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\ntimezone = \"Asia/Singapur\"\n",
        "\"Asia/Singapur\"",
    ),
    // A list has no order to sort by.
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[collections.p]\nroute = \"/{slug}/\"\nsort = [\"links\"]\nfields = { links = { type = \"list\", items = { type = \"url\" } } }\n",
        "\"links\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[taxonomies.tags]\ncollections = []\nroute = \"/tags/{term}/\"\nslugs = { \"C++\" = \"C Plus Plus\" }\n",
        "\"C Plus Plus\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[host]\nadapter = \"netlfy\"\n",
        "\"netlfy\"",
    ),
    (
        "config/wrong-type",
        "[site]\nurl = \"https://e.com\"\n[host]\nheaders = { path = \"/\" }\n",
        "{ path = \"/\" }",
    ),
    (
        "config/missing-key",
        "[site]\nurl = \"https://e.com\"\n[[host.redirects]]\nto = \"/b/\"\n",
        "redirects",
    ),
    (
        "config/unknown-key",
        "[site]\nurl = \"https://e.com\"\n[[host.redirects]]\nfrom = \"/a/\"\nto = \"/b/\"\ncode = 301\n",
        "code",
    ),
    // No host serves the files but Caddy and nginx.
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[host]\nprecompress = true\n",
        "true",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[[host.headers]]\npath = \"/*/a/*\"\nheaders = { X-A = \"1\" }\n",
        "\"/*/a/*\"",
    ),
    // `:name` is a placeholder on Cloudflare and Netlify, and text on Vercel.
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[[host.headers]]\npath = \"/:lang/\"\nheaders = { X-A = \"1\" }\n",
        "\"/:lang/\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[[host.headers]]\npath = \"/\"\nheaders = { \"X A\" = \"1\" }\n",
        "\"X A\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[[host.headers]]\npath = \"/\"\nheaders = { X-A = \"1\\nX-B: 2\" }\n",
        "\"1\\nX-B: 2\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[[host.headers]]\npath = \"/\"\nheaders = {}\n",
        "{}",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[[host.redirects]]\nfrom = \"/old/*\"\nto = \"/new/\"\n",
        "\"/old/*\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[[host.redirects]]\nfrom = \"/a/\"\nto = \"b/\"\n",
        "\"b/\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[[host.redirects]]\nfrom = \"/a/\"\nto = \"/a/\"\n",
        "\"/a/\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[[host.redirects]]\nfrom = \"/a/\"\nto = \"/b/\"\n[[host.redirects]]\nfrom = \"/a/\"\nto = \"/c/\"\n",
        "\"/a/\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[[host.redirects]]\nfrom = \"/a/\"\nto = \"/b/\"\nstatus = 303\n",
        "303",
    ),
    // ravif panics outside these ranges.
    (
        "config/out-of-range",
        "[site]\nurl = \"https://e.com\"\n[images]\navif_speed = 11\n",
        "11",
    ),
    (
        "config/out-of-range",
        "[site]\nurl = \"https://e.com\"\n[images]\nquality = { avif = 0 }\n",
        "0",
    ),
    (
        "config/out-of-range",
        "[site]\nurl = \"https://e.com\"\n[images]\nwidths = [480, 0]\n",
        "0",
    ),
    (
        "config/wrong-type",
        "[site]\nurl = \"https://e.com\"\n[images]\nwidths = 480\n",
        "480",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[images]\nwidths = []\n",
        "[]",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[images]\nformats = [\"jpeg\"]\n",
        "\"jpeg\"",
    ),
    (
        "config/invalid-value",
        "[site]\nurl = \"https://e.com\"\n[images]\nformats = [\"avif\", \"avif\"]\n",
        "\"avif\"",
    ),
];

#[test]
fn each_mistake_is_one_diagnostic_with_its_code_on_the_right_text() {
    let wrong: Vec<String> = CASES
        .iter()
        .filter_map(|(code, text, under)| {
            let (config, diagnostics) = check(text);
            let got: Vec<_> = diagnostics
                .iter()
                .map(|d| {
                    (
                        d.code.name,
                        &text[d.primary().range.clone()],
                        d.message.as_str(),
                    )
                })
                .collect();
            let ok = config.is_none() && got.len() == 1 && (got[0].0, got[0].1) == (*code, *under);
            (!ok).then(|| format!("{text}  want {code} on {under:?}\n  got {got:?}\n"))
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn every_mistake_in_a_file_is_reported_in_file_order() {
    let text = "[site]\nurl = 3\ntitel = \"x\"\n[collections.p]\nroute = \"/blog/\"\n";
    let (config, diagnostics) = check(text);
    assert!(config.is_none());
    let codes: Vec<_> = diagnostics.iter().map(|d| d.code.name).collect();
    assert_eq!(
        codes,
        [
            "config/wrong-type",
            "config/unknown-key",
            "config/invalid-value"
        ]
    );
}

#[test]
fn a_typo_suggests_the_key_it_meant_and_the_fix_applies() {
    let text = "[site]\nurl = \"https://e.com\"\ntitel = \"x\"\n";
    let (_, diagnostics) = check(text);
    let fix = &diagnostics[0].children[0].suggestions[0];
    // A guess at what the Author meant: a tool may not apply it unasked.
    assert_eq!(fix.applicability, Applicability::MaybeIncorrect);
    assert_eq!(fix.edits[0].replacement, "title");

    // apply_fixes only takes machine-applicable edits, so apply this one by hand.
    let range = fix.edits[0].span.range.clone();
    let fixed = format!("{}title{}", &text[..range.start], &text[range.end..]);
    assert!(check(&fixed).1.is_empty());

    // Nothing near enough: the valid keys are listed instead of a guess.
    let d = only_error("[site]\nurl = \"https://e.com\"\nzzzzzz = 1\n");
    assert!(d.children.iter().all(|c| c.suggestions.is_empty()));
    let note = &d.children[0].message;
    assert!(
        note.contains("`title`") && note.contains("`extra`"),
        "{note}"
    );
}

#[test]
fn rendered_unknown_key() {
    let text = "[site]\nurl = \"https://e.com\"\ntitel = \"x\"\n";
    let (files, _, diagnostics) = check_with(text, "1.4.2");
    insta::assert_snapshot!(diagnostics[0].render(&files, false));
}

#[test]
fn the_engine_version_check_uses_caret_meaning() {
    let config = |req: &str| format!("yuanme = \"{req}\"\n{MINIMAL}");
    let ok = |req: &str, engine: &str| check_with(&config(req), engine).2.is_empty();
    // Same major, same or newer minor: builds.
    assert!(ok("1.2", "1.2.0"));
    assert!(ok("1.2", "1.9.3"));
    // An older Engine, or another major: stops.
    assert!(!ok("1.2", "1.1.9"));
    assert!(!ok("1.2", "2.0.0"));
    assert!(!ok("1.2", "2.5.0"));
    // Before 1.0 a minor is breaking, so 0.3 accepts only 0.3.x.
    assert!(ok("0.3", "0.3.7"));
    assert!(!ok("0.3", "0.4.0"));
    assert!(!ok("0.3", "0.2.9"));
    // A git build of the Engine carries the commit after `+`.
    assert!(ok("0.0", "0.0.0+403e56e.dirty"));

    // A mismatch is the only thing reported: the rest of the file was written
    // for another Engine, so its errors would be noise.
    let text = "yuanme = \"2.0\"\n[site]\ntitel = 1\n";
    let (_, config, diagnostics) = check_with(text, "1.4.2");
    assert!(config.is_none());
    let [d] = diagnostics.as_slice() else {
        panic!("{diagnostics:#?}")
    };
    assert_eq!(d.code.name, "config/engine-version");
    assert!(
        d.message.contains("2.0") && d.message.contains("1.4.2"),
        "{}",
        d.message
    );
}

/// No key is deprecated yet, so this checks the mechanism with a schema that
/// renames `name` to `title`.
#[test]
fn a_deprecated_key_warns_with_a_fix_and_still_works() {
    const KEYS: &[Key] = &[
        Key::required("title", Shape::String, "The title."),
        Key::renamed("name", "title"),
    ];
    let text = "name = \"x\"\n";
    let mut files = Files::default();
    let file = files.add("yuanme.toml", text);
    let (table, diagnostics) = check_table(&files, file, KEYS);
    let [d] = diagnostics.as_slice() else {
        panic!("{diagnostics:#?}")
    };
    assert_eq!(d.code.name, "config/deprecated");
    assert_eq!(d.level, yuanme_diag::Level::Warning);
    // The old key reads as the new one, so a required key is satisfied.
    assert_eq!(table.unwrap()["title"].get_ref().as_str(), Some("x"));

    let fixed = apply_fixes(&files, file, &diagnostics).unwrap();
    assert_eq!(fixed, "title = \"x\"\n");
    let mut files = Files::default();
    let file = files.add("yuanme.toml", fixed);
    assert!(check_table(&files, file, KEYS).1.is_empty());

    // Both names at once is a mistake, not a rename.
    let mut files = Files::default();
    let file = files.add("yuanme.toml", "name = \"x\"\ntitle = \"y\"\n");
    let codes: Vec<_> = check_table(&files, file, KEYS)
        .1
        .iter()
        .map(|d| d.code.name)
        .collect();
    assert_eq!(codes, ["config/invalid-value"]);
}

#[test]
fn json_schema() {
    insta::assert_snapshot!(serde_json::to_string_pretty(&super::json_schema()).unwrap());
}
