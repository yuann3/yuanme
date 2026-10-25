//! Highlighted code (#53) against Shiki 3.19, the Reference site's
//! highlighter: every code block in the corpus, as the Engine reads it,
//! rendered and coloured through the stylesheet, must give each character
//! the colours and font style Shiki's tokens give it (tests/shiki/).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use serde_json::Value;
use sha2::{Digest, Sha256};
use yuanme_content::document::{Document, Kind};
use yuanme_content::{markdown, org};
use yuanme_diag::Files;
use yuanme_render::highlight;

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../corpus");

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(files(&path));
        } else if path.extension().is_some_and(|e| e == "md" || e == "org") {
            out.push(path);
        }
    }
    out.sort();
    out
}

fn document(path: &Path) -> Option<Document> {
    let text = std::fs::read_to_string(path).unwrap();
    let mut sources = Files::default();
    let file = sources.add("x", text.as_str());
    if path.extension().is_some_and(|e| e == "org") {
        return org::document(&sources, file).0;
    }
    let body = text
        .strip_prefix("---\n")
        .and_then(|rest| rest.find("\n---\n"))
        .map_or(0, |end| end + 9);
    markdown::document(&sources, file, body..text.len()).0
}

/// Every code block in the corpus that has a grammar, once, by its
/// grammar and text.
fn blocks() -> BTreeSet<(&'static str, String)> {
    let mut out = BTreeSet::new();
    for path in files(Path::new(CORPUS)) {
        let Some(doc) = document(&path) else { continue };
        for id in doc.descendants(Document::ROOT) {
            if let Kind::CodeBlock { lang, text } = &doc.node(id).kind
                && let Some(grammar) = highlight::grammar(lang.as_deref())
            {
                out.insert((grammar, text.clone()));
            }
        }
    }
    out
}

fn key(grammar: &str, text: &str) -> String {
    let hash = Sha256::digest(format!("{grammar}\n{text}"));
    hash.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

/// For tests/shiki/generate.mjs: the blocks Shiki must tokenize.
#[test]
#[ignore = "run by tests/shiki/generate.mjs"]
fn write_code_blocks() {
    let path = std::env::var("YUANME_CODE_BLOCKS").expect("YUANME_CODE_BLOCKS names the file");
    let list: Vec<Value> = blocks()
        .into_iter()
        .map(|(grammar, text)| {
            serde_json::json!({ "key": key(grammar, &text), "lang": grammar, "code": text })
        })
        .collect();
    std::fs::write(path, serde_json::to_string(&list).unwrap()).unwrap();
}

/// A character's colour and font style bits in the light theme, then in
/// the dark one.
type Style = (String, i64, String, i64);

/// What `css` gives each class, and `None` the text outside any span.
fn styles(css: &str) -> BTreeMap<Option<String>, Style> {
    let (light, dark) = css
        .split_once("@media (prefers-color-scheme:dark){")
        .unwrap();
    let rules = |part: &str| -> BTreeMap<Option<String>, (String, i64)> {
        part.split('}')
            .filter_map(|rule| {
                let (selector, body) = rule.split_once('{')?;
                let class = selector.trim().strip_prefix(".y-code")?;
                let class = class.strip_prefix(" .").map(str::to_string);
                let mut colour = String::new();
                let mut bits = 0;
                for declaration in body.split(';') {
                    match declaration.split_once(':') {
                        Some(("color", c)) => colour = c.to_string(),
                        Some(("font-style", "italic")) => bits |= 1,
                        Some(("font-weight", "bold")) => bits |= 2,
                        Some(("text-decoration", d)) => {
                            bits |= if d.contains("underline") { 4 } else { 0 }
                                | if d.contains("line-through") { 8 } else { 0 }
                        }
                        _ => {}
                    }
                }
                Some((class, (colour, bits)))
            })
            .collect()
    };
    let dark = rules(dark);
    rules(light)
        .into_iter()
        .map(|(class, (lc, lb))| {
            let (dc, db) = dark[&class].clone();
            (class, (lc, lb, dc, db))
        })
        .collect()
}

/// Each character of a block's HTML with the class of its span.
fn characters(html: &str) -> Vec<(char, Option<String>)> {
    let mut out = Vec::new();
    let mut class = None;
    let mut rest = html;
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("<span class=\"") {
            let (name, tail) = after.split_once("\">").unwrap();
            class = Some(name.to_string());
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix("</span>") {
            class = None;
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix("&#x3C;") {
            out.push(('<', class.clone()));
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix("&#x26;") {
            out.push(('&', class.clone()));
            rest = tail;
        } else {
            let c = rest.chars().next().unwrap();
            assert_ne!(c, '<', "unexpected markup: {rest:.40}");
            out.push((c, class.clone()));
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// Character-for-character agreement with Shiki over the corpus, which is
/// #53's bar. Blank characters are left out: their colour can't be seen,
/// and the renderer joins them to the run before them.
#[test]
fn code_is_coloured_as_shiki_colours_it() {
    let fixture: Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/shiki/tokens.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let shiki_styles: Vec<Style> = fixture["styles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s[0].as_str().unwrap().to_string(),
                s[1].as_i64().unwrap(),
                s[2].as_str().unwrap().to_string(),
                s[3].as_i64().unwrap(),
            )
        })
        .collect();
    let blocks: Vec<_> = blocks().into_iter().collect();
    // Each block's first wrong character, if any, and how many it checked.
    let results: Vec<Result<(Option<String>, usize), ()>> = blocks
        .par_iter()
        .map(|(grammar, text)| {
            let lines = fixture["blocks"].get(key(grammar, text)).ok_or(())?;
            Ok(compare(grammar, text, lines, &shiki_styles))
        })
        .collect();
    let missing = results.iter().filter(|r| r.is_err()).count();
    assert_eq!(
        missing, 0,
        "{missing} blocks have no Shiki tokens: run tests/shiki/generate.mjs"
    );
    let checked: usize = results.iter().flatten().map(|(_, n)| n).sum();
    let wrong: Vec<&String> = results
        .iter()
        .flatten()
        .filter_map(|(w, _)| w.as_ref())
        .collect();
    assert!(
        wrong.is_empty(),
        "{} of {} blocks differ from Shiki ({checked} characters checked):\n{}",
        wrong.len(),
        blocks.len(),
        wrong
            .iter()
            .map(|w| w.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(checked > 100_000, "only {checked} characters checked");
}

/// One block's first character whose colours differ from Shiki's, and how
/// many characters it checked.
fn compare(
    grammar: &str,
    text: &str,
    lines: &Value,
    shiki_styles: &[Style],
) -> (Option<String>, usize) {
    let html = highlight::block(Some(grammar), text);
    let pre =
        format!(" class=\"y-code\" tabindex=\"0\" data-language=\"\"><code>{html}</code></pre>");
    let classes = highlight::classes(&pre).unwrap_or_default();
    let css = styles(&highlight::css(classes.iter().copied()));
    let got = characters(&html);
    let want: Vec<(char, &Style)> = text
        .split('\n')
        .zip(lines.as_array().unwrap())
        .flat_map(|(line, runs)| {
            let line = line.strip_suffix('\r').unwrap_or(line);
            let runs = runs.as_array().unwrap();
            let mut chars = line.chars();
            let mut out: Vec<(char, &Style)> = runs
                .chunks(2)
                .flat_map(|run| {
                    let style = &shiki_styles[run[0].as_u64().unwrap() as usize];
                    (&mut chars)
                        .take(run[1].as_u64().unwrap() as usize)
                        .map(move |c| (c, style))
                        .collect::<Vec<_>>()
                })
                .collect();
            out.push(('\n', &shiki_styles[0]));
            out
        })
        .collect();
    let text_got: String = got.iter().map(|(c, _)| *c).collect();
    let text_want: String = want.iter().map(|(c, _)| *c).collect();
    assert_eq!(
        text_got + "\n",
        text_want,
        "{grammar} block {}",
        key(grammar, text)
    );
    let mut checked = 0;
    for (i, ((c, class), (_, style))) in got.iter().zip(&want).enumerate() {
        if c.is_whitespace() {
            continue;
        }
        checked += 1;
        if css[class] != **style {
            let wrong = format!(
                "{grammar} {} at {i} {c:?}: {:?}, Shiki {:?}",
                key(grammar, text),
                css[class],
                style
            );
            return (Some(wrong), checked);
        }
    }
    (None, checked)
}

/// #16's order: a Shiki name or alias, then Org's names for its modes,
/// then `jupyter-X`; the plain names are no grammar and no warning.
#[test]
fn languages_resolve_as_shiki_then_org_name_them() {
    use highlight::Language::{Grammar, Plain};
    let cases = [
        ("rust", Some(Grammar("rust"))),
        ("rs", Some(Grammar("rust"))),
        ("elisp", Some(Grammar("emacs-lisp"))),
        ("sh", Some(Grammar("shellscript"))),
        // Org's, not an alias in Shiki.
        ("C", Some(Grammar("c"))),
        ("C++", Some(Grammar("cpp"))),
        ("screen", Some(Grammar("shellscript"))),
        ("sqlite", Some(Grammar("sql"))),
        ("R", Some(Grammar("r"))),
        ("jupyter-python", Some(Grammar("python"))),
        ("text", Some(Plain)),
        ("example", Some(Plain)),
        ("fundamental", Some(Plain)),
        ("none", Some(Plain)),
        // No grammar has graphviz-dot-mode's language; gnuplot's grammar is
        // GPL, so it isn't embedded (ADR 0014).
        ("dot", None),
        ("gnuplot", None),
        ("jupyter-gnuplot", None),
        ("rsut", None),
    ];
    let got: Vec<_> = cases
        .iter()
        .map(|(name, _)| (*name, highlight::language(name)))
        .collect();
    assert_eq!(got, cases);
}
