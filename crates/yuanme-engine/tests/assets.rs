//! Stylesheets, files and Static files that Pages name (#56), built from
//! fixture projects through the Engine's one seam.

use std::fmt::Write;
use std::path::{Path, PathBuf};

use yuanme_engine::{Body, Build, Cache, Inputs, Mode, build};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/sites")
        .join(name)
}

/// The project at `root`, loaded as the binary loads it: `node_modules`
/// left out, images hashed as a stream.
fn load(root: &Path) -> Inputs {
    let mut inputs = Inputs::at(root.to_path_buf());
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path.strip_prefix(root).unwrap().to_str().unwrap();
            if path.is_dir() {
                if relative != "node_modules" {
                    stack.push(path);
                }
                continue;
            }
            let bytes = std::fs::read(&path).unwrap();
            if relative.ends_with(".png") {
                inputs.insert_streamed(relative, blake3::hash(&bytes));
            } else {
                inputs.insert(relative, bytes);
            }
        }
    }
    inputs
}

fn run(inputs: &Inputs) -> Build {
    build(inputs, &mut Cache::memory("1.0.0"), Mode::Build)
}

/// Every output file: text as it is, other bytes by size and hash.
fn show(build: &Build) -> String {
    assert!(!build.failed(), "{}", diagnostics(build));
    let mut out = String::new();
    for (path, output) in &build.files {
        match &output.body {
            Body::Bytes(bytes) => match std::str::from_utf8(bytes) {
                Ok(text) if !text.contains('\0') => writeln!(out, "=== {path}\n{text}").unwrap(),
                _ => writeln!(out, "=== {path}\n<{} bytes>", bytes.len()).unwrap(),
            },
            Body::Input(input) => writeln!(out, "=== {path}\n<copied from {input}>").unwrap(),
            Body::Cached(_) => unreachable!("no asset is cached yet"),
        }
    }
    out
}

fn diagnostics(build: &Build) -> String {
    build
        .diagnostics
        .iter()
        .map(|d| d.render(&build.sources, false))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn pages_name_hashed_bundles_and_copies_and_get_the_sidecar_stylesheet() {
    insta::assert_snapshot!(show(&run(&load(&fixture("assets")))));
}

/// Copies the folder `from` to `to`.
fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap();
        }
    }
}

#[test]
fn a_stylesheet_imports_packages_through_their_style_condition_or_field() {
    let hoisted = run(&load(&fixture("packages")));
    insta::assert_snapshot!(show(&hoisted));

    // Bun's isolated layout links each package from `node_modules/.bun`,
    // so its files resolve to paths there. The bundle is the same.
    #[cfg(unix)]
    {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        copy_dir(&fixture("packages"), root);
        let modules = root.join("node_modules");
        for (name, store) in [
            ("@fontsource/geist", "@fontsource+geist@5.0.0"),
            ("plain", "plain@1.0.0"),
        ] {
            let real = modules
                .join(".bun")
                .join(store)
                .join("node_modules")
                .join(name);
            std::fs::create_dir_all(real.parent().unwrap()).unwrap();
            std::fs::rename(modules.join(name), &real).unwrap();
            let link = modules.join(name);
            let up = if name.contains('/') {
                "../.bun"
            } else {
                ".bun"
            };
            let target = Path::new(up).join(store).join("node_modules").join(name);
            std::os::unix::fs::symlink(target, link).unwrap();
        }
        let isolated = run(&load(root));
        assert_eq!(show(&isolated), show(&hoisted));
    }
}

/// Files by path, over a fixture's.
type Over = &'static [(&'static str, &'static [u8])];

/// (code, files over the `assets` fixture, the file and text its primary
/// span covers)
const ERRORS: &[(&str, Over, &str, &str)] = &[
    (
        "asset/not-found",
        &[(
            "templates/home.html",
            b"{% extends \"layouts/base.html\" %}{% block head %}<link rel=\"stylesheet\" href=\"/styles/sit.css\">{% endblock %}",
        )],
        "templates/home.html",
        "/styles/sit.css",
    ),
    // A Static file's URL has no `static/`.
    (
        "asset/not-found",
        &[(
            "templates/home.html",
            b"{% extends \"layouts/base.html\" %}{% block head %}<link rel=\"icon\" href=\"/static/favicon.svg\">{% endblock %}",
        )],
        "templates/home.html",
        "/static/favicon.svg",
    ),
    (
        "asset/not-found",
        &[("styles/post.css", b"article {\n  background: url(../img/nope.png);\n}\n")],
        "styles/post.css",
        "../img/nope.png",
    ),
    (
        "asset/not-found",
        &[("styles/post.css", b"\n@import \"missing.css\";\n")],
        "styles/post.css",
        "missing.css",
    ),
    (
        "asset/not-found",
        &[("styles/post.css", b"@import \"no-such-package\";\n")],
        "styles/post.css",
        "no-such-package",
    ),
    (
        "asset/ambiguous-url",
        &[("static/styles/post.css", b"article {}")],
        "templates/posts/entry.html",
        "../../styles/post.css?v=2",
    ),
    (
        "asset/classic-script",
        &[(
            "templates/home.html",
            b"{% extends \"layouts/base.html\" %}{% block body %}<script src=\"/scripts/site.ts\"></script>{% endblock %}",
        )],
        "templates/home.html",
        "/scripts/site.ts",
    ),
    (
        "asset/invalid-css",
        &[("styles/parts/type.css", b"p { color: red }\n\n  a$b { color: red }\n")],
        "styles/parts/type.css",
        "",
    ),
    (
        "asset/invalid-css",
        &[("templates/components/row.css", b".row { content: \"caf\xe9\" }")],
        "templates/components/row.css",
        "",
    ),
];

#[test]
fn each_mistake_is_one_diagnostic_on_the_url_it_is_about() {
    let tested: std::collections::BTreeSet<&str> = ERRORS.iter().map(|(code, ..)| *code).collect();
    let all: std::collections::BTreeSet<&str> = yuanme_diag::codes::ALL
        .iter()
        .map(|c| c.name)
        .filter(|n| n.starts_with("asset/"))
        .collect();
    assert_eq!(tested, all);

    let base = load(&fixture("assets"));
    let wrong: Vec<String> = ERRORS
        .iter()
        .filter_map(|(code, files, file, under)| {
            let mut inputs = base.clone();
            for (path, bytes) in *files {
                inputs.insert(*path, *bytes);
            }
            let build = run(&inputs);
            let got: Vec<_> = build
                .diagnostics
                .iter()
                .map(|d| {
                    let span = d.primary();
                    let text = build.sources.text(span.file);
                    (
                        d.code.name,
                        build.sources.path(span.file),
                        &text[span.range.clone()],
                    )
                })
                .collect();
            let ok = build.failed() && build.files.is_empty() && got == [(*code, *file, *under)];
            (!ok).then(|| {
                format!(
                    "want {code} in {file} on {under:?}\n  got {got:?}\n{}",
                    diagnostics(&build)
                )
            })
        })
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn rendered_asset_diagnostics() {
    let mut inputs = load(&fixture("assets"));
    inputs.insert(
        "templates/home.html",
        b"{% extends \"layouts/base.html\" %}\n{% block head %}<link rel=\"stylesheet\" href=\"/styles/sit.css\">\n<script src=\"/scripts/site.ts\"></script>{% endblock %}\n".as_slice(),
    );
    let build = run(&inputs);
    insta::assert_snapshot!(diagnostics(&build));

    // The suggested `type="module"` makes it a Script, which builds.
    let fix = build
        .diagnostics
        .iter()
        .find(|d| d.code.name == "asset/classic-script")
        .and_then(|d| d.children.iter().flat_map(|c| &c.suggestions).next())
        .expect("a fix");
    let [edit] = fix.edits.as_slice() else {
        panic!("one edit")
    };
    let mut text = build.sources.text(edit.span.file).to_string();
    text.replace_range(edit.span.range.clone(), &edit.replacement);
    let text = text.replace("/styles/sit.css", "/styles/site.css");
    inputs.insert("templates/home.html", text.into_bytes());
    let build = run(&inputs);
    assert!(!build.failed(), "{}", diagnostics(&build));
}
