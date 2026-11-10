//! Where each link to another file or a fragment goes.

use std::collections::{BTreeMap, BTreeSet};

use rayon::prelude::*;
use yuanme_diag::{Diagnostic, Span, codes};

use super::{Route, RouteKind};
use crate::config::similar;
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
    let targets = Targets::new(entries, routes, left_out, diagnostics);
    // Each Entry on a thread of its own, since this walks every node of
    // every Document on every build.
    let found: Vec<(Links, Vec<Diagnostic>)> = entries
        .par_iter()
        .enumerate()
        .map(|(i, e)| targets.entry(i, e))
        .collect();
    found
        .into_iter()
        .map(|(out, found)| {
            diagnostics.extend(found);
            out
        })
        .collect()
}

/// What a link can lead to, across the whole Site.
struct Targets<'a> {
    entries: &'a [Entry],
    left_out: &'a BTreeSet<String>,
    /// Each Entry's Route.
    url_of: BTreeMap<usize, &'a str>,
    by_path: BTreeMap<&'a str, usize>,
    /// Every id a fragment can name: headings', and Org's targets and names.
    anchors: Vec<Vec<String>>,
    /// Where an Org `::search` finds a headline: Org's titles as written, and
    /// a Markdown heading's text.
    titles: Vec<BTreeMap<String, String>>,
    /// The Site-wide `:ID:` index: an Entry, and a heading's id or nothing.
    ids: BTreeMap<&'a str, (usize, &'a str, Span)>,
}

impl<'a> Targets<'a> {
    /// The indexes, and an error for each `:ID:` given twice.
    fn new(
        entries: &'a [Entry],
        routes: &'a [Route],
        left_out: &'a BTreeSet<String>,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Self {
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
        let anchors: Vec<Vec<String>> = entries
            .par_iter()
            .map(|e| {
                let headings = e.document.headings().iter().map(|h| h.id.clone());
                headings
                    .chain(e.document.names.targets.values().cloned())
                    .collect()
            })
            .collect();
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
        Targets {
            entries,
            left_out,
            url_of,
            by_path,
            anchors,
            titles,
            ids,
        }
    }

    /// Where the links in Entry `i`, `e`, go, and the ones that go nowhere.
    fn entry(&self, i: usize, e: &Entry) -> (Links, Vec<Diagnostic>) {
        let mut reported = Vec::new();
        let doc = &e.document;
        let mut out = Links::new();
        for &id in doc.all() {
            let node = doc.node(id);
            let Kind::Link { url, .. } = &node.kind else {
                continue;
            };
            let span = Span {
                file: e.file,
                range: node.span.clone(),
            };
            let bare = node.children.is_empty() || doc.names.undescribed.contains(&id);
            match self.link(i, e, url, bare, span) {
                Ok(Some(link)) => {
                    out.insert(id, link);
                }
                Ok(None) => {}
                Err(d) => reported.push(d),
            }
        }
        (out, reported)
    }

    /// Where `url`, a link in Entry `i`, goes: `None` when it stays as
    /// written. `bare` says the link shows no text of its own.
    fn link(
        &self,
        i: usize,
        e: &Entry,
        url: &str,
        bare: bool,
        span: Span,
    ) -> Result<Option<Link>, Diagnostic> {
        let org = e.format == SourceFormat::Org;
        if org && let Some(key) = url.strip_prefix("id:") {
            return self.id(key, span).map(Some);
        }
        // Org's `file:a.org::*Title` and `::name` search the other file.
        let search = url.split_once("::").filter(|(p, _)| org && !is_external(p));
        if search.is_none() && is_external(url) {
            return Ok(None);
        }
        let (path, fragment) = match search {
            Some((p, _)) => (p, None),
            None => url
                .split_once('#')
                .map_or((url, None), |(p, f)| (p, Some(f))),
        };
        let target = if path.is_empty() {
            i
        } else {
            self.file(e, path, &span)?
        };
        let fragment = match search {
            Some((_, query)) => Some(self.search(target, query, span)?),
            None => self.fragment(target, fragment, span)?,
        };
        // `a.md#x` in `a.md` itself still needs its Route.
        if path.is_empty() {
            return Ok(None);
        }
        let route = self.url_of[&target];
        let text = fragment
            .as_deref()
            .filter(|_| bare)
            .and_then(|f| {
                self.entries[target]
                    .document
                    .names
                    .numbers
                    .get(&percent_decode(f))
            })
            .cloned();
        let href = match fragment {
            Some(f) => format!("{route}#{f}"),
            None => route.to_string(),
        };
        Ok(Some(Link { href, text }))
    }

    /// An Org `id:` link, to the Entry or headline with that `:ID:`.
    fn id(&self, key: &str, span: Span) -> Result<Link, Diagnostic> {
        match self.ids.get(key) {
            Some((j, anchor, _)) => {
                let route = self.url_of[j];
                let href = if anchor.is_empty() {
                    route.to_string()
                } else {
                    format!("{route}#{anchor}")
                };
                Ok(Link { href, text: None })
            }
            None => Err(Diagnostic::error(
                &codes::LINK_UNRESOLVED,
                format!("no Entry or heading has the ID `{key}`"),
                span,
            )
            .help("an `id:` link leads to the Entry or headline whose `:PROPERTIES:` drawer has that `:ID:`")),
        }
    }

    /// The Entry written in `path`, from `e`'s folder.
    fn file(&self, e: &Entry, path: &str, span: &Span) -> Result<usize, Diagnostic> {
        let dir = e.path.rsplit_once('/').map_or("", |(d, _)| d);
        let resolved = normalize(&format!("{dir}/{}", percent_decode(path)));
        match self.by_path.get(resolved.as_str()) {
            Some(&j) => Ok(j),
            None if self.left_out.contains(&resolved) => {
                let message = format!("`{resolved}` is a Draft, which this build leaves out");
                Err(
                    Diagnostic::error(&codes::LINK_UNRESOLVED, message, span.clone()).help(
                        "publish it by removing `draft: true`, or remove the link until it is",
                    ),
                )
            }
            None => {
                let message = format!("no Entry is written in `{resolved}`");
                Err(
                    Diagnostic::error(&codes::LINK_UNRESOLVED, message, span.clone())
                        .help("link to an Entry's file by its path from this one; a URL or a `/` path is left as it is"),
                )
            }
        }
    }

    /// The anchor an Org `::query` finds in Entry `target`.
    fn search(&self, target: usize, query: &str, span: Span) -> Result<String, Diagnostic> {
        let titles = &self.titles[target];
        let found = match query.strip_prefix('*') {
            Some(title) => titles.get(&squash(title)),
            None => self.entries[target]
                .document
                .names
                .targets
                .get(&squash(query))
                .or_else(|| titles.get(&squash(query))),
        };
        match found {
            Some(anchor) => Ok(anchor.clone()),
            None => {
                let page = &self.entries[target].path;
                let message = format!(
                    "`{page}` has nothing `::{query}` finds: no headline with that title, `<<target>>` or `#+NAME`"
                );
                Err(Diagnostic::error(&codes::LINK_UNRESOLVED, message, span))
            }
        }
    }

    /// `#fragment`, checked against the ids in Entry `target`.
    fn fragment(
        &self,
        target: usize,
        fragment: Option<&str>,
        span: Span,
    ) -> Result<Option<String>, Diagnostic> {
        // `#` alone is the top of the page.
        let Some(f) = fragment.filter(|f| !f.is_empty()) else {
            return Ok(fragment.map(str::to_string));
        };
        let anchors = &self.anchors[target];
        if !anchors.contains(&percent_decode(f)) {
            let page = &self.entries[target].path;
            let fragment = percent_decode(f);
            let message = format!("`{page}` has no heading with the id `{fragment}`");
            let mut d = Diagnostic::error(&codes::LINK_UNRESOLVED, message, span);
            if let Some(near) = similar(&fragment, anchors.iter().map(String::as_str)) {
                d = d.help(format!("the closest id is `{near}`"));
            }
            return Err(d);
        }
        Ok(Some(f.to_string()))
    }
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
