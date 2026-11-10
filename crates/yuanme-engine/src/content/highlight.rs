//! Code blocks highlighted through the leaf cache (ADR 0011). The
//! stylesheet for their classes is the assets' (`assets/styles.rs`).

use std::ops::Range;

use rayon::prelude::*;
use yuanme_content::{Kind, NodeId, Site, similar};
use yuanme_diag::{Applicability, Diagnostic, Edit, FileId, Files, Span, codes};
use yuanme_render::{Code, Language, highlight_block, language, language_names};

use crate::Cache;

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
                let grammar = match lang.as_deref().map(|l| (l, language(l))) {
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
        .map(|b| (b.key, highlight_block(b.grammar, b.text)))
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
            let bytes = cache
                .get(KIND, key)
                .expect("code() inserted every block's key before bodies read them");
            let html =
                String::from_utf8(bytes.to_vec()).expect("code() caches the highlighter's String");
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
    match similar(lang, language_names()) {
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
