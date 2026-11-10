//! `[math]`: macros and Theorem kinds.

use std::collections::BTreeMap;

use toml::de::DeValue;

use super::check::Checker;
use super::{Table, as_table, entries, string, table};

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

pub(super) fn read(math: Option<&Table<'_>>, checker: &mut Checker) -> Math {
    let macros: Vec<(String, String)> = entries(math.and_then(|m| table(m, "macros")))
        .iter()
        .map(|(name, value)| {
            let definition = value.get_ref().as_str().expect("checked as a string");
            (name.get_ref().to_string(), definition.to_string())
        })
        .collect();
    if let Err((i, message)) = crate::math::converter(&macros) {
        let (name, value) = entries(math.and_then(|m| table(m, "macros")))[i];
        let message = format!("the macro `\\{}` can't be read: {message}", name.get_ref());
        checker.invalid(message, value.span());
    }
    let mut theorems = Math::default().theorems;
    for (name, t) in entries(math.and_then(|m| table(m, "theorems"))) {
        let t = as_table(t);
        let name: &str = name.get_ref().as_ref();
        let default = theorems.remove(name).unwrap_or_else(|| {
            let mut chars = name.chars();
            let label = chars
                .next()
                .map_or_else(String::new, |c| c.to_uppercase().chain(chars).collect());
            TheoremKind {
                label,
                counter: Some("theorem".into()),
            }
        });
        let counter = match t.get("counter").map(|c| c.get_ref()) {
            Some(DeValue::String(c)) => Some(c.to_string()),
            Some(_) => None,
            None => default.counter,
        };
        let kind = TheoremKind {
            label: string(t, "label").unwrap_or(default.label),
            counter,
        };
        theorems.insert(name.to_string(), kind);
    }
    Math { macros, theorems }
}
