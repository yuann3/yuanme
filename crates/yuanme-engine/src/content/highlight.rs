//! Code blocks highlighted through the leaf cache (ADR 0011), and the
//! stylesheet for the classes the Site's Pages use (#53).

use std::collections::BTreeSet;
use std::ops::Range;
use std::sync::Arc;

use rayon::prelude::*;
use yuanme_content::config;
use yuanme_content::document::{Kind, NodeId};
use yuanme_content::site::Site;
use yuanme_diag::{Applicability, Diagnostic, Edit, FileId, Files, Span, codes};
use yuanme_render::highlight::{self, Code, Language};

use crate::Cache;
use crate::assets::Seen;
use crate::hashing::{Asset, placeholder};

const KIND: &str = "highlight";

/// A code block, the grammar it is highlighted with, and its cache key,
/// which covers both and the Engine version, which pins the grammars.
struct Block<'a> {
    id: NodeId,
    grammar: Option<&'static str>,
    text: &'a str,
    key: blake3::Hash,
}

/// Each of an Entry's code blocks, by the key of its highlighted HTML in
/// the cache.
pub(crate) type Blocks = Vec<(NodeId, blake3::Hash)>;

/// Each Entry's code blocks, highlighted into the cache, and a warning for
/// each block in a language no grammar has, which is written as plain text.
pub(crate) fn code(
    site: &Site,
    sources: &Files,
    cache: &mut Cache,
) -> (Vec<Blocks>, Vec<Diagnostic>) {
    // Every Document is walked on every build, so on threads of their own.
    let walked: Vec<(Vec<Block>, Vec<Diagnostic>)> = site
        .entries
        .par_iter()
        .map(|entry| {
            let doc = &entry.document;
            let mut found = Vec::new();
            let mut diagnostics = Vec::new();
            for &id in doc.all() {
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
            (found, diagnostics)
        })
        .collect();
    let (blocks, diagnostics): (Vec<Vec<Block>>, Vec<Vec<Diagnostic>>) = walked.into_iter().unzip();
    let diagnostics = diagnostics.into_iter().flatten().collect();

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

    let blocks = blocks
        .iter()
        .map(|found| found.iter().map(|b| (b.id, b.key)).collect())
        .collect();
    (blocks, diagnostics)
}

/// An Entry's highlighted code blocks, which [`code`] put in the cache.
pub(crate) fn of(cache: &Cache, blocks: &Blocks) -> Code {
    blocks
        .iter()
        .map(|(id, key)| {
            let bytes = cache.get(KIND, key).expect("highlighted by code()");
            let html = String::from_utf8(bytes.to_vec()).expect("cached as UTF-8");
            (*id, html)
        })
        .collect()
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
pub(crate) fn stylesheet(
    pages: &mut [(String, Vec<u8>)],
    seen: &[Option<Arc<Seen>>],
    assets: &mut Vec<Asset>,
) {
    let mut classes = BTreeSet::new();
    let mut showing = Vec::new();
    for (i, seen) in seen.iter().enumerate() {
        if let Some(used) = seen.as_ref().and_then(|s| s.code.as_ref()) {
            classes.extend(used.iter().map(String::as_str));
            showing.push(i);
        }
    }
    if showing.is_empty() {
        return;
    }
    let css = highlight::css(classes);
    let tag = format!(
        "<link rel=\"stylesheet\" href=\"{}\">",
        placeholder(assets.len())
    );
    assets.push(Asset::Linked {
        name: "code.css".into(),
        bytes: css.into_bytes(),
    });
    // Each moves the rest of its Page, so on threads of their own.
    let showing = pages
        .par_iter_mut()
        .zip(seen)
        .filter(|(_, s)| s.as_ref().is_some_and(|s| s.code.is_some()));
    showing.for_each(|((_, bytes), _)| {
        let at = bytes
            .windows(7)
            .position(|w| w.eq_ignore_ascii_case(b"</head>"))
            .unwrap_or(0);
        bytes.splice(at..at, tag.bytes());
    });
}
