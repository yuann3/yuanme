use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use yuanme_content::site::RouteKind;
use yuanme_engine::{Build, Cache, Inputs, Mode};

use crate::docs;
use crate::load::load;
use crate::sync::{Report, sync};

const CONFIG: &str = "[site]\nurl = \"https://example.com\"\n";

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn build(root: &Path) -> Build {
    let inputs = load(root, &[root.join("dist")]).unwrap();
    let mut cache = Cache::open(root.join(".yuanme/cache"), "test").unwrap();
    let build = yuanme_engine::build(&inputs, &mut cache, Mode::Build);
    cache.save().unwrap();
    build
}

fn sync_build(root: &Path, build: &Build) -> std::io::Result<Report> {
    let manifest = root.join(".yuanme/cache/output.manifest");
    sync(build, root, &root.join("dist"), &manifest)
}

/// Every file under `dir` with its bytes and mtime: a file whose mtime
/// didn't move wasn't written.
fn snapshot(dir: &Path) -> BTreeMap<String, (Vec<u8>, SystemTime)> {
    let mut files = BTreeMap::new();
    for entry in ignore::WalkBuilder::new(dir)
        .standard_filters(false)
        .build()
    {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            let path = entry.path().strip_prefix(dir).unwrap();
            let mtime = entry.metadata().unwrap().modified().unwrap();
            let bytes = fs::read(entry.path()).unwrap();
            files.insert(path.to_str().unwrap().replace('\\', "/"), (bytes, mtime));
        }
    }
    files
}

#[test]
fn load_reads_what_a_build_could_use_and_streams_images() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for path in [
        "yuanme.toml",
        "content/posts/pew.md",
        "content/notes/proof.org",
        "templates/post.html",
        "styles/site.css",
        "static/.well-known/security.txt",
        "static/talk.pdf",
        // Editor droppings: an Emacs lock and auto-save, a backup, Vim's probe.
        "content/posts/.#pew.md",
        "content/posts/#pew.md#",
        "content/posts/pew.md~",
        "content/posts/4913",
        "static/.DS_Store",
        // What the Engine wrote, what Bun installed, and what git ignores.
        "dist/index.html",
        ".yuanme/cache/entry.pack",
        "node_modules/katex/katex.css",
        "private/secret.md",
    ] {
        write(&root.join(path), path);
    }
    write(&root.join(".gitignore"), "private/\n");
    write(&root.join("content/posts/IMG_1.JPG"), "jpeg bytes");

    let inputs = load(root, &[root.join("dist")]).unwrap();

    let paths: Vec<_> = inputs.iter().map(|(path, _)| path).collect();
    assert_eq!(
        paths,
        [
            "content/notes/proof.org",
            "content/posts/IMG_1.JPG",
            "content/posts/pew.md",
            "static/.well-known/security.txt",
            "static/talk.pdf",
            "styles/site.css",
            "templates/post.html",
            "yuanme.toml",
        ]
    );
    let input = |path| inputs.iter().find(|(p, _)| *p == path).unwrap().1;
    let pew = input("content/posts/pew.md");
    assert_eq!(
        pew.bytes.as_deref(),
        Some(b"content/posts/pew.md".as_slice())
    );
    let image = input("content/posts/IMG_1.JPG");
    assert_eq!(image.bytes, None);
    assert_eq!(image.hash, blake3::hash(b"jpeg bytes"));
}

#[test]
fn sync_writes_only_what_changed_in_ten_thousand_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let out = root.join("dist");
    write(&root.join("yuanme.toml"), CONFIG);
    let file = |i: usize| format!("d{:02}/f{i}.txt", i / 100);
    for i in 0..10_000 {
        write(&root.join("static").join(file(i)), format!("file {i}"));
    }
    write(&root.join("static/lone/only.txt"), "alone");
    write(&root.join("static/cat.png"), "png bytes");

    let report = sync_build(root, &build(root)).unwrap();
    // And the default adapter's `_headers`.
    assert_eq!(report.written.len(), 10_003);
    assert_eq!(fs::read(out.join("cat.png")).unwrap(), b"png bytes");
    assert_eq!(fs::read(out.join(file(7))).unwrap(), b"file 7");

    let unchanged = sync_build(root, &build(root)).unwrap();
    assert_eq!(unchanged, Report::default());

    // Change 10 files: only those 10 are written.
    let changed: Vec<String> = (0..10).map(|i| file(i * 1_000 + 7)).collect();
    for path in &changed {
        write(&root.join("static").join(path), format!("{path} edited"));
    }
    let before = snapshot(&out);
    let report = sync_build(root, &build(root)).unwrap();
    let after = snapshot(&out);
    let mut expected = changed.clone();
    expected.sort();
    assert_eq!(report.written, expected);
    for (path, (bytes, mtime)) in &after {
        if changed.contains(path) {
            assert_eq!(bytes, format!("{path} edited").as_bytes());
        } else {
            assert_eq!(
                (bytes, mtime),
                (&before[path].0, &before[path].1),
                "{path} was rewritten"
            );
        }
    }

    // Files edited or deleted outside the Engine are rewritten; a file the
    // Engine never wrote is left alone; a deleted input's file goes, and so
    // does the directory it leaves empty.
    write(&out.join(file(1)), "edited by hand");
    fs::remove_file(out.join(file(2))).unwrap();
    write(&out.join("notes.txt"), "mine");
    fs::remove_file(root.join("static/lone/only.txt")).unwrap();
    let report = sync_build(root, &build(root)).unwrap();
    assert_eq!(
        report,
        Report {
            written: vec![file(1), file(2)],
            deleted: vec!["lone/only.txt".into()],
            unknown: vec!["notes.txt".into()],
        }
    );
    assert_eq!(fs::read(out.join(file(1))).unwrap(), b"file 1");
    assert_eq!(fs::read(out.join(file(2))).unwrap(), b"file 2");
    assert_eq!(fs::read(out.join("notes.txt")).unwrap(), b"mine");
    assert!(!out.join("lone").exists());
}

#[test]
fn a_failed_build_leaves_the_output_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(&root.join("yuanme.toml"), CONFIG);
    write(&root.join("static/a.txt"), "a");
    write(&root.join("static/b.txt"), "b");
    sync_build(root, &build(root)).unwrap();
    let before = snapshot(&root.join("dist"));

    // A build that would change one file, add one and delete one, but whose
    // Site config has an error.
    write(&root.join("static/a.txt"), "a, edited");
    write(&root.join("static/c.txt"), "c");
    fs::remove_file(root.join("static/b.txt")).unwrap();
    write(
        &root.join("yuanme.toml"),
        format!("{CONFIG}titel = \"x\"\n"),
    );
    let failed = build(root);

    assert!(failed.failed());
    assert!(sync_build(root, &failed).is_err());
    assert_eq!(snapshot(&root.join("dist")), before);

    // The next good build still sees every change, since the manifest
    // wasn't touched either.
    write(&root.join("yuanme.toml"), CONFIG);
    let report = sync_build(root, &build(root)).unwrap();
    assert_eq!(report.written, ["a.txt", "c.txt"]);
    assert_eq!(report.deleted, ["b.txt"]);
}

#[test]
fn an_empty_collection_folder_on_disk_exists() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(
        &root.join("yuanme.toml"),
        format!("{CONFIG}[collections.posts]\nroute = \"/blog/{{slug}}/\"\n"),
    );
    let codes = || {
        build(root)
            .diagnostics
            .iter()
            .map(|d| d.code.name)
            .collect::<Vec<_>>()
    };

    assert_eq!(codes(), ["config/invalid-value"]);
    fs::create_dir_all(root.join("content/posts")).unwrap();
    assert_eq!(codes(), [] as [&str; 0]);
}

/// A project in memory: files by path, and folders that exist empty.
#[derive(Clone, Default)]
struct Project {
    files: BTreeMap<String, String>,
    dirs: BTreeSet<String>,
}

impl Project {
    fn build(&self) -> Build {
        let mut inputs = Inputs::default();
        for (path, text) in &self.files {
            inputs.insert(path.clone(), text.as_bytes());
        }
        for dir in &self.dirs {
            inputs.insert_dir(dir.clone());
        }
        yuanme_engine::build(&inputs, &mut Cache::memory("test"), Mode::Build)
    }

    /// Builds, first adding what an example's Site config names and no
    /// example wrote: an empty folder for each Collection and an empty
    /// Template for each Route. An example is one complete file, so the
    /// project supplies the rest.
    fn build_with_stubs(&mut self) -> Build {
        loop {
            let build = self.build();
            let before = (self.files.len(), self.dirs.len());
            if let Some(config) = &build.config {
                for collection in config.collections.values() {
                    self.dirs.insert(collection.dir.clone());
                }
                if let Some(site) = &build.site {
                    let templates = yuanme_render::template::route_templates(config, site);
                    for (route, template) in site.routes.iter().zip(templates) {
                        let builtin =
                            matches!(route.kind, RouteKind::Feed { .. }) && template == "rss.xml";
                        if !builtin {
                            self.files
                                .entry(format!("templates/{template}"))
                                .or_default();
                        }
                    }
                }
            }
            if before == (self.files.len(), self.dirs.len()) {
                return build;
            }
        }
    }
}

fn codes_of(build: &Build) -> Vec<&'static str> {
    let codes: BTreeSet<_> = build.diagnostics.iter().map(|d| d.code.name).collect();
    codes.into_iter().collect()
}

fn rendered(build: &Build) -> String {
    let lines: Vec<String> = build
        .diagnostics
        .iter()
        .map(|d| d.render_short(&build.sources))
        .collect();
    lines.join("\n")
}

/// A fenced block a docs page tests (#38): ```` ```LANG check [PATH] ````
/// must check clean, and ```` ```LANG fail CODE [PATH] ```` must report
/// exactly CODE.
#[derive(Debug)]
struct Example {
    line: usize,
    path: String,
    fails_with: Option<String>,
    text: String,
}

fn examples(page: &str, markdown: &str) -> Vec<Example> {
    let mut out = Vec::new();
    let mut lines = markdown.lines().enumerate();
    while let Some((n, line)) = lines.next() {
        let fence: String = line
            .chars()
            .take_while(|c| *c == '`' || *c == '~')
            .collect();
        if fence.len() < 3 {
            continue;
        }
        let words: Vec<&str> = line[fence.len()..].split_whitespace().collect();
        let mut text = String::new();
        for (_, line) in lines.by_ref() {
            if line.starts_with(&fence) && line[fence.len()..].trim().is_empty() {
                break;
            }
            text.push_str(line);
            text.push('\n');
        }
        let (fails_with, path) = match words.as_slice() {
            [_, "check", rest @ ..] => (None, rest),
            [_, "fail", code, rest @ ..] => (Some(code.to_string()), rest),
            _ => continue,
        };
        let path = match (path, words[0]) {
            ([path], _) => path.to_string(),
            ([], "toml") => "yuanme.toml".into(),
            ([], "md") => "content/posts/example.md".into(),
            ([], "org") => "content/posts/example.org".into(),
            _ => panic!("{page}:{}: say which file the example is", n + 1),
        };
        out.push(Example {
            line: n + 1,
            path,
            fails_with,
            text,
        });
    }
    out
}

/// What an example starts from: the Site that `getting-started` makes,
/// before its Templates and Entries.
const BASE_CONFIG: &str = r#"[site]
url = "https://example.com"
title = "Example"

[collections.posts]
route = "/blog/{slug}/"
list_route = "/blog/"
"#;

/// Each page's examples run in order in one project, so a page can build
/// on what it showed earlier: a `check` example stays in it, and a `fail`
/// one is taken out again.
#[test]
fn docs_examples_check_as_they_say() {
    let mut tested = 0;
    let mut failed = Vec::new();
    for name in docs::names().filter(|n| *n != "index") {
        let page = docs::get(name).unwrap();
        let mut project = Project::default();
        project
            .files
            .insert("yuanme.toml".into(), BASE_CONFIG.into());
        for example in examples(name, &page.body) {
            let at = format!(
                "docs {name}, the example on line {} after the front matter ({})",
                example.line, example.path
            );
            tested += 1;
            match &example.fails_with {
                None => {
                    project.files.insert(example.path, example.text);
                    let build = project.build_with_stubs();
                    if !build.diagnostics.is_empty() {
                        failed.push(format!("{at} should check clean:\n{}", rendered(&build)));
                    }
                }
                Some(code) => {
                    let mut failing = project.clone();
                    failing.files.insert(example.path, example.text);
                    let build = failing.build();
                    if codes_of(&build) != [code.as_str()] {
                        failed.push(format!(
                            "{at} should report only {code}:\n{}",
                            rendered(&build)
                        ));
                    }
                }
            }
        }
    }
    assert!(tested > 0, "no examples found");
    assert!(failed.is_empty(), "{}", failed.join("\n\n"));
}

/// The docs site's copy of every page but the index (#38), by path from
/// `site/content/`, with front matter for the site's Collections.
fn site_pages() -> BTreeMap<String, String> {
    docs::names()
        .filter(|n| *n != "index")
        .map(|name| {
            let page = docs::get(name).unwrap();
            let path = match name.contains('/') {
                true => format!("codes/{name}.md"),
                false => format!("docs/{name}.md"),
            };
            let quote = |s: &str| serde_json::to_string(s).unwrap();
            let text = format!(
                "---\ntitle: {}\ndescription: {}\n---\n\n{}",
                quote(&page.title),
                quote(&page.description),
                page.body
            );
            (path, text)
        })
        .collect()
}

/// `cargo xtask docs` runs this with `YUANME_UPDATE_DOCS=1` to write them.
#[test]
fn the_docs_site_has_every_page_as_this_binary_makes_it() {
    let content = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../site/content");
    let want = site_pages();
    if std::env::var_os("YUANME_UPDATE_DOCS").is_some() {
        for dir in ["docs", "codes"] {
            match fs::remove_dir_all(content.join(dir)) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => panic!("{e}"),
                _ => {}
            }
        }
        for (path, text) in &want {
            write(&content.join(path), text);
        }
    }
    let mut have = BTreeMap::new();
    for dir in ["docs", "codes"] {
        if content.join(dir).exists() {
            for (path, (bytes, _)) in snapshot(&content.join(dir)) {
                have.insert(format!("{dir}/{path}"), String::from_utf8(bytes).unwrap());
            }
        }
    }
    let paths: BTreeSet<&String> = want.keys().chain(have.keys()).collect();
    let stale: Vec<&&String> = paths
        .iter()
        .filter(|p| want.get(**p) != have.get(**p))
        .collect();
    assert!(
        stale.is_empty(),
        "site/content/ is stale; run `cargo xtask docs`: {stale:?}"
    );

    // #21's budget for `.yuanme/docs/`.
    let size: usize = docs::names().map(|n| docs::page(n).unwrap().len()).sum();
    assert!(size <= 400_000, "the docs are {size} bytes");
}

/// The pages are yuanme Markdown, which the docs site builds: no math by
/// accident in a `$`, and every link between pages leads somewhere.
#[test]
fn the_docs_site_pages_build_clean() {
    let mut project = Project::default();
    project.files.insert(
        "yuanme.toml".into(),
        "[site]\nurl = \"https://example.com\"\n\n\
         [collections.docs]\nroute = \"/docs/{slug}/\"\n\
         fields = { description = { type = \"string\" } }\n\n\
         [collections.codes]\nroute = \"/codes/{slug}/\"\n\
         fields = { description = { type = \"string\" } }\n"
            .into(),
    );
    for (path, text) in site_pages() {
        project.files.insert(format!("content/{path}"), text);
    }
    let build = project.build_with_stubs();
    assert!(build.diagnostics.is_empty(), "{}", rendered(&build));
}
