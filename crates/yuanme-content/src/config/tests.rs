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
dir = "content/work"
route = "/projects/{slug}/"

[taxonomies.tags]
collections = ["posts"]
route = "/tags/{term}/"
lowercase = true
slugs = { "C++" = "cplusplus" }

[data.nav]
type = "list"
items = { type = "object", fields = { label = { type = "string" }, href = { type = "string" } } }
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

    let tags = &config.taxonomies["tags"];
    assert_eq!(tags.collections, ["posts"]);
    assert!(tags.lowercase && !tags.single);
    assert_eq!(tags.slugs["C++"], "cplusplus");

    let FieldType::List { items, .. } = &config.data["nav"] else {
        panic!("nav is a list");
    };
    assert!(matches!(**items, FieldType::Object(_)));
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
    ("config/wrong-type", "site = \"x\"\n", "\"x\""),
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
