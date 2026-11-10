//! `[math]`: macros and Theorem kinds.

use std::collections::BTreeMap;

/// `[math]` (#17).
#[derive(Debug)]
pub struct Math {
    /// `[math.macros]`: each macro's name, without its backslash, and its
    /// definition.
    pub macros: Vec<(String, String)>,
    /// Every Theorem kind, by the name its blocks are written with: the
    /// built-in ones as `[math.theorems]` changes them, and the Site's own.
    pub theorems: BTreeMap<String, TheoremKind>,
}

impl Default for Math {
    fn default() -> Self {
        let kind = |label: &str, numbered: bool| TheoremKind {
            label: label.into(),
            counter: numbered.then(|| "theorem".into()),
        };
        let theorems = [
            ("theorem", kind("Theorem", true)),
            ("lemma", kind("Lemma", true)),
            ("proposition", kind("Proposition", true)),
            ("corollary", kind("Corollary", true)),
            ("conjecture", kind("Conjecture", true)),
            ("definition", kind("Definition", true)),
            ("example", kind("Example", true)),
            ("exercise", kind("Exercise", true)),
            ("remark", kind("Remark", false)),
            ("note", kind("Note", false)),
            ("proof", kind("Proof", false)),
        ];
        Math {
            macros: Vec::new(),
            theorems: theorems
                .into_iter()
                .map(|(name, k)| (name.to_string(), k))
                .collect(),
        }
    }
}

/// `[math.theorems.NAME]`.
#[derive(Debug, Clone, PartialEq)]
pub struct TheoremKind {
    /// What its blocks are called on the Page: `Lemma`.
    pub label: String,
    /// The counter its blocks are numbered on, which kinds can share; none
    /// for an unnumbered kind.
    pub counter: Option<String>,
}
