use super::*;
use crate::content::entries;

const CONFIG: &[u8] = b"[site]\nurl = \"https://example.com\"\n";

/// The smallest project: Site config and nothing else.
fn project() -> Inputs {
    let mut inputs = Inputs::default();
    inputs.insert("yuanme.toml", CONFIG);
    inputs
}

fn key(cache: &Cache, kind: &str, part: &str) -> blake3::Hash {
    let mut hasher = cache.key(kind);
    hasher.update(part.as_bytes());
    hasher.finalize()
}

fn text(body: &Body) -> &str {
    match body {
        Body::Bytes(bytes) => std::str::from_utf8(bytes).unwrap(),
        other => panic!("expected bytes, got {other:?}"),
    }
}

#[test]
fn static_files_are_copied_to_the_same_path_and_nothing_else_is() {
    let mut inputs = project();
    inputs.insert("static/robots.txt", b"User-agent: *".as_slice());
    inputs.insert("static/static/x.css", b"a{}".as_slice());
    inputs.insert_streamed("static/img/cat.jpg", blake3::hash(b"cat"));
    inputs.insert("content/posts/pew.md", b"# Pew".as_slice());

    let build = build(&inputs, &mut Cache::memory("v"), Mode::Build);

    assert!(!build.failed());
    let paths: Vec<_> = build.files.keys().map(String::as_str).collect();
    // `_headers` is the default adapter's.
    assert_eq!(
        paths,
        ["_headers", "img/cat.jpg", "robots.txt", "static/x.css"]
    );
    assert_eq!(text(&build.files["robots.txt"].body), "User-agent: *");
    assert_eq!(
        build.files["robots.txt"].hash,
        blake3::hash(b"User-agent: *")
    );
    // A streamed image is copied from the project, not held in memory.
    let cat = &build.files["img/cat.jpg"];
    assert_eq!(cat.body, Body::Input("static/img/cat.jpg".into()));
    assert_eq!(cat.hash, blake3::hash(b"cat"));
}

#[test]
fn a_config_error_fails_the_build_with_nothing_to_write() {
    let mut inputs = project();
    inputs.insert("static/robots.txt", b"User-agent: *".as_slice());
    inputs.insert(
        "yuanme.toml",
        b"[site]\nurl = \"https://e.com\"\ntitel = \"x\"\n".as_slice(),
    );

    let build = build(&inputs, &mut Cache::memory("1.0.0"), Mode::Build);

    assert!(build.failed());
    assert!(build.files.is_empty());
    let [d] = build.diagnostics.as_slice() else {
        panic!("{:#?}", build.diagnostics)
    };
    assert_eq!(d.code.name, "config/unknown-key");
    assert_eq!(build.sources.path(d.primary().file), "yuanme.toml");
    assert_eq!(
        &build.sources.text(d.primary().file)[d.primary().range.clone()],
        "titel"
    );

    // The version check reads the Engine version the cache is keyed by,
    // including a git build's.
    inputs.insert(
        "yuanme.toml",
        [b"yuanme = \"0.4\"\n".as_slice(), CONFIG].concat(),
    );
    let codes = |version| {
        let build = super::build(&inputs, &mut Cache::memory(version), Mode::Build);
        build
            .diagnostics
            .iter()
            .map(|d| d.code.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(codes("0.4.1+403e56e.dirty"), [] as [&str; 0]);
    assert_eq!(codes("0.3.0"), ["config/engine-version"]);
}

#[test]
fn site_config_that_is_not_utf8_is_an_error_not_a_lossy_guess() {
    let mut inputs = project();
    // Valid TOML once the bad byte becomes U+FFFD in a comment.
    inputs.insert("yuanme.toml", [CONFIG, b"# caf\xe9\n".as_slice()].concat());

    let build = build(&inputs, &mut Cache::memory("1.0.0"), Mode::Build);

    let [d] = build.diagnostics.as_slice() else {
        panic!("{:#?}", build.diagnostics)
    };
    assert_eq!(d.code.name, "config/syntax");
    assert_eq!(d.primary().range, CONFIG.len() + 5..CONFIG.len() + 5);
    assert!(build.failed() && build.files.is_empty());
}

#[test]
fn link_names_assets_by_the_hash_of_their_final_bytes() {
    // A font holds NUL bytes, which are not placeholders.
    let font = b"wOF2\0\x01\0".to_vec();
    let font_hash = blake3::hash(&font).to_hex();
    let font_url = format!("/assets/math-{}.woff2", &font_hash[..16]);
    // The stylesheet's name must cover the font's name, or a new font would
    // be served under the old stylesheet's immutable URL.
    let css = format!("@font-face{{src:url({font_url})}}");
    let css_url = format!(
        "/assets/site-{}.css",
        &blake3::hash(css.as_bytes()).to_hex()[..16]
    );

    let files = hashing::link(
        vec![(
            "index.html".into(),
            b"<link href=\"\0asset:1\0\"><p>\0asset:0\0".to_vec(),
        )],
        vec![
            hashing::Asset::Copy {
                name: "math.woff2".into(),
                body: Body::Bytes(font.clone().into()),
                hash: blake3::hash(&font),
            },
            hashing::Asset::Linked {
                name: "site.css".into(),
                bytes: b"@font-face{src:url(\0asset:0\0)}".to_vec(),
            },
        ],
        &mut Cache::memory("v"),
        false,
    );

    let page = format!("<link href=\"{css_url}\"><p>{font_url}");
    assert_eq!(text(&files["index.html"].body), page);
    assert_eq!(files["index.html"].hash, blake3::hash(page.as_bytes()));
    assert_eq!(text(&files[&css_url[1..]].body), css);
    assert_eq!(files[&font_url[1..]].hash, blake3::hash(&font));
    assert_eq!(files.len(), 3);
}

#[test]
fn the_disk_cache_survives_a_restart_but_not_a_new_engine_version() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache");

    let mut cache = Cache::open(path.clone(), "1.0.0").unwrap();
    let small = key(&cache, "entry", "pew.md");
    let large = key(&cache, "image", "cat.jpg@800w");
    cache.insert("entry", small, b"parsed".as_slice());
    cache.insert_large(large, b"AVIF").unwrap();
    cache.save().unwrap();

    let cache = Cache::open(path.clone(), "1.0.0").unwrap();
    assert_eq!(
        cache.get("entry", &small).as_deref(),
        Some(b"parsed".as_slice())
    );
    let Some(Body::Cached(file)) = cache.get_large(&large) else {
        panic!("a large entry is a file on disk");
    };
    assert_eq!(std::fs::read(file).unwrap(), b"AVIF");

    let cache = Cache::open(path, "1.0.1").unwrap();
    assert_eq!(cache.get("entry", &key(&cache, "entry", "pew.md")), None);
    assert_eq!(cache.get_large(&key(&cache, "image", "cat.jpg@800w")), None);
}

#[test]
fn entries_unread_for_ten_successful_builds_are_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let after = |builds: usize, mode: Mode| {
        let path = dir.path().join(format!("{builds}-{mode:?}"));
        let mut cache = Cache::open(path.clone(), "v").unwrap();
        let small = key(&cache, "entry", "a");
        let large = key(&cache, "image", "a");
        cache.insert("entry", small, b"x".as_slice());
        cache.insert_large(large, b"x").unwrap();
        for _ in 0..builds {
            build(&project(), &mut cache, mode);
        }
        cache.save().unwrap();
        let cache = Cache::open(path.clone(), "v").unwrap();
        let file = path.join("large").join(large.to_hex().as_str());
        (
            cache.get("entry", &small).is_some(),
            cache.get_large(&large).is_some(),
            file.exists(),
        )
    };

    // The first build wrote them, which counts as reading them. They are
    // kept while any of the last ten builds read them.
    assert_eq!(after(10, Mode::Build), (true, true, true));
    assert_eq!(after(11, Mode::Build), (false, false, false));
    // `yuanme dev` builds on every save, and reads neither the image
    // variants nor what only `yuanme build` reads, so its builds don't
    // count.
    assert_eq!(after(11, Mode::Dev), (true, true, true));
}

#[cfg(unix)]
#[test]
fn a_pack_is_rewritten_only_when_it_changed() {
    use std::os::unix::fs::MetadataExt;

    let dir = tempfile::tempdir().unwrap();
    let pack = dir.path().join("entry.pack");
    let inode = || std::fs::metadata(&pack).unwrap().ino();
    let mut cache = Cache::open(dir.path().into(), "v").unwrap();
    let a = key(&cache, "entry", "a");
    cache.insert("entry", a, b"x".as_slice());
    cache.save().unwrap();
    let written = inode();

    // A build that reads nothing from the pack leaves the file alone.
    let mut cache = Cache::open(dir.path().into(), "v").unwrap();
    build(&project(), &mut cache, Mode::Build);
    cache.save().unwrap();
    assert_eq!(inode(), written);

    // Reading an entry records its use, which is a change.
    let mut cache = Cache::open(dir.path().into(), "v").unwrap();
    cache.get("entry", &a);
    cache.save().unwrap();
    assert_ne!(inode(), written);
}

#[test]
fn content_is_resolved_and_its_errors_fail_the_build() {
    let mut inputs = project();
    inputs.insert(
        "yuanme.toml",
        [
            CONFIG,
            b"[collections.posts]\nroute = \"/blog/{slug}/\"\n".as_slice(),
        ]
        .concat(),
    );
    let codes = |inputs: &Inputs, mode| {
        let build = build(inputs, &mut Cache::memory("1.0.0"), mode);
        assert_eq!(build.failed(), !build.diagnostics.is_empty());
        assert!(!build.failed() || build.files.is_empty());
        build
            .diagnostics
            .iter()
            .map(|d| d.code.name)
            .collect::<Vec<_>>()
    };
    assert_eq!(codes(&inputs, Mode::Build), ["config/invalid-value"]);
    // An empty Collection folder exists.
    inputs.insert_dir("content/posts");
    assert_eq!(codes(&inputs, Mode::Build), [] as [&str; 0]);

    // A Draft has a Route only in dev or with Drafts, where it meets the
    // Static file.
    inputs.insert(
        "content/posts/wip.md",
        b"---\ntitle: WIP\ndraft: true\n---\n".as_slice(),
    );
    inputs.insert("static/blog/wip/index.html", b"<p>old".as_slice());
    assert_eq!(codes(&inputs, Mode::Build), [] as [&str; 0]);
    assert_eq!(codes(&inputs, Mode::Drafts), ["entry/output-collision"]);
    assert_eq!(codes(&inputs, Mode::Dev), ["entry/output-collision"]);

    inputs.insert(
        "content/posts/b.md",
        b"---\ntitle: caf\xe9\n---\n".as_slice(),
    );
    assert_eq!(codes(&inputs, Mode::Build), ["entry/syntax"]);
}

#[test]
fn templates_render_each_route_and_the_sitemap_and_sidecars_are_not_templates() {
    let mut inputs = project();
    inputs.insert(
        "yuanme.toml",
        [
            CONFIG,
            b"[collections.posts]\nroute = \"/blog/{slug}/\"\n".as_slice(),
        ]
        .concat(),
    );
    inputs.insert(
        "content/posts/pew.md",
        b"---\ntitle: Pew\n---\nHi.".as_slice(),
    );
    inputs.insert(
        "templates/posts/entry.html",
        b"<h1>{{ entry.title }}</h1>{{ entry.content }}".as_slice(),
    );
    // A Sidecar, which is CSS, not Jinja: `{#` would open a comment.
    inputs.insert("templates/posts/entry.css", b"a{#x{color:red}}".as_slice());
    inputs.insert("static/robots.txt", b"".as_slice());

    let build = build(&inputs, &mut Cache::memory("1.0.0"), Mode::Build);

    assert!(!build.failed(), "{:#?}", build.diagnostics);
    let paths: Vec<_> = build.files.keys().map(String::as_str).collect();
    let sidecars = format!(
        "assets/sidecars-{}.css",
        &blake3::hash(b"a[data-y-ihxwj1] #x[data-y-ihxwj1]{color:red}").to_hex()[..16]
    );
    assert_eq!(
        paths,
        [
            "_headers",
            sidecars.as_str(),
            "blog/pew/index.html",
            "robots.txt",
            "sitemap-0.xml",
            "sitemap-index.xml"
        ]
    );
    assert_eq!(
        text(&build.files["blog/pew/index.html"].body),
        "<h1 data-y-ihxwj1>Pew</h1><p>Hi.</p>"
    );

    // The sitemap is the Engine's, so a Static file can't take its name.
    inputs.insert("static/sitemap-index.xml", b"".as_slice());
    let build = super::build(&inputs, &mut Cache::memory("1.0.0"), Mode::Build);
    let codes: Vec<_> = build.diagnostics.iter().map(|d| d.code.name).collect();
    assert_eq!(codes, ["entry/output-collision"]);
}

#[test]
fn code_is_highlighted_through_the_cache_and_styled_by_one_stylesheet() {
    let mut inputs = project();
    inputs.insert(
        "yuanme.toml",
        [
            CONFIG,
            b"[collections.posts]\nroute = \"/blog/{slug}/\"\n".as_slice(),
        ]
        .concat(),
    );
    // The same text in two languages, which a key without the grammar
    // would give one highlighting.
    let post = "---\ntitle: Pew\n---\n```rust\nfn f() {}\n```\n\n```c\nfn f() {}\n```\n\n```pyhton\nx\n```\n";
    inputs.insert("content/posts/pew.md", post.as_bytes());
    inputs.insert(
        "content/posts/plain.md",
        b"---\ntitle: Plain\n---\nHi.".as_slice(),
    );
    inputs.insert(
        "templates/posts/entry.html",
        b"<html><head><title>{{ entry.title }}</title></head><body>{{ entry.content }}</body></html>"
            .as_slice(),
    );

    let mut cache = Cache::memory("1.0.0");
    let build = super::build(&inputs, &mut cache, Mode::Build);

    assert!(!build.failed(), "{:#?}", build.diagnostics);
    let [d] = build.diagnostics.as_slice() else {
        panic!("{:#?}", build.diagnostics)
    };
    assert_eq!(d.code.name, "entry/unknown-language");
    let at = &d.primary().range;
    assert_eq!(&post[at.clone()], "pyhton");
    let edit = &d.children[0].suggestions[0].edits[0];
    assert_eq!(
        (&edit.span.range, edit.replacement.as_str()),
        (at, "python")
    );

    let page = text(&build.files["blog/pew/index.html"].body);
    let css_path = build
        .files
        .keys()
        .find(|p| p.starts_with("assets/code-") && p.ends_with(".css"))
        .expect("a stylesheet for the code");
    let (head, body) = page.split_once("</head>").unwrap();
    assert!(
        head.ends_with(&format!("<link rel=\"stylesheet\" href=\"/{css_path}\">")),
        "{head}"
    );
    let blocks: Vec<&str> = body.split("<pre").skip(1).collect();
    assert_eq!(blocks.len(), 3, "{body}");
    assert!(
        blocks[0].starts_with(
            " class=\"y-code\" tabindex=\"0\" data-language=\"rust\"><code><span class=\""
        ),
        "{}",
        blocks[0]
    );
    assert!(blocks[1].contains("data-language=\"c\""));
    let code = |b: &str| b.split_once("<code>").unwrap().1.to_string();
    // Rust colours `fn` as a keyword; C has no such keyword.
    assert_ne!(code(blocks[0]), code(blocks[1]));
    assert_eq!(
        blocks[2],
        " class=\"y-code\" tabindex=\"0\" data-language=\"plaintext\"><code>x</code></pre></body></html>"
    );
    // Every class the Page uses has a rule, in both themes.
    let css = text(&build.files[css_path].body);
    let class = code(blocks[0]);
    let class = class.split('"').nth(1).unwrap();
    assert_eq!(
        css.matches(&format!(".y-code .{class}{{")).count(),
        2,
        "{css}"
    );
    // A Page with no code gets no stylesheet.
    assert!(!text(&build.files["blog/plain/index.html"].body).contains("<link"));

    // A warm build reads the cache and writes the same bytes.
    let again = super::build(&inputs, &mut cache, Mode::Build);
    assert_eq!(text(&again.files["blog/pew/index.html"].body), page);
}

#[test]
fn pages_with_math_get_one_font_subset_for_the_site_and_its_stylesheet() {
    let mut inputs = project();
    inputs.insert(
        "yuanme.toml",
        [
            CONFIG,
            b"[collections.posts]\nroute = \"/blog/{slug}/\"\n".as_slice(),
        ]
        .concat(),
    );
    inputs.insert(
        "content/posts/a.md",
        b"---\ntitle: A\n---\n$x$\n".as_slice(),
    );
    inputs.insert(
        "content/posts/b.md",
        b"---\ntitle: B\n---\n$\\sum y$\n".as_slice(),
    );
    inputs.insert(
        "content/posts/c.md",
        b"---\ntitle: C\n---\nNo math.\n".as_slice(),
    );
    inputs.insert(
        "templates/posts/entry.html",
        b"<html><head></head><body>{{ entry.content }}</body></html>".as_slice(),
    );

    let mut cache = Cache::memory("1.0.0");
    let build = super::build(&inputs, &mut cache, Mode::Build);
    assert!(!build.failed(), "{:#?}", build.diagnostics);
    let find = |files: &BTreeMap<String, Output>, prefix: &str, ext: &str| {
        let found: Vec<String> = files
            .keys()
            .filter(|p| p.starts_with(prefix) && p.ends_with(ext))
            .cloned()
            .collect();
        let [one] = &found[..] else {
            panic!("{found:?}")
        };
        one.clone()
    };
    let css = find(&build.files, "assets/math-", ".css");
    let font = find(&build.files, "assets/math-", ".woff2");
    for page in ["blog/a/index.html", "blog/b/index.html"] {
        let head = text(&build.files[page].body)
            .split_once("</head>")
            .unwrap()
            .0;
        assert!(
            head.ends_with(&format!("<link rel=\"stylesheet\" href=\"/{css}\">")),
            "{head}"
        );
    }
    assert!(!text(&build.files["blog/c/index.html"].body).contains("<link"));
    let sheet = text(&build.files[&css].body);
    assert!(
        sheet.starts_with(&format!(
            "@font-face{{font-family:\"yuanme NCM Math\";src:url(\"/{font}\") format(\"woff2\")"
        )),
        "{sheet}"
    );
    assert!(sheet.contains("math{font-family:\"yuanme NCM Math\",math}"));
    // One subset for both Pages, of what their math shows, its italics and
    // what browsers measure.
    let chars = BTreeSet::from([
        ' ', '\u{AF}', '1', 'X', 'o', '\u{2212}', '\u{221A}', 'x', '𝑥', '∑', 'y', '𝑦',
    ]);
    let subset = yuanme_assets::font::subset(yuanme_assets::font::NCM_MATH, &chars);
    let expected = yuanme_assets::font::woff2(&subset);
    assert_eq!(build.files[&font].hash, blake3::hash(&expected));

    // `yuanme dev` serves the whole font.
    let dev = super::build(&inputs, &mut cache, Mode::Dev);
    let otf = find(&dev.files, "assets/math-", ".otf");
    assert_eq!(
        dev.files[&otf].hash,
        blake3::hash(yuanme_assets::font::NCM_MATH)
    );
    let sheet = text(&dev.files[&find(&dev.files, "assets/math-", ".css")].body);
    assert!(
        sheet.contains(&format!("src:url(\"/{otf}\") format(\"opentype\")")),
        "{sheet}"
    );

    // A Site without math gets neither.
    inputs.insert("content/posts/a.md", b"---\ntitle: A\n---\nx\n".as_slice());
    inputs.insert("content/posts/b.md", b"---\ntitle: B\n---\ny\n".as_slice());
    let plain = super::build(&inputs, &mut cache, Mode::Build);
    assert!(!plain.files.keys().any(|p| p.starts_with("assets/math-")));
}

/// A project whose two Posts, one in each Source format, use every kind of
/// field and most of what a body parses into.
fn every_field() -> Inputs {
    let mut inputs = project();
    inputs.insert(
        "yuanme.toml",
        [
            CONFIG,
            br#"timezone = "Asia/Singapore"

[collections.posts]
route = "/blog/{slug}/"

[collections.posts.fields]
rating = { type = "float", optional = true }
stars = { type = "int", optional = true }
at = { type = "datetime", optional = true }
local = { type = "datetime", optional = true }
home = { type = "url", optional = true }
cover = { type = "image", optional = true }
links = { type = "list", items = { type = "string" }, optional = true }
by = { type = "object", fields = { name = { type = "string" } }, optional = true }

[taxonomies.tags]
collections = ["posts"]
route = "/tags/{term}/"

[math.macros]
R = '\mathbb{R}'
"#
            .as_slice(),
        ]
        .concat(),
    );
    inputs.insert(
        "content/posts/pew.md",
        br#"---
title: "Pew"
date: 2026-01-02
updated: 2026-02-03
rating: .nan
stars: 3
at: 2026-01-02T09:30:00+08:00
local: 2026-01-02T09:30:00
home: https://example.com/pew
cover: cat.svg
links: [a, b]
by: { name: Yuan }
tags: [Rust, C#]
---

## Sums {#sums}

"Quoted" -- text with `code`, *emphasis* and a [link](org.org#top)[^1].

$$
\sum_{x \in \R} x
$$ {#eq:sum}

- [x] done
- [ ] not

| a | b |
|:--|--:|
| 1 | 2 |

```rust
fn main() {}
```

[^1]: A footnote.
"#
        .as_slice(),
    );
    inputs.insert(
        "content/posts/org.org",
        b"#+TITLE: Org\n#+DATE: 2026-01-04\n#+FILETAGS: :rust:\n\n* Top\n:PROPERTIES:\n:ID: top\n:END:\n\nSee <<here>> and [[id:top]], \\(x^2\\).\n\n#+BEGIN_QUOTE\nQuoted.\n#+END_QUOTE\n"
            .as_slice(),
    );
    for template in ["posts/entry.html", "tags/index.html", "tags/term.html"] {
        inputs.insert(format!("templates/{template}"), b"<p>x</p>".as_slice());
    }
    inputs.insert(
        "content/posts/cat.svg",
        b"<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 4 3\"></svg>".as_slice(),
    );
    inputs
}

/// ADR 0011: a parsed Entry is a leaf cache, so a build that finds it on
/// disk or in memory gets the Entry a fresh read makes.
#[test]
fn a_cached_entry_is_the_entry_a_fresh_read_makes() {
    let inputs = every_field();
    let mode = Mode::Check { drafts: false };
    let entries = |build: &Build| {
        assert!(!build.failed(), "{:#?}", build.diagnostics);
        assert!(build.diagnostics.is_empty(), "{:#?}", build.diagnostics);
        let site = build.site.as_ref().unwrap();
        assert_eq!(site.entries.len(), 2);
        format!("{:#?}", site.entries)
    };
    let fresh = entries(&build(&inputs, &mut Cache::memory("v"), mode));

    let dir = tempfile::tempdir().unwrap();
    let mut cache = Cache::open(dir.path().into(), "v").unwrap();
    assert_eq!(entries(&build(&inputs, &mut cache, mode)), fresh);
    cache.save().unwrap();

    // Another process finds them on disk, and the next build in memory.
    let mut cache = Cache::open(dir.path().into(), "v").unwrap();
    for path in ["content/posts/pew.md", "content/posts/org.org"] {
        let bytes = cache.get("entry", &entries::key(&cache, &inputs, path));
        let read = bytes.and_then(|b| entries::decode(&b, path));
        assert!(read.is_some(), "{path} isn't cached");
    }
    assert_eq!(entries(&build(&inputs, &mut cache, mode)), fresh);
    assert_eq!(entries(&build(&inputs, &mut cache, mode)), fresh);
}

/// ADR 0011: a cached Entry is keyed by everything its read reads, so a
/// change to any of it reads the Entry again.
#[test]
fn an_entry_is_read_again_when_anything_it_read_changes() {
    let mut inputs = every_field();
    let mut cache = Cache::memory("v");
    let mode = Mode::Check { drafts: false };
    let mut codes = |inputs: &Inputs| {
        let build = build(inputs, &mut cache, mode);
        let codes: Vec<&str> = build.diagnostics.iter().map(|d| d.code.name).collect();
        (build, codes)
    };
    assert_eq!(codes(&inputs).1, [] as [&str; 0]);

    // The Collection's schema, in Site config.
    let config = inputs.get("yuanme.toml").unwrap().bytes.clone().unwrap();
    let config = std::str::from_utf8(&config).unwrap();
    inputs.insert(
        "yuanme.toml",
        config
            .replace("stars = { type = \"int\"", "stars = { type = \"bool\"")
            .as_bytes(),
    );
    assert_eq!(codes(&inputs).1, ["entry/wrong-type"]);
    // The Site's timezone, which a datetime without an offset is in.
    inputs.insert(
        "yuanme.toml",
        config.replace("Asia/Singapore", "Europe/London").as_bytes(),
    );
    let (build, _) = codes(&inputs);
    let local = |build: &Build| {
        let site = build.site.as_ref().unwrap();
        let pew = site.entries.iter().find(|e| e.slug == "pew").unwrap();
        pew.fields["local"].clone()
    };
    let london = local(&build);
    inputs.insert("yuanme.toml", config.as_bytes());
    assert_ne!(local(&codes(&inputs).0), london);

    // A file the Entry names, which is gone.
    let cat = inputs.remove("content/posts/cat.svg").unwrap();
    assert_eq!(codes(&inputs).1, ["image/not-found"]);
    inputs.insert("content/posts/cat.svg", cat.bytes.unwrap());
    assert_eq!(codes(&inputs).1, [] as [&str; 0]);

    // A new file read before it, which shifts the id of the Entry's file.
    inputs.insert("content/posts/a.md", b"---\ntitle: A\n---\nA.\n".as_slice());
    let (build, codes) = codes(&inputs);
    assert_eq!(codes, [] as [&str; 0]);
    for e in &build.site.as_ref().unwrap().entries {
        assert_eq!(build.sources.path(e.file), e.path);
    }
}

/// Every HTML Page a build makes, by its path, which must not fail.
fn html(build: &Build) -> BTreeMap<String, String> {
    assert!(!build.failed(), "{:#?}", build.diagnostics);
    build
        .files
        .iter()
        .filter(|(p, _)| p.ends_with(".html"))
        .map(|(p, o)| (p.clone(), text(&o.body).to_string()))
        .collect()
}

/// ADR 0011: an Entry's body HTML is a leaf cache, so a build that finds it
/// makes the Pages a fresh render makes.
#[test]
fn a_cached_body_is_the_body_a_fresh_render_makes() {
    let mut inputs = every_field();
    inputs.insert(
        "templates/posts/entry.html",
        b"<html><head></head><body>{{ entry.content }}</body></html>".as_slice(),
    );
    let fresh = html(&build(&inputs, &mut Cache::memory("v"), Mode::Dev));
    let mut cache = Cache::memory("v");
    for _ in 0..3 {
        assert_eq!(html(&build(&inputs, &mut cache, Mode::Dev)), fresh);
    }
}

/// ADR 0011: a cached body is keyed by everything its render reads, so a
/// change to any of it renders the body again.
#[test]
fn a_body_is_rendered_again_when_anything_it_reads_changes() {
    let mut inputs = project();
    let config = [
        CONFIG,
        b"\n[collections.posts]\nroute = \"/blog/{slug}/\"\n\n[math.macros]\nR = '\\mathbb{R}'\n"
            .as_slice(),
    ]
    .concat();
    inputs.insert("yuanme.toml", config.clone());
    let a =
        "---\ntitle: A\n---\nBy [](b.md#thm:b), in [B](b.md), over $\\R$.\n\n![A cat](cat.svg)\n";
    inputs.insert("content/posts/a.md", a.as_bytes());
    let b = "---\ntitle: B\n---\n:::theorem{#thm:b}\nx\n:::\n";
    inputs.insert("content/posts/b.md", b.as_bytes());
    let cat = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 4 3\"></svg>";
    inputs.insert("content/posts/cat.svg", cat.as_bytes());
    inputs.insert(
        "templates/posts/entry.html",
        b"<html><head></head><body>{{ entry.content }}</body></html>".as_slice(),
    );
    let mut cache = Cache::memory("v");
    let mut body = |inputs: &Inputs, what: &str| {
        let cached = html(&build(inputs, &mut cache, Mode::Dev));
        let fresh = html(&build(inputs, &mut Cache::memory("v"), Mode::Dev));
        assert_eq!(cached, fresh, "after {what}");
        cached["blog/a/index.html"].clone()
    };
    let mut before = body(&inputs, "the first build");
    assert_eq!(body(&inputs, "no change"), before);
    let mut changed = |inputs: &Inputs, what: &str, shows: &str| {
        let now = body(inputs, what);
        assert_ne!(now, before, "{what} didn't change the body");
        assert!(now.contains(shows), "after {what}: {now}");
        before = now;
    };

    // The Entry's own text.
    inputs.insert("content/posts/a.md", a.replace("By", "Then by").as_bytes());
    changed(&inputs, "an edit", "Then by");
    // The number of another Entry's Theorem, which the link shows.
    let b = b.replace(
        ":::theorem{#thm:b}",
        ":::theorem\nw\n:::\n\n:::theorem{#thm:b}",
    );
    inputs.insert("content/posts/b.md", b.as_bytes());
    changed(&inputs, "a Theorem before the one it links to", "Theorem 2");
    // Another Entry's Slug, which its links go to.
    inputs.insert(
        "content/posts/b.md",
        b.replace("title: B", "title: B\nslug: bee").as_bytes(),
    );
    changed(&inputs, "a new Slug", "/blog/bee/#thm:b");
    // A math macro, in Site config.
    let bold = String::from_utf8(config)
        .unwrap()
        .replace("mathbb", "mathbf");
    inputs.insert("yuanme.toml", bold.as_bytes());
    changed(&inputs, "a math macro", "<mi>𝐑</mi>");
    // The image it shows, whose size its markup gives.
    inputs.insert(
        "content/posts/cat.svg",
        cat.replace("4 3", "8 3").as_bytes(),
    );
    changed(&inputs, "a wider image", "width=\"8\"");
}

/// ADR 0011: what `yuanme dev` keeps of each Page between builds is keyed
/// by the Page's bytes and what else each pass reads, so a build with it
/// makes the files a build without it makes.
#[test]
fn a_page_is_processed_again_when_anything_it_reads_changes() {
    let mut inputs = project();
    let config = [
        CONFIG,
        b"\n[collections.posts]\nroute = \"/blog/{slug}/\"\n".as_slice(),
    ]
    .concat();
    inputs.insert("yuanme.toml", config.clone());
    let page = "<html><head></head><body>{{ entry.content }}</body></html>";
    inputs.insert("templates/posts/entry.html", page.as_bytes());
    let a = "---\ntitle: A\n---\nSee [B](/blog/b/) and [C](https://other.com/c/).\n";
    inputs.insert("content/posts/a.md", a.as_bytes());
    let b = b"---\ntitle: B\n---\nB.\n".as_slice();
    inputs.insert("content/posts/b.md", b);
    inputs.insert("styles/a.css", b"p { color: red }".as_slice());
    let mut cache = Cache::memory("v");
    // The files and the Diagnostics, which must be a fresh build's.
    let mut output = |inputs: &Inputs, what: &str| {
        let seen = |build: Build| {
            assert!(!build.failed(), "{what}: {:#?}", build.diagnostics);
            let rendered: Vec<String> = build
                .diagnostics
                .iter()
                .map(|d| d.render_short(&build.sources))
                .collect();
            let files: BTreeMap<String, blake3::Hash> =
                build.files.into_iter().map(|(p, o)| (p, o.hash)).collect();
            (files, rendered)
        };
        let cached = seen(build(inputs, &mut cache, Mode::Dev));
        let fresh = seen(build(inputs, &mut Cache::memory("v"), Mode::Dev));
        assert!(fresh.0.contains_key("blog/a/index.html"), "{what}");
        assert_eq!(cached, fresh, "after {what}");
        cached.1
    };
    assert_eq!(output(&inputs, "the first build"), [] as [String; 0]);
    assert_eq!(output(&inputs, "no change"), [] as [String; 0]);

    // A stylesheet its Template now links, which only a new scan finds.
    let linked = page.replace(
        "<head>",
        "<head><link rel=\"stylesheet\" href=\"/styles/a.css\">",
    );
    inputs.insert("templates/posts/entry.html", linked.as_bytes());
    output(&inputs, "a new stylesheet link");
    // The stylesheet, whose new name the Page links to, though the Page
    // is the same until the link pass.
    inputs.insert("styles/a.css", b"p { color: blue }".as_slice());
    output(&inputs, "an edited stylesheet");
    // The Page a link goes to, which is gone.
    inputs.remove("content/posts/b.md");
    assert_eq!(output(&inputs, "a removed Page").len(), 1);
    inputs.insert("content/posts/b.md", b);
    assert_eq!(output(&inputs, "the Page back").len(), 0);
    // The Site's origin, on which a link that went elsewhere is broken.
    let moved = String::from_utf8(config)
        .unwrap()
        .replace("example.com", "other.com");
    inputs.insert("yuanme.toml", moved.as_bytes());
    assert_eq!(output(&inputs, "a new origin").len(), 1);
}
