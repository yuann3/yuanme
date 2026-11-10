//! Where each link to another file or a fragment goes.

use std::collections::{BTreeMap, BTreeSet};

use rayon::prelude::*;
use yuanme_diag::{Diagnostic, Span, codes};

use super::{Route, RouteKind};
use crate::document::{Kind, Link, Links};
use crate::entry::{Entry, SourceFormat};

/// Resolves every link to another file or a fragment (#16). A link to an
/// Entry's source goes to its Route, and one that leads nowhere, such as to
/// a Draft `left_out` of the build, is an error; a URL or a `/` path is
/// left as written.
pub(super) fn links(
    entries: &[Entry],
    routes: &[Route],
    left_out: &BTreeSet<String>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Links> {
    let url_of: BTreeMap<usize, &str> = routes
        .iter()
        .filter_map(|r| match r.kind {
            RouteKind::Entry(i) => Some((i, r.url.as_str())),
            _ => None,
        })
        .collect();
    let by_path: BTreeMap<&str, usize> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.path.as_str(), i))
        .collect();
    // Every id a fragment can name: headings', and Org's targets and names.
    let anchors: Vec<Vec<String>> = entries
        .par_iter()
        .map(|e| {
            let headings = e.document.headings().iter().map(|h| h.id.clone());
            headings
                .chain(e.document.names.targets.values().cloned())
                .collect()
        })
        .collect();
    // Where an Org `::search` finds a headline: Org's titles as written, and
    // a Markdown heading's text.
    let titles: Vec<BTreeMap<String, String>> = entries
        .par_iter()
        .map(|e| match e.format {
            SourceFormat::Org => e.document.names.titles.clone(),
            SourceFormat::Markdown => {
                let headings = e.document.headings().iter().rev();
                headings.map(|h| (squash(&h.text), h.id.clone())).collect()
            }
        })
        .collect();
    // The Site-wide `:ID:` index: an Entry, and a heading's id or nothing.
    let mut ids: BTreeMap<&str, (usize, &str, Span)> = BTreeMap::new();
    for (i, e) in entries.iter().enumerate() {
        for (id, anchor, at) in &e.document.names.ids {
            let span = Span {
                file: e.file,
                range: at.clone(),
            };
            if let Some((_, _, first)) = ids.get(id.as_str()) {
                diagnostics.push(
                    Diagnostic::error(
                        &codes::ORG_DUPLICATE_ID,
                        format!("the ID `{id}` is given twice"),
                        span,
                    )
                    .label("given again here")
                    .secondary(first.clone(), "first given here")
                    .help("give one of them a new ID; in Emacs, `org-id-get-create` makes one"),
                );
                continue;
            }
            ids.insert(id, (i, anchor, span));
        }
    }
    // Each Entry on a thread of its own, since this walks every node of
    // every Document on every build.
    let found: Vec<(Links, Vec<Diagnostic>)> = entries
        .par_iter()
        .enumerate()
        .map(|(i, e)| {
            let mut reported = Vec::new();
            let doc = &e.document;
            let org = e.format == SourceFormat::Org;
            let mut out = Links::new();
            for &id in doc.all() {
                let node = doc.node(id);
                let Kind::Link { url, .. } = &node.kind else { continue };
                let span = Span { file: e.file, range: node.span.clone() };
                if org && let Some(key) = url.strip_prefix("id:") {
                    match ids.get(key) {
                        Some((j, anchor, _)) => {
                            let route = url_of[j];
                            let href = if anchor.is_empty() { route.to_string() } else { format!("{route}#{anchor}") };
                            out.insert(id, Link { href, text: None });
                        }
                        None => reported.push(
                            Diagnostic::error(&codes::LINK_UNRESOLVED, format!("no Entry or heading has the ID `{key}`"), span)
                                .help("an `id:` link leads to the Entry or headline whose `:PROPERTIES:` drawer has that `:ID:`"),
                        ),
                    }
                    continue;
                }
                // Org's `file:a.org::*Title` and `::name` search the other file.
                let search = url
                    .split_once("::")
                    .filter(|(p, _)| org && !is_external(p));
                if search.is_none() && is_external(url) {
                    continue;
                }
                let (path, fragment) = match search {
                    Some((p, _)) => (p, None),
                    None => url.split_once('#').map_or((url.as_str(), None), |(p, f)| (p, Some(f))),
                };
                let target = if path.is_empty() {
                    i
                } else {
                    let dir = e.path.rsplit_once('/').map_or("", |(d, _)| d);
                    let resolved = normalize(&format!("{dir}/{}", percent_decode(path)));
                    match by_path.get(resolved.as_str()) {
                        Some(&j) => j,
                        None if left_out.contains(&resolved) => {
                            let message = format!("`{resolved}` is a Draft, which this build leaves out");
                            reported.push(
                                Diagnostic::error(&codes::LINK_UNRESOLVED, message, span)
                                    .help("publish it by removing `draft: true`, or remove the link until it is"),
                            );
                            continue;
                        }
                        None => {
                            let message = format!("no Entry is written in `{resolved}`");
                            reported.push(
                                Diagnostic::error(&codes::LINK_UNRESOLVED, message, span)
                                    .help("link to an Entry's file by its path from this one; a URL or a `/` path is left as it is"),
                            );
                            continue;
                        }
                    }
                };
                let page = &entries[target].path;
                let fragment = match search {
                    Some((_, query)) => {
                        let found = match query.strip_prefix('*') {
                            Some(title) => titles[target].get(&squash(title)),
                            None => entries[target]
                                .document
                                .names
                                .targets
                                .get(&squash(query))
                                .or_else(|| titles[target].get(&squash(query))),
                        };
                        match found {
                            Some(anchor) => Some(anchor.clone()),
                            None => {
                                let message = format!("`{page}` has nothing `::{query}` finds: no headline with that title, `<<target>>` or `#+NAME`");
                                reported.push(Diagnostic::error(&codes::LINK_UNRESOLVED, message, span));
                                continue;
                            }
                        }
                    }
                    // `#` alone is the top of the page.
                    None => match fragment.filter(|f| !f.is_empty()) {
                        Some(f) => {
                            if !anchors[target].contains(&percent_decode(f)) {
                                let fragment = percent_decode(f);
                                let message = format!("`{page}` has no heading with the id `{fragment}`");
                                let mut d = Diagnostic::error(&codes::LINK_UNRESOLVED, message, span);
                                if let Some(near) = crate::config::similar(&fragment, anchors[target].iter().map(String::as_str)) {
                                    d = d.help(format!("the closest id is `{near}`"));
                                }
                                reported.push(d);
                                continue;
                            }
                            Some(f.to_string())
                        }
                        None => fragment.map(str::to_string),
                    },
                };
                // `a.md#x` in `a.md` itself still needs its Route.
                if !path.is_empty() {
                    let route = url_of[&target];
                    let bare = node.children.is_empty() || doc.names.undescribed.contains(&id);
                    let text = fragment
                        .as_deref()
                        .filter(|_| bare)
                        .and_then(|f| entries[target].document.names.numbers.get(&percent_decode(f)))
                        .cloned();
                    let href = match fragment {
                        Some(f) => format!("{route}#{f}"),
                        None => route.to_string(),
                    };
                    out.insert(id, Link { href, text });
                }
            }
            (out, reported)
        })
        .collect();
    found
        .into_iter()
        .map(|(out, found)| {
            diagnostics.extend(found);
            out
        })
        .collect()
}

/// Runs of whitespace as one space, as Org compares titles.
fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A URL with a scheme, a protocol-relative one, or a path from the Site's
/// root: none of them is a file of the project.
fn is_external(url: &str) -> bool {
    let scheme = url.split_once(':').is_some_and(|(s, _)| {
        s.starts_with(|c: char| c.is_ascii_alphabetic())
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c))
    });
    scheme || url.starts_with('/')
}

/// Resolves `.` and `..` in a path from the project root.
fn normalize(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            _ => out.push(seg),
        }
    }
    out.join("/")
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        match (
            bytes[i],
            bytes.get(i + 1).and_then(|&b| hex(b)),
            bytes.get(i + 2).and_then(|&b| hex(b)),
        ) {
            (b'%', Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            (b, _, _) => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}
