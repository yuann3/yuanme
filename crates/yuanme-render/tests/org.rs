//! Org through the parser and the HTML writer: Markdown and Org fixtures
//! that must render the same, Org's own constructs, and the Org corpus
//! (#14).

use std::path::{Path, PathBuf};

use yuanme_content::{markdown, org};
use yuanme_diag::Files;

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/org");

fn files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(files(&path, ext));
        } else if path.extension().is_some_and(|e| e == ext) {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// An Org file's Diagnostics, then its body as HTML.
fn render(name: &str, text: &str) -> String {
    let mut sources = Files::default();
    let file = sources.add(name, text);
    let (doc, diagnostics) = org::document(&sources, file);
    let mut out: String = diagnostics
        .iter()
        .map(|d| d.render(&sources, false) + "\n")
        .collect();
    if let Some(doc) = doc {
        out.push_str(&yuanme_render::html::body(
            &doc,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        ));
    }
    out
}

fn markdown_html(name: &str, text: &str) -> String {
    let mut sources = Files::default();
    let file = sources.add(name, text);
    let (doc, diagnostics) = markdown::document(&sources, file, 0..text.len());
    assert!(diagnostics.is_empty(), "{name}: {diagnostics:?}");
    yuanme_render::html::body(
        &doc.unwrap(),
        &Default::default(),
        &Default::default(),
        &Default::default(),
    )
}

/// A construct both Source formats have renders as Markdown renders it
/// (#16): each `tests/pairs/x.md` and `x.org` give the same HTML.
#[test]
fn org_renders_what_markdown_renders() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pairs");
    let pairs = files(&dir, "md");
    let wrong: Vec<String> = pairs
        .iter()
        .filter_map(|md| {
            let want = markdown_html("a.md", &std::fs::read_to_string(md).unwrap());
            let got = render(
                "a.org",
                &std::fs::read_to_string(md.with_extension("org")).unwrap(),
            );
            (got != want).then(|| format!("{}:\n{got}\nwant:\n{want}\n", md.display()))
        })
        .collect();
    assert!(
        wrong.is_empty(),
        "differ from Markdown:\n{}",
        wrong.join("\n")
    );
    assert_eq!(pairs.len(), 7);
}

/// Org's own constructs, and what it reads but doesn't render.
#[test]
fn org_only_constructs() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/org");
    for path in files(&dir, "org") {
        let name = path.file_stem().unwrap().to_str().unwrap().to_string();
        let text = std::fs::read_to_string(&path).unwrap();
        insta::assert_snapshot!(
            format!("org__{name}"),
            render(&format!("{name}.org"), &text)
        );
    }
}

#[test]
fn every_org_corpus_file() {
    let all = files(Path::new(CORPUS), "org");
    assert_eq!(all.len(), 116);
    for path in all {
        let name = path.strip_prefix(CORPUS).unwrap().to_str().unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        insta::assert_snapshot!(
            format!("corpus_org__{}", name.replace('/', "__")),
            render(name, &text)
        );
    }
}
