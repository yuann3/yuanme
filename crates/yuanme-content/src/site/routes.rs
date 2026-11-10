//! Every Route the Site has, and the files they write.

use std::collections::BTreeMap;

use yuanme_diag::{Diagnostic, Span, codes};

use super::{Route, RouteKind, Term, sort};
use crate::config::{Collection, Config, SortKey};
use crate::entry::Entry;
use crate::slug;

fn output_of(url: &str) -> String {
    let path = &url[1..];
    if path.is_empty() || path.ends_with('/') {
        format!("{path}index.html")
    } else {
        path.to_string()
    }
}

/// `entries` split into pages of `size`, with at least one page.
fn pages_of(entries: &[usize], size: Option<u64>) -> Vec<Vec<usize>> {
    match size {
        Some(n) if !entries.is_empty() => {
            entries.chunks(n as usize).map(<[usize]>::to_vec).collect()
        }
        _ => vec![entries.to_vec()],
    }
}

fn page_url(first: &str, page: usize) -> String {
    if page == 1 {
        first.to_string()
    } else {
        format!("{}/{page}/", first.trim_end_matches('/'))
    }
}

/// Every Route the Site asks for, at its URL, with where it came from for
/// a collision.
pub(super) fn wanted(
    collections: &BTreeMap<String, Collection>,
    config: &Config,
    entries: &[Entry],
    lists: &BTreeMap<String, Vec<usize>>,
    taxonomies: &BTreeMap<String, Vec<Term>>,
) -> Vec<(String, RouteKind, Span)> {
    let mut routes: Vec<(String, RouteKind, Span)> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let url = collections[&e.collection].route.replace("{slug}", &e.slug);
        let at = Span {
            file: e.file,
            range: e.slug_at.clone(),
        };
        routes.push((url, RouteKind::Entry(i), at));
    }
    for (name, c) in collections {
        let Some(list) = &c.list_route else { continue };
        for (n, page) in pages_of(&lists[name], c.paginate).into_iter().enumerate() {
            let kind = RouteKind::List {
                collection: name.clone(),
                page: n + 1,
                entries: page,
            };
            let at = Span {
                file: config.file,
                range: c.dir_at.clone(),
            };
            routes.push((page_url(list, n + 1), kind, at));
        }
    }
    for (name, t) in &config.taxonomies {
        let index = t
            .route
            .split("{term}")
            .next()
            .expect("split yields one")
            .to_string();
        let kind = RouteKind::Terms {
            taxonomy: name.clone(),
        };
        let at = Span {
            file: config.file,
            range: 0..0,
        };
        routes.push((index, kind, at));
        for (ti, term) in taxonomies[name].iter().enumerate() {
            let first = t.route.replace("{term}", &term.slug);
            for (n, page) in pages_of(&term.entries, t.paginate).into_iter().enumerate() {
                let kind = RouteKind::Term {
                    taxonomy: name.clone(),
                    term: ti,
                    page: n + 1,
                    entries: page,
                };
                let at = Span {
                    file: entries[term.entries[0]].file,
                    range: 0..0,
                };
                routes.push((page_url(&first, n + 1), kind, at));
            }
        }
    }

    for (name, r) in &config.routes {
        let at = Span {
            file: config.file,
            range: r.template_at.clone(),
        };
        routes.push((r.path.clone(), RouteKind::Template(name.clone()), at));
    }
    for (name, f) in &config.feeds {
        let mut ids: Vec<usize> = (0..entries.len())
            .filter(|&i| {
                let e = &entries[i];
                !e.draft && !e.unlisted && f.collections.contains(&e.collection)
            })
            .collect();
        let newest_first = SortKey {
            field: "date".into(),
            descending: true,
        };
        sort(&mut ids, entries, &[newest_first]);
        let kind = RouteKind::Feed {
            name: name.clone(),
            entries: ids,
        };
        let at = Span {
            file: config.file,
            range: f.template_at.clone(),
        };
        routes.push((f.path.clone(), kind, at));
    }
    routes
}

/// Gives each wanted Route its output file: the first to want a file
/// gets it, and each later one is an error.
pub(super) fn claim(
    routes: Vec<(String, RouteKind, Span)>,
    entries: &[Entry],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Route> {
    let mut out: Vec<Route> = Vec::new();
    let mut taken: BTreeMap<String, usize> = BTreeMap::new();
    for (url, kind, at) in routes {
        let output = output_of(&url);
        if let Some(&first) = taken.get(&output) {
            let message = format!(
                "`{}` and `{}` would both write `{output}`",
                describe(&out[first].kind, entries),
                describe(&kind, entries)
            );
            diagnostics.push(
                Diagnostic::error(&codes::ENTRY_OUTPUT_COLLISION, message, at)
                    .help("change one Collection's `route`, or rename one of the files"),
            );
            continue;
        }
        taken.insert(output.clone(), out.len());
        out.push(Route {
            url: slug::url_path(&url),
            output,
            kind,
        });
    }
    out
}

fn describe(kind: &RouteKind, entries: &[Entry]) -> String {
    match kind {
        RouteKind::Entry(i) => entries[*i].path.clone(),
        RouteKind::List {
            collection, page, ..
        } => format!("page {page} of the `{collection}` list"),
        RouteKind::Term { taxonomy, page, .. } => format!("page {page} of a `{taxonomy}` Term"),
        RouteKind::Terms { taxonomy } => format!("the `{taxonomy}` index"),
        RouteKind::Template(name) => format!("`[routes.{name}]`"),
        RouteKind::Feed { name, .. } => format!("`[feeds.{name}]`"),
    }
}
