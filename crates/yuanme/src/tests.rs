use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use yuanme_diag::{Diagnostic, Files, Span, codes};
use yuanme_engine::{Build, Cache, Mode};

use crate::load::load;
use crate::sync::{Report, sync};

fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn build(root: &Path) -> Build {
    let inputs = load(root, &root.join("dist")).unwrap();
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

    let inputs = load(root, &root.join("dist")).unwrap();

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
    write(&root.join("yuanme.toml"), "");
    let file = |i: usize| format!("d{:02}/f{i}.txt", i / 100);
    for i in 0..10_000 {
        write(&root.join("static").join(file(i)), format!("file {i}"));
    }
    write(&root.join("static/lone/only.txt"), "alone");
    write(&root.join("static/cat.png"), "png bytes");

    let report = sync_build(root, &build(root)).unwrap();
    assert_eq!(report.written.len(), 10_002);
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
    write(&root.join("yuanme.toml"), "");
    write(&root.join("static/a.txt"), "a");
    write(&root.join("static/b.txt"), "b");
    sync_build(root, &build(root)).unwrap();
    let before = snapshot(&root.join("dist"));

    // A build that would change one file, add one and delete one, but has an error.
    write(&root.join("static/a.txt"), "a, edited");
    write(&root.join("static/c.txt"), "c");
    fs::remove_file(root.join("static/b.txt")).unwrap();
    let mut failed = build(root);
    let mut sources = Files::default();
    let file = sources.add("content/posts/pew.md", "");
    failed.diagnostics.push(Diagnostic::error(
        &codes::INTERNAL_PANIC,
        "yuanme crashed",
        Span { file, range: 0..0 },
    ));
    failed.sources = sources;

    assert!(sync_build(root, &failed).is_err());
    assert_eq!(snapshot(&root.join("dist")), before);

    // The next good build still sees every change, since the manifest
    // wasn't touched either.
    let report = sync_build(root, &build(root)).unwrap();
    assert_eq!(report.written, ["a.txt", "c.txt"]);
    assert_eq!(report.deleted, ["b.txt"]);
}
