//! Theorem blocks numbered (#17): per Entry, in the order they are written,
//! each on its kind's counter, so kinds that share one count together, as
//! amsthm's `\newtheorem{lemma}[theorem]` does.

use std::collections::BTreeMap;

use crate::config::TheoremKind;
use crate::document::{Document, Kind, NodeId};

/// Fills in each Theorem block's name, such as `Lemma 2`, from `kinds`,
/// which has every kind a block was lowered with, and gives it to the
/// block's Label as what a link to it shows.
pub(crate) fn number(doc: &mut Document, kinds: &BTreeMap<String, TheoremKind>) {
    let mut blocks: Vec<NodeId> = doc
        .descendants(Document::ROOT)
        .into_iter()
        .filter(|&id| matches!(doc.node(id).kind, Kind::Theorem { .. }))
        .collect();
    // Source order: Markdown keeps footnote definitions at the end.
    blocks.sort_by_key(|&id| doc.node(id).span.start);
    let mut counters: BTreeMap<&str, usize> = BTreeMap::new();
    for id in blocks {
        let Kind::Theorem { kind, name } = doc.kind_mut(id) else {
            unreachable!("filtered")
        };
        let k = &kinds[kind.as_str()];
        *name = match &k.counter {
            Some(counter) => {
                let n = counters.entry(counter).or_default();
                *n += 1;
                format!("{} {n}", k.label)
            }
            None => k.label.clone(),
        };
        let name = name.clone();
        let node = doc.node(id);
        if let Some((_, label)) = node.attrs.iter().find(|(a, _)| a == "id") {
            let label = label.clone();
            doc.names.numbers.entry(label).or_insert(name);
        }
    }
}
