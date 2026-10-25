//! One Label namespace per Entry (ADR 0010): equations' `\label`s, the
//! ids written on headings, and Org's `#+NAME`s and `<<targets>>` each
//! become an id on the Page, so no two may be the same, and each is kept
//! to characters an id and a `#fragment` take as written.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use yuanme_diag::{Applicability, Diagnostic, Edit, FileId, Span, codes};

use crate::document::{Document, Kind};

/// Checks the Labels written in `doc`, and `equations`', which are where
/// math-core found them. Returns every Label, which a generated heading id
/// must not take.
pub(crate) fn check(
    doc: &Document,
    text: &str,
    file: FileId,
    equations: &[(String, Range<usize>)],
    out: &mut Vec<Diagnostic>,
) -> BTreeSet<String> {
    let span = |range: Range<usize>| Span { file, range };
    let mut written: Vec<(String, Range<usize>, &str)> = equations
        .iter()
        .map(|(name, at)| (name.clone(), at.clone(), "an equation"))
        .collect();
    for id in doc.descendants(Document::ROOT) {
        let node = doc.node(id);
        match &node.kind {
            // Generated ids are filled in later, and never take a Label.
            Kind::Heading { id: anchor, .. } if !anchor.is_empty() => {
                written.push((anchor.clone(), node.span.clone(), "a heading"));
            }
            Kind::Target { id: anchor } => {
                written.push((anchor.clone(), node.span.clone(), "a target"));
            }
            _ => {}
        }
        for (name, value) in &node.attrs {
            if name == "id" {
                written.push((value.clone(), node.span.clone(), "an element"));
            }
        }
    }
    written.sort_by_key(|(_, at, _)| at.start);

    let mut seen: BTreeMap<String, (Range<usize>, &str)> = BTreeMap::new();
    for (name, at, what) in written {
        // The Label itself where it is written as it is, so a fix replaces
        // only it.
        let exact = text
            .get(at.clone())
            .and_then(|s| s.find(name.as_str()))
            .map(|i| at.start + i..at.start + i + name.len());
        if !is_valid(&name) {
            let fixed = corrected(&name);
            let message = format!("`{name}` isn't a valid Label");
            let mut d = Diagnostic::error(
                &codes::MATH_INVALID_LABEL,
                message,
                span(exact.clone().unwrap_or(at.clone())),
            )
            .note("a Label is letters, digits, `:`, `_`, `.` and `-`, as it becomes an id");
            d = match &exact {
                Some(exact) if !fixed.is_empty() => d.suggest(
                    format!("use `{fixed}`"),
                    Applicability::MaybeIncorrect,
                    vec![Edit {
                        span: span(exact.clone()),
                        replacement: fixed,
                    }],
                ),
                _ if !fixed.is_empty() => d.help(format!("use `{fixed}`")),
                _ => d,
            };
            out.push(d);
        }
        match seen.get(&name) {
            Some((first, first_what)) => {
                let message = format!("the Label `{name}` is given twice");
                out.push(
                    Diagnostic::error(
                        &codes::MATH_DUPLICATE_LABEL,
                        message,
                        span(exact.unwrap_or(at)),
                    )
                    .label(format!("given again here, to {what}"))
                    .secondary(
                        span(first.clone()),
                        format!("first given here, to {first_what}"),
                    )
                    .help("rename one of them: every Label in an Entry becomes an id on its Page"),
                );
            }
            None => {
                seen.insert(name, (exact.unwrap_or(at), what));
            }
        }
    }
    seen.into_keys().collect()
}

fn is_valid(label: &str) -> bool {
    !label.is_empty()
        && label
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, ':' | '_' | '.' | '-'))
}

/// `label` with each run of characters a Label can't have as one `-`.
fn corrected(label: &str) -> String {
    let mut out = String::new();
    for c in label.trim().chars() {
        if c.is_alphanumeric() || matches!(c, ':' | '_' | '.' | '-') {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}
