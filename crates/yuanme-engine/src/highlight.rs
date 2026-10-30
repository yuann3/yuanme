//! Code blocks highlighted through the leaf cache (ADR 0011), and the
//! stylesheet for the classes the Site's Pages use (#53).

use std::collections::BTreeSet;
use std::ops::Range;

use rayon::prelude::*;
use yuanme_content::config;
use yuanme_content::document::{Document, Kind, NodeId};
use yuanme_content::site::Site;
use yuanme_diag::{Applicability, Diagnostic, Edit, FileId, Files, Span, codes};
use yuanme_render::highlight::{self, Code, Language};

use crate::Cache;
use crate::link::Asset;

const KIND: &str = "highlight";

/// A code block, the grammar it is highlighted with, and its cache key,
/// which covers both and the Engine version, which pins the grammars.
struct Block<'a> {
    id: NodeId,
    grammar: Option<&'static str>,
    text: &'a str,
    key: blake3::Hash,
}

/// Each Entry's highlighted code blocks, and a warning for each block in a
/// language no grammar has, which is written as plain text.
pub(crate) fn code(
    site: &Site,
    sources: &Files,
    cache: &mut Cache,
) -> (Vec<Code>, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let mut blocks: Vec<Vec<Block>> = Vec::new();
    for entry in &site.entries {
        let doc = &entry.document;
        let mut found = Vec::new();
        for id in doc.descendants(Document::ROOT) {
            let Kind::CodeBlock { lang, text } = &doc.node(id).kind else {
                continue;
            };
            let grammar = match lang.as_deref().map(|l| (l, highlight::language(l))) {
                None | Some((_, Some(Language::Plain))) => None,
                Some((_, Some(Language::Grammar(g)))) => Some(g),
                Some((lang, None)) => {
                    let span = doc.node(id).span.clone();
                    diagnostics.push(unknown(sources, entry.file, span, lang));
                    None
                }
            };
            let mut key = cache.key(KIND);
            let name = grammar.unwrap_or_default();
            key.update(&(name.len() as u64).to_le_bytes());
            key.update(name.as_bytes());
            key.update(text.as_bytes());
            let key = key.finalize();
            found.push(Block {
                id,
                grammar,
                text,
                key,
            });
        }
        blocks.push(found);
    }

    let mut missing: Vec<&Block> = blocks
        .iter()
        .flatten()
        .filter(|b| cache.get(KIND, &b.key).is_none())
        .collect();
    // By grammar: a thread's highlighter compiles a grammar again after
    // it highlights another.
    missing.sort_unstable_by_key(|b| (b.grammar, *b.key.as_bytes()));
    missing.dedup_by_key(|b| b.key);
    let highlighted: Vec<(blake3::Hash, String)> = missing
        .into_par_iter()
        .map(|b| (b.key, highlight::block(b.grammar, b.text)))
        .collect();
    for (key, html) in highlighted {
        cache.insert(KIND, key, html.into_bytes());
    }

    let code = blocks
        .iter()
        .map(|found| {
            found
                .iter()
                .map(|b| {
                    let bytes = cache.get(KIND, &b.key).expect("highlighted above");
                    let html = String::from_utf8(bytes.to_vec()).expect("cached as UTF-8");
                    (b.id, html)
                })
                .collect()
        })
        .collect();
    (code, diagnostics)
}

/// The warning for a code block, at `block` in `file`, whose language no
/// grammar has. It points at the name on the block's first line.
fn unknown(sources: &Files, file: FileId, block: Range<usize>, lang: &str) -> Diagnostic {
    let first = sources.text(file)[block.clone()]
        .lines()
        .next()
        .unwrap_or_default();
    let at = first.find(lang).map_or(block.start..block.start, |i| {
        block.start + i..block.start + i + lang.len()
    });
    let span = Span {
        file,
        range: at.clone(),
    };
    let d = Diagnostic::warning(
        &codes::ENTRY_UNKNOWN_LANGUAGE,
        format!("no grammar highlights `{lang}`, so this code block is plain text"),
        span.clone(),
    );
    match config::similar(lang, highlight::names()) {
        Some(near) if !at.is_empty() => d.suggest(
            format!("did you mean `{near}`?"),
            Applicability::MaybeIncorrect,
            vec![Edit {
                span,
                replacement: near.into(),
            }],
        ),
        _ => d,
    }
}

/// The stylesheet for every highlighted class the Pages use, as an asset,
/// with a `<link>` to it at the end of the head of each HTML Page that
/// shows code, as math's stylesheet goes only to Pages with math (#17).
/// It comes after the assets stage's stylesheets, as a Sidecar's does.
pub(crate) fn stylesheet(pages: &mut [(String, Vec<u8>)], assets: &mut Vec<Asset>) {
    // Every Page is read on every build, so on threads of their own.
    let used: Vec<Option<BTreeSet<&str>>> = pages
        .par_iter()
        .map(|(path, bytes)| {
            if !path.ends_with(".html") {
                return None;
            }
            highlight::classes(std::str::from_utf8(bytes).expect("a Page is UTF-8"))
        })
        .collect();
    let mut classes = BTreeSet::new();
    let mut showing = Vec::new();
    for (i, used) in used.into_iter().enumerate() {
        if let Some(used) = used {
            classes.extend(used.into_iter().map(str::to_string));
            showing.push(i);
        }
    }
    if showing.is_empty() {
        return;
    }
    let css = highlight::css(classes.iter().map(String::as_str));
    let tag = format!(
        "<link rel=\"stylesheet\" href=\"\0asset:{}\0\">",
        assets.len()
    );
    assets.push(Asset::Linked {
        name: "code.css".into(),
        bytes: css.into_bytes(),
    });
    for i in showing {
        let bytes = &mut pages[i].1;
        let at = bytes
            .windows(7)
            .position(|w| w.eq_ignore_ascii_case(b"</head>"))
            .unwrap_or(0);
        bytes.splice(at..at, tag.bytes());
    }
}
