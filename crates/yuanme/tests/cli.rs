//! The CLI as an agent drives it (#21): every command's `--json` events,
//! each validated against `yuanme schema events`, its exit codes, and the
//! files it writes under `.yuanme/`.

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

const SITE: &str = r#"=== yuanme.toml
[site]
url = "https://example.com"
title = "Example"
timezone = "Asia/Singapore"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"

[collections.posts.fields]
description = { type = "string" }
kind = { type = "enum", values = ["essay", "note"] }
hero = { type = "image", optional = true }

[taxonomies.tags]
collections = ["posts"]
route = "/tags/{term}/"

[data.nav]
type = "list"
items = { type = "object", fields = { name = { type = "string" }, href = { type = "string" } } }

[routes.home]
path = "/"
template = "home.html"

[feeds.rss]
collections = ["posts"]
path = "/rss.xml"
=== content/posts/first.md
---
title: First
date: 2025-03-01
description: The first one.
kind: essay
tags: [Rust]
---
Hello.
=== content/posts/wip.md
---
title: Work in progress
date: 2025-04-01
description: Not yet.
kind: note
draft: true
---
=== data/nav.yaml
- name: Blog
  href: /blog/
=== static/robots.txt
User-agent: *
=== templates/home.html
<h1>{{ site.title }}</h1>{% for item in data.nav %}<a href="{{ item.href }}">{{ item.name }}</a>{% endfor %}
=== templates/posts/entry.html
<h1>{{ entry.title }}</h1>{{ entry.content }}
=== templates/posts/list.html
{% for e in entries %}<a href="{{ e.url }}">{{ e.title }}</a>{% endfor %}
=== templates/tags/term.html
<h1>{{ term.name }}</h1>
=== templates/tags/index.html
{% for t in terms %}{{ t.name }}{% endfor %}
"#;

fn project(files: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), files);
    dir
}

/// Files written as `=== path` lines, each followed by its text.
fn write(root: &Path, files: &str) {
    let mut current: Option<(String, String)> = None;
    let flush = |file: Option<(String, String)>| {
        if let Some((path, text)) = file {
            let path = root.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
    };
    for line in files.split_inclusive('\n') {
        match line.strip_prefix("=== ") {
            Some(path) => flush(current.replace((path.trim_end().into(), String::new()))),
            None => current.as_mut().unwrap().1.push_str(line),
        }
    }
    flush(current);
}

struct Output {
    code: i32,
    stdout: String,
    stderr: String,
}

fn yuanme(root: &Path, args: &[&str]) -> Output {
    yuanme_with(root, args, &[])
}

/// With the Managed tool overrides in `env` set, and the others unset.
fn yuanme_with(root: &Path, args: &[&str], env: &[(&str, &Path)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_yuanme"));
    command
        .args(args)
        .current_dir(root)
        .env_remove("YUANME_TAILWIND_BIN")
        .env_remove("YUANME_TSC_BIN")
        .envs(env.iter().copied());
    let out = command.output().unwrap();
    Output {
        code: out.status.code().unwrap(),
        stdout: String::from_utf8(out.stdout).unwrap(),
        stderr: String::from_utf8(out.stderr).unwrap(),
    }
}

/// The events schema with every object closed, so an emitted field the
/// schema doesn't declare fails here. The published schema stays open,
/// because a later release may add fields (ADR 0015).
fn strict(schema: &mut Value) {
    match schema {
        Value::Object(map) => {
            if map.contains_key("properties") && !map.contains_key("additionalProperties") {
                map.insert("additionalProperties".into(), false.into());
            }
            map.values_mut().for_each(strict);
        }
        Value::Array(items) => items.iter_mut().for_each(strict),
        _ => {}
    }
}

/// Runs with `--json` and returns the exit code and every event, each
/// validated against `yuanme schema events`. Nothing but events may reach
/// stdout, and no human text reaches stderr.
fn json(root: &Path, args: &[&str]) -> (i32, Vec<Value>) {
    let mut schema = serde_json::from_str(&yuanme(root, &["schema", "events"]).stdout).unwrap();
    strict(&mut schema);
    let validator = jsonschema::validator_for(&schema).unwrap();
    let out = yuanme(root, &[args, &["--json"]].concat());
    assert_eq!(out.stderr, "", "{args:?} wrote human text with --json");
    let events = out
        .stdout
        .lines()
        .map(|line| {
            let event: Value = serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("{args:?}: not JSON ({e}): {line}"));
            let errors: Vec<String> = validator
                .iter_errors(&event)
                .map(|e| format!("{e} at {}", e.instance_path()))
                .collect();
            assert!(errors.is_empty(), "{args:?}: {line}\n{errors:#?}");
            event
        })
        .collect();
    (out.code, events)
}

/// Events as snapshot text, one per line, with times zeroed.
fn show(events: &[Value]) -> String {
    let mut out = String::new();
    for event in events {
        let mut event = event.clone();
        if event.get("duration_ms").is_some() {
            event["duration_ms"] = 0.into();
        }
        out.push_str(&event.to_string());
        out.push('\n');
    }
    out
}

fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(
                    path.strip_prefix(dir)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/"),
                );
            }
        }
    }
    out.sort();
    out
}

#[test]
fn build_writes_the_site_and_its_yuanme_files() {
    let dir = project(SITE);
    let root = dir.path();
    let (code, events) = json(root, &["build"]);
    assert_eq!(code, 0);
    insta::assert_snapshot!(show(&events));
    // Every file is a URL but `_headers`, which only the host reads.
    assert_eq!(
        files_under(&root.join("dist")).len(),
        events[1]["changed_urls"].as_array().unwrap().len() + 1
    );
    assert!(
        !root.join("dist/blog/wip").exists(),
        "a Draft isn't published"
    );

    // Every schema the Site makes, and the docs for this Engine.
    assert_eq!(
        files_under(&root.join(".yuanme/schemas")),
        [
            "context/entry.json",
            "context/feed.json",
            "context/index.json",
            "context/list.json",
            "context/template.json",
            "context/term.json",
            "data/nav.json",
            "entry/pages.json",
            "entry/posts.json",
            "yuanme.toml.json",
        ]
    );
    let docs = files_under(&root.join(".yuanme/docs"));
    assert!(docs.contains(&"config/syntax.md".to_string()));
    assert!(docs.contains(&"VERSION".to_string()));

    let report: Value =
        serde_json::from_str(&fs::read_to_string(root.join(".yuanme/report.json")).unwrap())
            .unwrap();
    let pages: Vec<&str> = report["pages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["url"].as_str().unwrap())
        .collect();
    assert_eq!(pages.len(), 6, "{pages:?}");
    assert!(pages.contains(&"/blog/first/"));
    // And the default adapter's `_headers`.
    assert_eq!(report["written"], 10);
    let md = fs::read_to_string(root.join(".yuanme/report.md")).unwrap();
    assert!(md.contains("| `/blog/first/` |"), "{md}");

    // Nothing changed, so nothing is written; a new title writes its Page,
    // and the list and feed that show it.
    let (_, events) = json(root, &["build"]);
    assert_eq!(events[1]["changed_urls"], serde_json::json!([]));
    let first = root.join("content/posts/first.md");
    let edited = fs::read_to_string(&first)
        .unwrap()
        .replace("title: First", "title: Firsts");
    fs::write(&first, edited).unwrap();
    let (_, events) = json(root, &["build"]);
    assert_eq!(
        events[1]["changed_urls"],
        serde_json::json!(["/blog/", "/blog/first/", "/rss.xml"])
    );

    // --drafts publishes the Draft; --clean removes a file yuanme didn't write.
    fs::write(root.join("dist/mine.txt"), "mine").unwrap();
    let (_, events) = json(root, &["build", "--drafts"]);
    assert!(
        events[1]["changed_urls"]
            .as_array()
            .unwrap()
            .contains(&"/blog/wip/".into())
    );
    assert!(root.join("dist/mine.txt").exists());
    json(root, &["build", "--clean"]);
    assert!(!root.join("dist/mine.txt").exists());
    assert!(!root.join("dist/blog/wip").exists());

    // --out, from the project root.
    let (code, _) = json(root, &["build", "--out", "public"]);
    assert_eq!(code, 0);
    assert!(root.join("public/blog/first/index.html").is_file());

    // Without it, the adapter's: the Build Output API's for vercel, whose
    // URLs aren't under `static/`.
    let config = fs::read_to_string(root.join("yuanme.toml")).unwrap();
    fs::write(
        root.join("yuanme.toml"),
        config + "\n[host]\nadapter = \"vercel\"\n",
    )
    .unwrap();
    let (code, events) = json(root, &["build"]);
    assert_eq!(code, 0);
    assert!(root.join(".vercel/output/config.json").is_file());
    assert!(
        root.join(".vercel/output/static/blog/first/index.html")
            .is_file()
    );
    let changed = events[1]["changed_urls"].as_array().unwrap();
    assert!(changed.contains(&"/blog/first/".into()), "{changed:?}");
    assert_eq!(
        changed.len() + 1,
        files_under(&root.join(".vercel/output")).len()
    );
}

#[test]
fn the_report_lists_what_went_into_each_script_bundle() {
    let dir = project(
        r#"=== yuanme.toml
[site]
url = "https://example.com"

[routes.home]
path = "/"
template = "home.html"
=== templates/home.html
<head><script type="module" src="/scripts/site.ts"></script></head>
=== scripts/site.ts
import { greet } from "./greet";
greet();
=== scripts/greet.ts
export const greet = (): void => console.log("hi");
"#,
    );
    let root = dir.path();
    assert_eq!(json(root, &["build"]).0, 0);
    let report: Value =
        serde_json::from_str(&fs::read_to_string(root.join(".yuanme/report.json")).unwrap())
            .unwrap();
    let [bundle] = report["scripts"].as_array().unwrap().as_slice() else {
        panic!("{report:#}")
    };
    let file = bundle["file"].as_str().unwrap();
    let written = fs::read(root.join("dist").join(file)).unwrap();
    assert_eq!(bundle["bytes"], written.len());
    let modules: Vec<&str> = bundle["modules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["module"].as_str().unwrap())
        .collect();
    assert_eq!(modules, ["scripts/greet.ts", "scripts/site.ts"]);
    let md = fs::read_to_string(root.join(".yuanme/report.md")).unwrap();
    assert!(md.contains(&format!("`{file}`")), "{md}");
    assert!(md.contains("| `scripts/greet.ts` |"), "{md}");
}

#[test]
fn check_reports_what_build_would_and_writes_no_site() {
    let dir = project(SITE);
    let root = dir.path();
    let (code, events) = json(root, &["check"]);
    assert_eq!((code, events.len()), (0, 2));
    assert_eq!(events[1]["pages"], 6);
    assert!(!root.join("dist").exists());
    assert!(root.join(".yuanme/schemas/entry/posts.json").is_file());

    // A Template error, and an Org keyword with no effect, which warns.
    write(
        root,
        "=== templates/posts/list.html\n{{ entries | lenght }}\n=== content/posts/org.org\n#+TITLE: Org\n#+DESCRIPTION: x\n#+KIND: note\n#+AUTHOR: Yuan\n",
    );
    let (code, events) = json(root, &["check"]);
    assert_eq!(code, 1);
    insta::assert_snapshot!(show(&events));
    let build = yuanme(root, &["build"]);
    assert_eq!(build.code, 1);
    assert!(!root.join("dist").exists(), "a failed build writes nothing");
    // Without a terminal: one line per Diagnostic, and a summary.
    let check = yuanme(root, &["check"]);
    assert_eq!((check.code, check.stdout.as_str()), (1, ""));
    let (summary, ms) = check.stderr.rsplit_once(" in ").unwrap();
    assert!(ms.ends_with(" ms\n"), "{ms}");
    insta::assert_snapshot!(summary);

    // The warning alone passes, unless --strict makes it an error.
    write(
        root,
        "=== templates/posts/list.html\n{{ entries | length }}\n",
    );
    assert_eq!(json(root, &["check"]).0, 0);
    let (code, events) = json(root, &["check", "--strict"]);
    assert_eq!(code, 1);
    assert_eq!(events[1]["level"], "error");
    assert_eq!(events[1]["code"], "org/no-effect");
    assert_eq!(yuanme(root, &["build", "--strict", "-q"]).code, 1);
    assert!(!root.join("dist").exists());

    // `wrangler.jsonc` must serve the output directory.
    write(
        root,
        "=== content/posts/org.org\n#+TITLE: Org\n#+DESCRIPTION: x\n#+KIND: note\n=== wrangler.jsonc\n{ \"assets\": { \"directory\": \"./public\" } }\n",
    );
    let (code, events) = json(root, &["check"]);
    assert_eq!(code, 0);
    assert_eq!(events[1]["code"], "host/wrangler-mismatch");
    assert_eq!(events.len(), 3, "{}", show(&events));
    write(
        root,
        "=== wrangler.jsonc\n// The Worker\n{ \"assets\": { \"directory\": \"./dist\" } }\n",
    );
    assert_eq!(json(root, &["check"]).1.len(), 2);
}

#[test]
fn routes_lists_every_route_and_what_produces_it() {
    let dir = project(SITE);
    let root = dir.path();
    let (code, events) = json(root, &["routes"]);
    assert_eq!(code, 0);
    insta::assert_snapshot!(show(&events));
    let human = yuanme(root, &["routes"]);
    assert_eq!((human.code, human.stderr.as_str()), (0, ""));
    insta::assert_snapshot!(human.stdout);

    let (_, drafts) = json(root, &["routes", "--drafts"]);
    assert_eq!(drafts.len(), events.len() + 1);
    assert!(drafts.iter().any(|r| r["url"] == "/blog/wip/"));

    // A broken Template doesn't hide the Routes; an Entry error does.
    write(root, "=== templates/home.html\n{{ site.titel }}\n");
    assert_eq!(json(root, &["routes"]), (0, events));
    write(root, "=== content/posts/bad.md\n---\ntitle: Bad\n---\n");
    let (code, events) = json(root, &["routes"]);
    assert_eq!(code, 1);
    assert!(events.iter().all(|e| e["type"] == "diagnostic"));
    assert_eq!(events[0]["code"], "entry/missing-field");
}

#[test]
fn new_creates_an_entry_that_checks() {
    let dir = project(SITE);
    let root = dir.path();
    let today = jiff::Zoned::now()
        .with_time_zone(jiff::tz::TimeZone::get("Asia/Singapore").unwrap())
        .date()
        .to_string();
    let (code, events) = json(root, &["new", "posts", "Hello: World, again"]);
    assert_eq!(code, 0);
    insta::assert_snapshot!(show(&events));
    let text = fs::read_to_string(root.join("content/posts/hello-world-again.md")).unwrap();
    insta::assert_snapshot!(text.replace(&today, "TODAY"));

    let human = yuanme(root, &["new", "posts", "Notes on Org", "--format", "org"]);
    assert_eq!(human.code, 0);
    assert_eq!(
        human.stdout,
        "content/posts/notes-on-org.org\n/blog/notes-on-org/\n"
    );
    let org = fs::read_to_string(root.join("content/posts/notes-on-org.org")).unwrap();
    assert!(org.starts_with("#+TITLE: Notes on Org\n#+DATE: ["), "{org}");

    // Both are valid Entries, published at the URLs `new` printed.
    let (code, _) = json(root, &["check"]);
    assert_eq!(code, 0);
    let (_, routes) = json(root, &["routes"]);
    for url in ["/blog/hello-world-again/", "/blog/notes-on-org/"] {
        assert!(routes.iter().any(|r| r["url"] == url), "{url}");
    }

    let again = yuanme(root, &["new", "posts", "Hello: World, again"]);
    assert_eq!(again.code, 2);
    assert_eq!(
        again.stderr,
        "error: content/posts/hello-world-again.md already exists\n"
    );
    let typo = yuanme(root, &["new", "post", "X"]);
    assert_eq!(typo.code, 2);
    assert_eq!(
        typo.stderr,
        "error: there is no Collection `post`; did you mean `posts`?\n"
    );
}

#[test]
fn schema_prints_each_schema() {
    let dir = project(SITE);
    let root = dir.path();
    json(root, &["check"]);
    for (args, name) in [
        (&["schema", "config"][..], "config"),
        (&["schema", "events"], "events"),
        (&["schema", "entry", "posts"], "entry/posts"),
        (&["schema", "data", "nav"], "data/nav"),
        (&["schema", "context", "entry"], "context/entry"),
    ] {
        let (code, events) = json(root, args);
        assert_eq!((code, events.len()), (0, 1), "{args:?}");
        assert_eq!(events[0]["name"], name);
        // Without --json, the schema alone, which is what .yuanme/schemas/ holds.
        let bare: Value = serde_json::from_str(&yuanme(root, args).stdout).unwrap();
        assert_eq!(bare, events[0]["schema"], "{args:?}");
        jsonschema::meta::validate(&bare).unwrap();
        if name.starts_with("entry/") {
            let file = root.join(".yuanme/schemas").join(format!("{name}.json"));
            assert_eq!(
                bare,
                serde_json::from_str::<Value>(&fs::read_to_string(file).unwrap()).unwrap()
            );
        }
    }
    insta::assert_snapshot!(yuanme(root, &["schema", "entry", "posts"]).stdout);

    for (args, error) in [
        (
            &["schema", "entry", "post"][..],
            "there is no Collection `post`; did you mean `posts`?",
        ),
        (
            &["schema", "data", "social"],
            "there is no Data file with a schema `social`; the Site has nav",
        ),
        (
            &["schema", "context", "page"],
            "there is no kind of Route `page`; the Site has entry, feed, index, list, template, term",
        ),
    ] {
        let out = yuanme(root, args);
        assert_eq!(
            (out.code, out.stderr.trim_end()),
            (2, format!("error: {error}").as_str())
        );
    }
}

#[test]
fn docs_prints_pages_by_code() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let index = yuanme(root, &["docs"]);
    assert_eq!(index.code, 0);
    assert!(
        index.stdout.contains("- `template/unknown-field`: "),
        "{}",
        index.stdout
    );
    let (code, events) = json(root, &["docs", "config/syntax"]);
    assert_eq!(code, 0);
    insta::assert_snapshot!(events[0]["markdown"].as_str().unwrap());
    let typo = yuanme(root, &["docs", "config/syntx"]);
    assert_eq!(typo.code, 2);
    assert_eq!(
        typo.stderr,
        "error: there is no docs page `config/syntx`; did you mean `config/syntax`?\n"
    );
}

#[test]
fn exit_codes_and_the_project_folder() {
    let dir = project(SITE);
    let root = dir.path();
    assert_eq!(yuanme(root, &["build", "--frob"]).code, 2);
    assert_eq!(yuanme(root, &["frob"]).code, 2);
    assert_eq!(yuanme(root, &["--help"]).code, 0);

    // -C runs in another folder, as if started there.
    let elsewhere = tempfile::tempdir().unwrap();
    let missing = yuanme(elsewhere.path(), &["check"]);
    assert_eq!(missing.code, 2);
    assert!(
        missing.stderr.starts_with("error: no yuanme.toml in "),
        "{}",
        missing.stderr
    );
    let path = root.to_str().unwrap();
    let (code, events) = json(elsewhere.path(), &["-C", path, "check"]);
    assert_eq!((code, events.len()), (0, 2));
    assert_eq!(yuanme(root, &["build", "--out", "."]).code, 2);
}

/// `check`, and the commands that build only to read the Site, encode no
/// image: that is `build`'s work, and the slowest part of it.
#[test]
fn only_build_encodes_images() {
    let dir = project(
        "=== yuanme.toml
[site]
url = \"https://example.com\"

[collections.posts]
route = \"/blog/{slug}/\"

[images]
widths = [32]
=== templates/posts/entry.html
{{ entry.content }}
=== content/posts/sea/index.md
---
title: Sea
---
![A beach](beach.jpg)
",
    );
    let root = dir.path();
    let photo = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../yuanme-assets/fixtures/images/beach.jpg"
    );
    fs::copy(photo, root.join("content/posts/sea/beach.jpg")).unwrap();
    let encoded = || match fs::read_dir(root.join(".yuanme/cache/large")) {
        Ok(entries) => entries.count(),
        Err(_) => 0,
    };

    for command in [&["check"][..], &["routes"], &["schema", "entry", "posts"]] {
        let run = yuanme(root, command);
        assert_eq!(run.code, 0, "{command:?}: {}", run.stderr);
        assert_eq!(encoded(), 0, "{command:?} encoded an image");
    }
    assert_eq!(yuanme(root, &["build", "-q"]).code, 0);
    // An AVIF and a JPEG, 32 wide.
    assert_eq!(encoded(), 2);
    let page = fs::read_to_string(root.join("dist/blog/sea/index.html")).unwrap();
    assert!(page.contains("<picture>"), "{page}");
}

#[test]
fn budgets_warn_unless_strict_and_fill_the_report() {
    let site = r#"=== yuanme.toml
[site]
url = "https://example.com"

[routes.home]
path = "/"
template = "home.html"

[routes.about]
path = "/about/"
template = "about.html"

[budgets]
js = 0.01

[budgets.overages."/about/"]
js = 1
=== templates/home.html
<script>console.log("a script of more than ten bytes")</script>
=== templates/about.html
<script>console.log("a script of more than ten bytes, acknowledged")</script>
"#;
    let dir = project(site);
    let root = dir.path();
    let (code, events) = json(root, &["build"]);
    assert_eq!(code, 0, "a breach is a warning");
    insta::assert_snapshot!(show(&events));
    let budget = |events: &[Value]| -> Vec<(String, bool)> {
        events
            .iter()
            .filter(|e| e["type"] == "budget")
            .map(|e| {
                (
                    e["page"].as_str().unwrap().into(),
                    e["acknowledged"] == true,
                )
            })
            .collect()
    };
    // In the order of the Site's Routes, by name.
    assert_eq!(
        budget(&events),
        [("/about/".to_string(), true), ("/".to_string(), false)]
    );

    let report = || -> Value {
        serde_json::from_str(&fs::read_to_string(root.join(".yuanme/report.json")).unwrap())
            .unwrap()
    };
    let budgets = &report()["budgets"];
    assert_eq!(budgets["limits"]["js"], 10);
    assert_eq!(budgets["limits"]["requests"], 4, "the Starters' number");
    let home = &budgets["pages"][1];
    assert_eq!(home["url"], "/");
    let event = events.iter().find(|e| e["page"] == "/").unwrap();
    assert_eq!(home["js"], event["value"]);
    assert_eq!(home["requests"], 1);
    let over: Vec<(&str, bool)> = budgets["over"]
        .as_array()
        .unwrap()
        .iter()
        .map(|o| (o["page"].as_str().unwrap(), o["acknowledged"] == true))
        .collect();
    assert_eq!(over, [("/about/", true), ("/", false)]);
    let md = fs::read_to_string(root.join(".yuanme/report.md")).unwrap();
    assert!(md.contains("\n## Budgets\n"), "{md}");
    insta::assert_snapshot!(md.split_once("\n## Budgets\n").unwrap().1);

    // Strict mode makes the breach an error and writes nothing, but the
    // report still says why.
    fs::remove_dir_all(root.join("dist")).unwrap();
    let (code, events) = json(root, &["build", "--strict-budgets"]);
    assert_eq!(code, 1);
    let errors: Vec<&Value> = events.iter().filter(|e| e["level"] == "error").collect();
    assert_eq!(errors.len(), 1, "{}", show(&events));
    assert_eq!(errors[0]["code"], "budget/js");
    assert!(
        errors[0]["rendered"]
            .as_str()
            .unwrap()
            .contains("`--strict-budgets` makes"),
        "{}",
        errors[0]["rendered"]
    );
    assert!(!root.join("dist").exists());
    assert_eq!(report()["ok"], false);
    assert_eq!(report()["budgets"]["over"][1]["page"], "/");
    // As does --strict, and budgets.strict.
    assert_eq!(json(root, &["build", "--strict"]).0, 1);
    write(
        root,
        &site.replace("[budgets]\n", "[budgets]\nstrict = true\n"),
    );
    let (code, events) = json(root, &["build"]);
    assert_eq!(code, 1);
    assert!(show(&events).contains("`budgets.strict` makes"));
    assert_eq!(json(root, &["check"]).0, 0, "check measures no Budget");

    let (head, _) = site.split_once("[budgets]").unwrap();
    let (_, templates) = site.split_once("=== templates/home.html").unwrap();
    let off = head.replace("=== yuanme.toml\n", "=== yuanme.toml\nbudgets = false\n");
    write(root, &format!("{off}=== templates/home.html{templates}"));
    let (code, events) = json(root, &["build", "--strict-budgets"]);
    assert_eq!((code, events.len()), (0, 2), "{}", show(&events));
    assert_eq!(report()["budgets"], Value::Null);
}

const TAILWIND_SITE: &str = r#"=== yuanme.toml
[site]
url = "https://example.com"

[routes.home]
path = "/"
template = "home.html"
=== styles/site.css
@import "tailwindcss";
=== templates/home.html
<link rel="stylesheet" href="/styles/site.css"><h1 class="text-3xl">Home</h1>
"#;

/// The diagnostic events of a `--json` run.
fn codes(stdout: &str) -> Vec<(String, String)> {
    stdout
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .filter(|e| e["type"] == "diagnostic")
        .map(|e| {
            let code = e["code"].as_str().unwrap().to_string();
            (code, e["message"].as_str().unwrap().to_string())
        })
        .collect()
}

/// Offline, with no Tailwind CLI to run, the build says how to give it one.
#[test]
fn a_tailwind_cli_that_isnt_there_is_a_diagnostic() {
    let dir = project(TAILWIND_SITE);
    let root = dir.path();
    let missing = root.join("no-such-tailwindcss");
    let out = yuanme_with(
        root,
        &["build", "--json"],
        &[("YUANME_TAILWIND_BIN", &missing)],
    );
    assert_eq!((out.code, out.stderr.as_str()), (1, ""));
    let [(code, message)] = codes(&out.stdout).try_into().unwrap();
    assert_eq!(code, "asset/tool-unavailable");
    assert!(message.contains("`YUANME_TAILWIND_BIN` names"), "{message}");
    assert!(!root.join("dist").exists());
}

/// The Tailwind CLI's output is cached in `.yuanme/cache` under the
/// tokens it was built from, so a build that adds no class runs no
/// Tailwind, even with none to run.
#[test]
#[ignore = "downloads the Tailwind CLI"]
fn a_warm_build_runs_no_tailwind() {
    let dir = project(TAILWIND_SITE);
    let root = dir.path();
    let stylesheet = |root: &Path| {
        let name = files_under(&root.join("dist/assets"))
            .into_iter()
            .find(|f| f.ends_with(".css"))
            .unwrap();
        fs::read_to_string(root.join("dist/assets").join(name)).unwrap()
    };
    let cold = yuanme_with(root, &["build"], &[]);
    assert_eq!(cold.code, 0, "{}", cold.stderr);
    let css = stylesheet(root);
    assert!(css.contains(".text-3xl{"), "{css}");

    let missing = root.join("no-such-tailwindcss");
    let warm = yuanme_with(root, &["build"], &[("YUANME_TAILWIND_BIN", &missing)]);
    assert_eq!(warm.code, 0, "{}", warm.stderr);
    assert_eq!(stylesheet(root), css);

    // A new class is a new token set, which only Tailwind can build.
    fs::write(
        root.join("templates/home.html"),
        "<link rel=\"stylesheet\" href=\"/styles/site.css\"><h1 class=\"text-4xl\">Home</h1>\n",
    )
    .unwrap();
    let changed = yuanme_with(
        root,
        &["build", "--json"],
        &[("YUANME_TAILWIND_BIN", &missing)],
    );
    assert_eq!(changed.code, 1);
    let codes: Vec<String> = codes(&changed.stdout).into_iter().map(|(c, _)| c).collect();
    assert_eq!(codes, ["asset/tool-unavailable"]);
}

const TYPESCRIPT_SITE: &str = r#"=== yuanme.toml
[site]
url = "https://example.com"

[typescript]
check = true

[routes.home]
path = "/"
template = "home.html"
=== scripts/site.ts
const year: number = "2026";
console.log(year);
=== templates/home.html
<script type="module" src="/scripts/site.ts"></script><h1>Home</h1>
"#;

/// Offline, with no `tsc` to run, the check says how to give it one. A
/// Site whose Scripts are all JavaScript has nothing to check.
#[test]
fn a_tsc_that_isnt_there_is_a_diagnostic() {
    let dir = project(TYPESCRIPT_SITE);
    let root = dir.path();
    let missing = root.join("no-such-tsc");
    let env = [("YUANME_TSC_BIN", missing.as_path())];
    let out = yuanme_with(root, &["check", "--json"], &env);
    assert_eq!((out.code, out.stderr.as_str()), (1, ""));
    let [(code, message)] = codes(&out.stdout).try_into().unwrap();
    assert_eq!(code, "asset/tool-unavailable");
    assert!(message.contains("`YUANME_TSC_BIN` names"), "{message}");
    // Written for the Author's editor, whether or not types are checked.
    let types = fs::read_to_string(root.join(".yuanme/types/yuanme.d.ts")).unwrap();
    assert!(types.contains("declare module 'yuanme:router'"), "{types}");

    fs::remove_file(root.join("scripts/site.ts")).unwrap();
    fs::write(root.join("scripts/site.js"), "console.log(2026);\n").unwrap();
    fs::write(
        root.join("templates/home.html"),
        "<script type=\"module\" src=\"/scripts/site.js\"></script><h1>Home</h1>\n",
    )
    .unwrap();
    let out = yuanme_with(root, &["check", "--json"], &env);
    assert_eq!(out.code, 0, "{}", out.stdout);
}

/// `tsc`'s result is cached in `.yuanme/cache` under every file it may
/// read, so a check of an unchanged project runs no `tsc`, even with none
/// to run, and still reports its type errors.
#[test]
#[ignore = "downloads tsc"]
fn an_unchanged_check_runs_no_tsc() {
    let dir = project(TYPESCRIPT_SITE);
    let root = dir.path();
    let cold = yuanme_with(root, &["check", "--json"], &[]);
    assert_eq!(cold.code, 1, "{}", cold.stderr);
    let errors = codes(&cold.stdout);
    assert_eq!(
        errors,
        [(
            "asset/type-error".to_string(),
            "TS2322: Type 'string' is not assignable to type 'number'.".to_string()
        )]
    );

    let missing = root.join("no-such-tsc");
    let env = [("YUANME_TSC_BIN", missing.as_path())];
    let warm = yuanme_with(root, &["check", "--json"], &env);
    assert_eq!(warm.code, 1, "{}", warm.stderr);
    assert_eq!(codes(&warm.stdout), errors);

    // An edit is a new input, which only tsc can check.
    fs::write(
        root.join("scripts/site.ts"),
        "const year: number = 2026;\nconsole.log(year);\n",
    )
    .unwrap();
    let changed = yuanme_with(root, &["check", "--json"], &env);
    let codes: Vec<String> = codes(&changed.stdout).into_iter().map(|(c, _)| c).collect();
    assert_eq!(codes, ["asset/tool-unavailable"]);
}

/// A running `yuanme dev --json` on a port of its own, with each event,
/// validated against `yuanme schema events`, read as it comes.
struct Dev {
    child: std::process::Child,
    events: std::sync::mpsc::Receiver<Value>,
    address: String,
}

impl Dev {
    fn start(root: &Path) -> Dev {
        let mut schema = serde_json::from_str(&yuanme(root, &["schema", "events"]).stdout).unwrap();
        strict(&mut schema);
        let mut child = Command::new(env!("CARGO_BIN_EXE_yuanme"))
            .args(["dev", "--json", "--port", "0"])
            .current_dir(root)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (send, events) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let validator = jsonschema::validator_for(&schema).unwrap();
            for line in std::io::BufRead::lines(std::io::BufReader::new(stdout)) {
                let line = line.unwrap();
                let event: Value = serde_json::from_str(&line).unwrap();
                let errors: Vec<String> = validator
                    .iter_errors(&event)
                    .map(|e| e.to_string())
                    .collect();
                assert!(errors.is_empty(), "{line}\n{errors:#?}");
                if send.send(event).is_err() {
                    return;
                }
            }
        });
        let mut dev = Dev {
            child,
            events,
            address: String::new(),
        };
        let ready = dev.until("server-ready");
        assert_eq!(ready["pid"], dev.child.id());
        let url = ready["url"].as_str().unwrap();
        dev.address = url
            .strip_prefix("http://")
            .and_then(|u| u.strip_suffix('/'))
            .unwrap()
            .to_string();
        dev
    }

    /// The events up to and including the next of type `kind`.
    fn events_until(&self, kind: &str) -> Vec<Value> {
        let mut seen = Vec::new();
        loop {
            let event = self
                .events
                .recv_timeout(std::time::Duration::from_secs(30))
                .unwrap_or_else(|_| panic!("no {kind} event after {}", show(&seen)));
            let done = event["type"] == kind;
            seen.push(event);
            if done {
                return seen;
            }
        }
    }

    fn until(&self, kind: &str) -> Value {
        self.events_until(kind).pop().unwrap()
    }

    /// The status, headers and body of a GET of `path`.
    fn get(&self, path: &str) -> (u16, String, String) {
        let (status, head, body) = self.get_bytes(path);
        (status, head, String::from_utf8(body).unwrap())
    }

    fn get_bytes(&self, path: &str) -> (u16, String, Vec<u8>) {
        use std::io::{Read, Write};
        let mut stream = std::net::TcpStream::connect(&self.address).unwrap();
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            self.address
        )
        .unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        let at = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8(response[..at].to_vec()).unwrap();
        let status = head.split(' ').nth(1).unwrap().parse().unwrap();
        (
            status,
            head.to_ascii_lowercase(),
            response[at + 4..].to_vec(),
        )
    }
}

impl Drop for Dev {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `yuanme dev` serves the Site with its Drafts from memory, rebuilds it
/// on a save, tells browsers which Pages to reload and keeps serving the
/// last good build while the project has an error.
#[test]
fn dev_serves_the_site_and_reloads_what_a_save_changed() {
    use tungstenite::Message;

    let dir = project(SITE);
    let root = dir.path();
    let dev = Dev::start(root);

    let (status, head, body) = dev.get("/blog/first/");
    assert_eq!(status, 200);
    assert!(head.contains("content-type: text/html"), "{head}");
    assert!(head.contains("cache-control: no-store"), "{head}");
    assert!(body.contains("Hello."), "{body}");
    assert_eq!(dev.get("/blog/wip/").0, 200, "a Draft is served");
    let (status, head, _) = dev.get("/blog/first?x=1");
    assert_eq!(status, 308);
    assert!(head.contains("location: /blog/first/?x=1"), "{head}");
    assert_eq!(dev.get("/no/such/page/").0, 404);
    assert_eq!(
        dev.get("/data/nav.yaml").0,
        404,
        "a project file isn't served"
    );

    let (mut socket, _) =
        tungstenite::connect(format!("ws://{}/__yuanme/ws", dev.address)).unwrap();
    let console = serde_json::json!({
        "type": "browser-console",
        "level": "error",
        "message": "boom",
        "url": "/blog/first/",
        "stack": "at main.js:1",
    });
    socket.send(Message::text(console.to_string())).unwrap();
    assert_eq!(dev.until("browser-console"), console);

    let post = root.join("content/posts/first.md");
    let text = fs::read_to_string(&post).unwrap();
    fs::write(&post, text.replace("Hello.", "Hello, again.")).unwrap();
    let events = dev.events_until("reload");
    let kinds: Vec<&str> = events.iter().map(|e| e["type"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["build-started", "build-finished", "reload"]);
    // Only the Page: the list and the feed show none of its body.
    let urls = serde_json::json!(["/blog/first/"]);
    assert_eq!(events[1]["changed_urls"], urls);
    assert_eq!(events[2]["urls"], urls);
    let Message::Text(message) = socket.read().unwrap() else {
        panic!("a reload is text")
    };
    assert_eq!(
        serde_json::from_str::<Value>(&message).unwrap(),
        serde_json::json!({ "type": "reload", "urls": urls })
    );
    assert!(dev.get("/blog/first/").2.contains("Hello, again."));

    // An error is reported, and the last good build stays served.
    fs::write(&post, text.replace("kind: essay", "kind: poem")).unwrap();
    let events = dev.events_until("build-finished");
    assert_eq!(events.last().unwrap()["ok"], false);
    assert!(events.iter().any(|e| e["type"] == "diagnostic"));
    assert!(dev.get("/blog/first/").2.contains("Hello, again."));
}

/// Interrupted, `yuanme dev` saves its cache and exits 0.
#[cfg(unix)]
#[test]
fn dev_stops_on_sigterm_and_keeps_its_cache() {
    let dir = project(SITE);
    let root = dir.path();
    let mut dev = Dev::start(root);
    let status = Command::new("kill")
        .arg(dev.child.id().to_string())
        .status()
        .unwrap();
    assert!(status.success());
    assert_eq!(dev.child.wait().unwrap().code(), Some(0));
    let mut stderr = String::new();
    std::io::Read::read_to_string(&mut dev.child.stderr.take().unwrap(), &mut stderr).unwrap();
    assert_eq!(stderr, "", "human text with --json");
    assert!(root.join(".yuanme/cache/generation").exists());
}

/// `yuanme dev` encodes an image only when a browser asks for it, into the
/// bytes `yuanme build` writes, and keeps it in the cache.
#[test]
fn dev_encodes_an_image_when_a_browser_asks() {
    let dir = project(
        "=== yuanme.toml
[site]
url = \"https://example.com\"

[collections.posts]
route = \"/blog/{slug}/\"

[images]
widths = [32]
=== templates/posts/entry.html
{{ entry.content }}
=== content/posts/sea/index.md
---
title: Sea
---
![A beach](beach.jpg)
",
    );
    let root = dir.path();
    let photo = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../yuanme-assets/fixtures/images/beach.jpg"
    );
    fs::copy(photo, root.join("content/posts/sea/beach.jpg")).unwrap();
    let encoded = || match fs::read_dir(root.join(".yuanme/cache/large")) {
        Ok(entries) => entries.count(),
        Err(_) => 0,
    };

    let dev = Dev::start(root);
    assert_eq!(encoded(), 0);
    let page = dev.get("/blog/sea/").2;
    let avif = page
        .split('"')
        .find(|s| s.ends_with(".avif") || s.ends_with(".avif 32w"))
        .map(|s| s.trim_end_matches(" 32w"))
        .unwrap_or_else(|| panic!("no AVIF in {page}"));
    let (status, head, served) = dev.get_bytes(avif);
    assert_eq!(status, 200);
    assert!(head.contains("content-type: image/avif"), "{head}");
    assert_eq!(encoded(), 1, "only the variant asked for");
    // Again, from the cache.
    assert_eq!(dev.get_bytes(avif).2, served);
    drop(dev);

    assert_eq!(yuanme(root, &["build", "-q"]).code, 0);
    let built = fs::read(root.join("dist").join(avif.trim_start_matches('/'))).unwrap();
    assert!(built == served, "dev encoded other bytes than build");
}
