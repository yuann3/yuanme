//! The Markdown corpus (#14) through the parser and the HTML writer.

use std::path::{Path, PathBuf};

use yuanme_content::markdown;
use yuanme_diag::Files;
use yuanme_render::highlight;

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus/md");

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(files(&path));
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// A file's body as HTML, or its Diagnostics if it fails.
fn render(path: &Path) -> String {
    render_with(path, false)
}

fn render_with(path: &Path, highlighted: bool) -> String {
    let text = std::fs::read_to_string(path).unwrap();
    // Front matter is the Entry's, not the body's.
    let body = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.find("\n---\n"))
        .map_or(0, |end| end + 9);
    let mut sources = Files::default();
    let name = path.strip_prefix(CORPUS).unwrap_or(path).to_str().unwrap();
    let file = sources.add(name, text.as_str());
    match markdown::document(&sources, file, body..text.len()) {
        (Some(doc), _) => {
            let code = match highlighted {
                true => highlight::document(&doc),
                false => Default::default(),
            };
            yuanme_render::html::body(&doc, &Default::default(), &code)
        }
        (None, diagnostics) => diagnostics
            .iter()
            .map(|d| d.render(&sources, false) + "\n")
            .collect(),
    }
}

#[test]
fn every_corpus_file() {
    for path in files(Path::new(CORPUS)) {
        let name = path
            .strip_prefix(CORPUS)
            .unwrap()
            .to_str()
            .unwrap()
            .replace('/', "__");
        insta::assert_snapshot!(name, render(&path));
    }
}

/// Each `<pre>` as only the text of its code, as the parity test compares
/// highlighted code (docs/parity-exceptions.md); its colours are
/// tests/highlight.rs's.
fn pre_as_text(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    while let Some(start) = rest.find("<pre") {
        out.push_str(&rest[..start]);
        let end = start + rest[start..].find("</pre>").unwrap() + 6;
        let pre = &rest[start..end];
        let code = &pre[pre.find("<code").unwrap()..pre.rfind("</code>").unwrap()];
        let mut text = String::new();
        for (i, part) in code.split('<').enumerate() {
            text.push_str(if i == 0 {
                part
            } else {
                part.split_once('>').unwrap().1
            });
        }
        out.push_str(&format!(
            "<pre>{}</pre>",
            text.strip_suffix('\n').unwrap_or(&text)
        ));
        rest = &rest[end..];
    }
    out + rest
}

/// The Reference site's Posts render byte for byte as Astro renders them
/// (tests/astro/generate.mjs), and highlighted, their code has Astro's text.
#[test]
fn reference_posts_render_as_astro_does() {
    let dir = Path::new(CORPUS).join("eyuan-me");
    let wrong: Vec<String> = files(&dir)
        .iter()
        .filter_map(|path| {
            let name = path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .replace(".md", ".html");
            let astro = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/astro")
                .join(&name);
            let want = std::fs::read_to_string(astro).unwrap();
            let highlighted = pre_as_text(&render_with(path, true)) != pre_as_text(&want);
            (render(path) != want || highlighted).then_some(name)
        })
        .collect();
    assert!(wrong.is_empty(), "differ from Astro: {wrong:?}");
    assert_eq!(files(&dir).len(), 15);
}

/// Edge cases render byte for byte as Astro renders them
/// (tests/astro/generate.mjs).
#[test]
fn edge_cases_render_as_astro_does() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/astro/cases");
    let cases = files(&dir);
    let wrong: Vec<String> = cases
        .iter()
        .filter_map(|path| {
            let want = std::fs::read_to_string(path.with_extension("html")).unwrap();
            let got = render(path);
            (got != want).then(|| format!("{}:\n{got}\nwant:\n{want}", path.display()))
        })
        .collect();
    assert!(wrong.is_empty(), "differ from Astro:\n{}", wrong.join("\n"));
    assert_eq!(cases.len(), 7);
}
