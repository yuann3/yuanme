use std::fs;
use std::path::Path;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use yuanme_engine::{Build, Cache, Mode};

use super::*;
use crate::load::load;
use crate::watch::watch;

const SIDECARS: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../yuanme-engine/tests/sites/eyuan-sidecars"
);
const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/md/eyuan-me");
const PHOTO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../yuanme-assets/fixtures/images/beach.jpg"
);

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

/// A Site the size of the Reference site: eyuan-sidecars' Templates,
/// Sidecars, Scripts and Data files, with the Reference site's 2 Posts and
/// 13 projects from the corpus in place of its 4 stand-ins.
pub(crate) fn reference(root: &Path) {
    copy_dir(Path::new(SIDECARS), root);
    fs::remove_dir_all(root.join("content")).unwrap();
    let mut names: Vec<_> = fs::read_dir(CORPUS)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    names.sort();
    for (i, path) in names.iter().enumerate() {
        let name = path.file_stem().unwrap().to_str().unwrap();
        let text = fs::read_to_string(path).unwrap();
        let (astro, body) = text
            .strip_prefix("---\n")
            .and_then(|t| t.split_once("\n---\n"))
            .unwrap();
        // Astro's front matter, as this fixture's Collections declare it.
        let field = |key: &str| {
            astro
                .lines()
                .find_map(|l| l.strip_prefix(key)?.strip_prefix(':'))
                .unwrap()
                .trim()
                .trim_matches(['\'', '"'])
                .replace('"', "\\\"")
        };
        let (title, description) = (field("title"), field("description"));
        let date = format!("2025-{:02}-01", i % 12 + 1);
        // A relative link to a file the fixture lacks.
        let body = body.replace("(LICENSE)", "(https://example.com/LICENSE)");
        let (dir, slug, extra) = match name.split_once("--").unwrap() {
            ("blog", slug) => ("posts", slug, "tags: [C, Programming]"),
            (_, slug) => ("projects", slug, "stack: [go]"),
        };
        let entry = format!(
            "---\ntitle: \"{title}\"\ndescription: \"{description}\"\ndate: {date}\n{extra}\n---\n{body}"
        );
        write(&root.join(format!("content/{dir}/{slug}.md")), &entry);
    }
}

/// Applies every batch the watcher sends until it has been quiet for a
/// while, returning what each did.
fn settle(session: &mut Session, batches: &Receiver<Batch>) -> Vec<Option<Update>> {
    let mut updates = Vec::new();
    let mut wait = Duration::from_secs(5);
    while let Ok(batch) = batches.recv_timeout(wait) {
        updates.push(session.refresh(&batch).unwrap().then(|| session.build()));
        wait = Duration::from_millis(300);
    }
    updates
}

fn rendered(build: &Build) -> Vec<String> {
    build
        .diagnostics
        .iter()
        .map(|d| d.render_short(&build.sources))
        .collect()
}

/// A clean build of the project as it is on disk now, with dev
/// visibility and a cache of its own.
fn clean(root: &Path) -> Build {
    let inputs = load(root, &root.join("dist")).unwrap();
    yuanme_engine::build(&inputs, &mut Cache::memory("test"), Mode::Dev)
}

/// What the session serves is byte for byte what a clean build makes.
fn assert_served_is_clean(session: &Session, root: &Path, step: &str) {
    let clean = clean(root);
    assert!(!clean.failed(), "{step}: {:#?}", rendered(&clean));
    let served = &session.snapshot().build;
    let files = |b: &Build| -> Vec<(String, blake3::Hash)> {
        b.files.iter().map(|(p, o)| (p.clone(), o.hash)).collect()
    };
    let (ours, theirs) = (files(served), files(&clean));
    let only = |a: &[(String, blake3::Hash)], b: &[(String, blake3::Hash)]| -> Vec<String> {
        a.iter()
            .filter(|f| !b.contains(f))
            .map(|(p, _)| p.clone())
            .collect()
    };
    assert!(
        ours == theirs,
        "{step}: served and clean differ\nonly served: {:?}\nonly clean: {:?}",
        only(&ours, &theirs),
        only(&theirs, &ours)
    );
    assert_eq!(rendered(served), rendered(&clean), "{step}");
    assert_eq!(
        served.images.keys().collect::<Vec<_>>(),
        clean.images.keys().collect::<Vec<_>>(),
        "{step}"
    );
}

/// The equivalence test (ADR 0011): edits replayed through the watcher and
/// the session serve what a clean build of the same files makes. It is
/// what keeps every cache key complete, and what tests the walks a folder
/// change and a lost event need.
#[test]
fn every_edit_serves_what_a_clean_build_makes() {
    let dir = tempfile::tempdir().unwrap();
    // The watcher reports canonical paths, such as /private/var on macOS.
    let root = &dir.path().canonicalize().unwrap();
    reference(root);
    let (_watcher, batches) = watch(root, false).unwrap();
    let mut session =
        Session::start(root.clone(), root.join("dist"), Cache::memory("test")).unwrap();
    assert_served_is_clean(&session, root, "the first build");
    let post = root.join("content/posts/extending_c_stdlib.md");

    // A prose edit changes its Page, and the Pages that list it only if
    // what they show of it changed.
    let text = fs::read_to_string(&post).unwrap();
    write(&post, &format!("{text}\nOne more paragraph.\n"));
    let updates = settle(&mut session, &batches);
    let Some(Some(Update::Built(urls))) = updates.first() else {
        panic!("a prose edit builds once")
    };
    assert_eq!(urls, &["/blog/extending_c_stdlib/"]);
    assert_served_is_clean(&session, root, "a prose edit");

    // A Draft with a new tag, and a page bundle with a photo, in a new
    // folder.
    write(
        &root.join("content/posts/notes/wip.md"),
        "---\ntitle: WIP\ndescription: Not yet\ndate: 2026-01-01\ndraft: true\ntags: [Rust]\n---\nSoon.\n",
    );
    write(
        &root.join("content/posts/notes/sea/index.md"),
        "---\ntitle: Sea\ndescription: Waves\ndate: 2026-01-02\ntags: [Life]\n---\n![A beach](beach.jpg)\n",
    );
    fs::copy(PHOTO, root.join("content/posts/notes/sea/beach.jpg")).unwrap();
    settle(&mut session, &batches);
    assert_served_is_clean(&session, root, "a new folder");
    let served = session.snapshot();
    assert!(served.build.files.contains_key("tags/rust/index.html"));
    assert!(!served.build.images.is_empty(), "the photo's variants");

    // An editor's save: a new file renamed over the old.
    let pew = root.join("content/projects/Pew.md");
    let text = fs::read_to_string(&pew).unwrap();
    write(
        &root.join("content/projects/Pew.md.yuanme~"),
        &text.replace("Pew", "Pew!"),
    );
    fs::rename(root.join("content/projects/Pew.md.yuanme~"), &pew).unwrap();
    settle(&mut session, &batches);
    assert_served_is_clean(&session, root, "a save by rename");

    // A Layout, a Sidecar and a Data file.
    let edit = |path: &str, from: &str, to: &str| {
        let path = root.join(path);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains(from), "{}", path.display());
        write(&path, &text.replacen(from, to, 1));
    };
    edit(
        "templates/layouts/post.html",
        "<article",
        "<article data-edited",
    );
    edit("templates/projects/list.css", "gap: 12px", "gap: 16px");
    edit("data/nav.yaml", "title: About", "title: About me");
    settle(&mut session, &batches);
    assert_served_is_clean(&session, root, "a Template, a Sidecar and a Data file");

    // Files no build reads.
    for dropping in ["#wip.md#", "wip.md~", "4913", ".wip.md.swp"] {
        write(&root.join("content/posts/notes").join(dropping), "x");
    }
    let updates = settle(&mut session, &batches);
    assert!(!updates.is_empty());
    assert!(
        updates.iter().all(Option::is_none),
        "editor droppings built"
    );

    // A folder renamed, and then one inside it, whose events a watcher
    // that kept the old name would report under the old path.
    fs::rename(
        root.join("content/posts/notes"),
        root.join("content/posts/journal"),
    )
    .unwrap();
    settle(&mut session, &batches);
    assert_served_is_clean(&session, root, "a folder renamed");
    fs::rename(
        root.join("content/posts/journal/sea"),
        root.join("content/posts/journal/ocean"),
    )
    .unwrap();
    settle(&mut session, &batches);
    assert_served_is_clean(&session, root, "a folder in it renamed");
    assert!(
        session
            .snapshot()
            .build
            .files
            .contains_key("blog/journal/ocean/index.html")
    );

    // An error keeps the last good build served, and its fix is built.
    let good = session.snapshot();
    let text = fs::read_to_string(&post).unwrap();
    write(&post, &text.replacen("date:", "date: nope\nwhen:", 1));
    let updates = settle(&mut session, &batches);
    let Some(Some(Update::Failed(failed))) = updates.last() else {
        panic!("the error failed the build")
    };
    assert_eq!(rendered(failed), rendered(&clean(root)));
    assert!(Arc::ptr_eq(&good, &session.snapshot()));
    write(&post, &text);
    settle(&mut session, &batches);
    assert_served_is_clean(&session, root, "an error fixed");

    // A removed Entry.
    fs::remove_file(root.join("content/projects/ylib.md")).unwrap();
    settle(&mut session, &batches);
    assert_served_is_clean(&session, root, "an Entry removed");

    // A new ignore file hides an Entry.
    write(&root.join(".gitignore"), "content/projects/shell.md\n");
    settle(&mut session, &batches);
    assert_served_is_clean(&session, root, "an ignore file");
    assert!(
        !session
            .snapshot()
            .build
            .files
            .contains_key("projects/shell/index.html")
    );

    // Changes the watcher lost, as an overflowing queue loses them, are
    // found by a Rescan.
    edit("data/footer_links.yaml", "", "# edited\n");
    fs::remove_file(root.join("content/posts/so_here_we_go_again.md")).unwrap();
    write(&root.join("static/robots.txt"), "User-agent: *\n");
    while batches.recv_timeout(Duration::from_millis(300)).is_ok() {}
    let rescan = Batch {
        paths: BTreeSet::new(),
        rescan: true,
    };
    assert!(session.refresh(&rescan).unwrap());
    assert!(matches!(session.build(), Update::Built(_)));
    assert_served_is_clean(&session, root, "a Rescan");
}

/// #15's Gate for the Reference site: a prose edit rebuilds in at most
/// 50 ms at p95, Engine side: reading the saved file again, building the
/// Site and finding the URLs that changed.
#[test]
#[ignore = "a timing, meaningful only in a release build; CI's dev-budget job runs it"]
fn a_prose_edit_rebuilds_in_at_most_50_ms_p95() {
    let dir = tempfile::tempdir().unwrap();
    let root = &dir.path().canonicalize().unwrap();
    reference(root);
    let mut session =
        Session::start(root.clone(), root.join("dist"), Cache::memory("test")).unwrap();
    let post = root.join("content/posts/extending_c_stdlib.md");
    let text = fs::read_to_string(&post).unwrap();
    let batch = Batch {
        paths: BTreeSet::from([post.clone()]),
        rescan: false,
    };
    let mut times = Vec::new();
    for i in 0..60 {
        write(
            &post,
            &text.replacen("\n\n", &format!("\n\nEdit {i}.\n\n"), 1),
        );
        let started = std::time::Instant::now();
        assert!(session.refresh(&batch).unwrap());
        let Update::Built(urls) = session.build() else {
            panic!("the edit failed the build")
        };
        let took = started.elapsed();
        assert_eq!(urls, ["/blog/extending_c_stdlib/"]);
        // The first edits warm what the first build didn't.
        if i >= 10 {
            times.push(took);
        }
    }
    times.sort();
    let p95 = times[(times.len() * 95).div_ceil(100) - 1];
    eprintln!(
        "{} Pages; a prose edit rebuilt in {:?} at p50, {p95:?} at p95, {:?} at most",
        session.snapshot().build.site.as_ref().unwrap().routes.len(),
        times[times.len() / 2],
        times.last().unwrap()
    );
    assert!(p95 <= Duration::from_millis(50), "{p95:?} at p95");
}
