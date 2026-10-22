use super::*;

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
    let mut inputs = Inputs::default();
    inputs.insert("static/robots.txt", b"User-agent: *".as_slice());
    inputs.insert("static/static/x.css", b"a{}".as_slice());
    inputs.insert_streamed("static/img/cat.jpg", blake3::hash(b"cat"));
    inputs.insert("content/posts/pew.md", b"# Pew".as_slice());
    inputs.insert("yuanme.toml", b"".as_slice());

    let build = build(&inputs, &mut Cache::memory("v"), Mode::Build);

    assert!(!build.failed());
    let paths: Vec<_> = build.files.keys().map(String::as_str).collect();
    assert_eq!(paths, ["img/cat.jpg", "robots.txt", "static/x.css"]);
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
fn link_names_assets_by_the_hash_of_their_final_bytes() {
    let font = b"FONT".to_vec();
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
            link::Asset {
                name: "math.woff2".into(),
                bytes: font.clone(),
            },
            link::Asset {
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
            build(&Inputs::default(), &mut cache, Mode::Build);
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
    build(&Inputs::default(), &mut cache, Mode::Build);
    cache.save().unwrap();
    assert_eq!(inode(), written);

    // Reading an entry records its use, which is a change.
    let mut cache = Cache::open(dir.path().into(), "v").unwrap();
    cache.get("entry", &a);
    cache.save().unwrap();
    assert_ne!(inode(), written);
}
