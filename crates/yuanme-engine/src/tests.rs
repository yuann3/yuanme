use super::*;

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

    let files = link::link(
        vec![(
            "index.html".into(),
            b"<link href=\"\0asset:1\0\"><p>\0asset:0\0".to_vec(),
        )],
        vec![
            link::Asset::Copy {
                name: "math.woff2".into(),
                body: Body::Bytes(font.clone().into()),
                hash: blake3::hash(&font),
            },
            link::Asset::Linked {
                name: "site.css".into(),
                bytes: b"@font-face{src:url(\0asset:0\0)}".to_vec(),
            },
        ],
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
    let after = |builds: usize| {
        let path = dir.path().join(builds.to_string());
        let mut cache = Cache::open(path.clone(), "v").unwrap();
        let small = key(&cache, "entry", "a");
        let large = key(&cache, "image", "a");
        cache.insert("entry", small, b"x".as_slice());
        cache.insert_large(large, b"x").unwrap();
        for _ in 0..builds {
            build(&project(), &mut cache, Mode::Build);
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
    assert_eq!(after(10), (true, true, true));
    assert_eq!(after(11), (false, false, false));
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
        &blake3::hash(b"a #x{color:red}").to_hex()[..16]
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
        "<h1>Pew</h1><p>Hi.</p>"
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
