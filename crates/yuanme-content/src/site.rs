//! The resolve stage (#24): every Entry, then what only the whole Site
//! knows: Slugs that collide, sort order, Terms, Routes and output paths.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::ops::Range;

use jiff::tz::TimeZone;
use yuanme_diag::{Diagnostic, FileId, Files, Span, codes};

use crate::config::{Collection, Config, SortKey, Taxonomy};
use crate::data;
use crate::entry::{self, Entry, SourceFormat};
use crate::fields::{Cx, Value, compare};
use crate::slug;

#[derive(Debug)]
pub struct Site {
    /// Every Entry the build has: Drafts only when they are shown.
    pub entries: Vec<Entry>,
    /// Each Collection's listed Entries, in its sort order: no Drafts unless
    /// shown, and never an Unlisted Entry.
    pub collections: BTreeMap<String, Vec<usize>>,
    pub taxonomies: BTreeMap<String, Vec<Term>>,
    pub routes: Vec<Route>,
    pub data: BTreeMap<String, Value>,
}

#[derive(Debug)]
pub struct Term {
    /// How the Term is shown: as first spelled, or lowercased.
    pub name: String,
    pub slug: String,
    /// Its listed Entries, in the Taxonomy's sort order.
    pub entries: Vec<usize>,
}

#[derive(Debug)]
pub struct Route {
    /// As a link writes it: non-ASCII percent-encoded (ADR 0005).
    pub url: String,
    /// The file it writes, from the output directory.
    pub output: String,
    pub kind: RouteKind,
}

#[derive(Debug)]
pub enum RouteKind {
    Entry(usize),
    /// Page `page` (from 1) of a Collection's list.
    List {
        collection: String,
        page: usize,
        entries: Vec<usize>,
    },
    /// Page `page` of one Term's Entries.
    Term {
        taxonomy: String,
        term: usize,
        page: usize,
        entries: Vec<usize>,
    },
    /// A Taxonomy's index of Terms, emitted even with none.
    Terms {
        taxonomy: String,
    },
}

/// What the build reads, besides Site config.
pub struct Project<'a> {
    pub files: &'a Files,
    /// Content files, by Collection.
    pub entries: Vec<(String, FileId)>,
    pub data: Vec<FileId>,
    pub is_file: &'a dyn Fn(&str) -> bool,
    pub is_dir: &'a dyn Fn(&str) -> bool,
}

/// The built-in Collection for prose pages outside any other (#18).
fn pages() -> Collection {
    Collection {
        dir: "content/pages".into(),
        dir_at: 0..0,
        route: "/{slug}/".into(),
        list_route: None,
        paginate: None,
        sort: vec![SortKey {
            field: "date".into(),
            descending: true,
        }],
        fields: BTreeMap::new(),
    }
}

/// Every Collection, the built-in `pages` included unless Site config
/// declares its own.
pub fn collections(config: &Config) -> BTreeMap<String, Collection> {
    let mut all = config.collections.clone();
    all.entry("pages".into()).or_insert_with(pages);
    all
}

/// Sorts project paths into content files, by Collection, and Data files.
/// A file in two Collections' folders belongs to the deeper one.
pub fn sources<'p>(
    config: &Config,
    paths: impl IntoIterator<Item = &'p str>,
) -> (Vec<(String, &'p str)>, Vec<&'p str>) {
    let collections = collections(config);
    let mut entries = Vec::new();
    let mut data = Vec::new();
    for path in paths {
        if let Some(name) = path.strip_prefix("data/")
            && !name.contains('/')
            && matches!(
                name.rsplit_once('.').map(|(_, e)| e),
                Some("toml" | "yaml" | "yml" | "json")
            )
        {
            data.push(path);
            continue;
        }
        if !(path.ends_with(".md") || path.ends_with(".org")) {
            continue;
        }
        let owner = collections
            .iter()
            .filter(|(_, c)| {
                path.strip_prefix(c.dir.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
            })
            .max_by_key(|(_, c)| c.dir.len());
        if let Some((name, _)) = owner {
            entries.push((name.clone(), path));
        }
    }
    (entries, data)
}

/// Resolves the Site. It returns `None` if anything has an error; every
/// error is reported, not only the first.
pub fn resolve(
    config: &Config,
    project: &Project<'_>,
    drafts: bool,
) -> (Option<Site>, Vec<Diagnostic>) {
    let mut diagnostics = Vec::new();
    let collections = collections(config);
    let timezone = TimeZone::get(&config.site.timezone).expect("config checked the zone");
    let config_span = |range: Range<usize>| Span {
        file: config.file,
        range,
    };

    for (name, c) in &config.collections {
        if !(project.is_dir)(&c.dir) {
            let message = format!(
                "the folder `{}` of the Collection `{name}` doesn't exist",
                c.dir
            );
            diagnostics.push(
                Diagnostic::error(
                    &codes::CONFIG_INVALID_VALUE,
                    message,
                    config_span(c.dir_at.clone()),
                )
                .help(format!(
                    "create `{}`, or set `dir` to the folder its Entries are in",
                    c.dir
                )),
            );
        }
    }

    let mut entries = Vec::new();
    let mut failed = false;
    for (collection, file) in &project.entries {
        let path = project.files.path(*file);
        let text = project.files.text(*file);
        let c = &collections[collection];
        let taxonomies: Vec<(&str, &Taxonomy)> = config
            .taxonomies
            .iter()
            .filter(|(_, t)| t.collections.contains(collection))
            .map(|(n, t)| (n.as_str(), t))
            .collect();
        let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
        let mut cx = Cx {
            file: *file,
            diagnostics: &mut diagnostics,
            timezone: &timezone,
            dir,
            is_file: project.is_file,
        };
        if path.ends_with(".org") {
            let d = cx
                .error(
                    &codes::ORG_NOT_YET_SUPPORTED,
                    "yuanme can't read Org Entries yet".into(),
                    0..0,
                )
                .note("Org support arrives with the Org subset, before the first release");
            cx.push(d);
            failed = true;
            continue;
        }
        let Some((raw, head, body)) = entry::front_matter(text, &mut cx) else {
            failed = true;
            continue;
        };
        match entry::read(
            (collection, c),
            &taxonomies,
            raw,
            head,
            body,
            path,
            SourceFormat::Markdown,
            &mut cx,
        ) {
            Some(e) => entries.push(e),
            None => failed = true,
        }
    }

    duplicate_slugs(&entries, &mut diagnostics);
    let data = data::read(config, project, &timezone, &mut diagnostics);

    if failed
        || diagnostics
            .iter()
            .any(|d| d.level == yuanme_diag::Level::Error)
    {
        return (None, sorted(diagnostics));
    }
    if !drafts {
        entries.retain(|e| !e.draft);
    }
    // A stable order whatever order the files came in.
    entries.sort_by(|a, b| (&a.collection, &a.slug).cmp(&(&b.collection, &b.slug)));

    let listed = |e: &Entry| !e.unlisted;
    let mut lists = BTreeMap::new();
    for (name, c) in &collections {
        let mut ids: Vec<usize> = (0..entries.len())
            .filter(|&i| entries[i].collection == *name && listed(&entries[i]))
            .collect();
        sort(&mut ids, &entries, &c.sort);
        lists.insert(name.clone(), ids);
    }

    let mut taxonomies = BTreeMap::new();
    for (name, t) in &config.taxonomies {
        taxonomies.insert(name.clone(), terms(name, t, &entries, &mut diagnostics));
    }

    let routes = routes(
        &collections,
        config,
        &entries,
        &lists,
        &taxonomies,
        &mut diagnostics,
    );
    if diagnostics
        .iter()
        .any(|d| d.level == yuanme_diag::Level::Error)
    {
        return (None, sorted(diagnostics));
    }
    let site = Site {
        entries,
        collections: lists,
        taxonomies,
        routes,
        data: data.expect("no errors"),
    };
    (Some(site), sorted(diagnostics))
}

fn sorted(mut diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics.sort_by_key(|d| (d.primary().file, d.primary().range.start));
    diagnostics
}

fn duplicate_slugs(entries: &[Entry], diagnostics: &mut Vec<Diagnostic>) {
    let mut seen: BTreeMap<(&str, &str), &Entry> = BTreeMap::new();
    for e in entries {
        match seen.get(&(e.collection.as_str(), e.slug.as_str())) {
            Some(first) => {
                let message = format!(
                    "`{}` and `{}` have the same Slug, `{}`",
                    first.path, e.path, e.slug
                );
                let span = |e: &Entry| Span {
                    file: e.file,
                    range: e.slug_at.clone(),
                };
                diagnostics.push(
                    Diagnostic::error(&codes::ENTRY_DUPLICATE_SLUG, message, span(e))
                        .label("this Entry")
                        .secondary(span(first), "has the same Slug as this one")
                        .help("rename one file, or give one a different `slug:`"),
                );
            }
            None => {
                seen.insert((&e.collection, &e.slug), e);
            }
        }
    }
}

/// Sorts by `keys` in turn: a missing value sorts last whichever the
/// direction, and ties break by Slug (#18).
fn sort(ids: &mut [usize], entries: &[Entry], keys: &[SortKey]) {
    ids.sort_by(|&a, &b| {
        for key in keys {
            let order = match (entries[a].get(&key.field), entries[b].get(&key.field)) {
                (Some(x), Some(y)) if key.descending => compare(&y, &x),
                (Some(x), Some(y)) => compare(&x, &y),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            };
            if order != Ordering::Equal {
                return order;
            }
        }
        entries[a].slug.cmp(&entries[b].slug)
    });
}

/// A Taxonomy's Terms, by Slug. Spellings with one Slug are one Term; a
/// `slugs` override that gives two Terms one Slug is an error (ADR 0005).
fn terms(
    name: &str,
    t: &Taxonomy,
    entries: &[Entry],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Term> {
    let overrides: BTreeMap<String, &str> = t
        .slugs
        .iter()
        .map(|(k, v)| (slug::term_slug(k), v.as_str()))
        .collect();
    // Keyed by the Slug the spelling derives, which decides identity.
    let mut by_identity: BTreeMap<String, (String, Vec<usize>, Span)> = BTreeMap::new();
    for (i, e) in entries.iter().enumerate() {
        for (spelled, at) in e.terms.get(name).into_iter().flatten() {
            let identity = slug::term_slug(spelled);
            let first = Span {
                file: e.file,
                range: at.clone(),
            };
            let term = by_identity
                .entry(identity)
                .or_insert_with(|| (spelled.clone(), Vec::new(), first));
            if !e.unlisted && !term.1.contains(&i) {
                term.1.push(i);
            }
        }
    }
    let mut out: Vec<Term> = Vec::new();
    let mut owner: BTreeMap<String, (String, Span)> = BTreeMap::new();
    for (identity, (spelled, mut ids, first)) in by_identity {
        let final_slug = overrides
            .get(&identity)
            .map_or(identity.clone(), |s| s.to_string());
        if let Some((other, other_at)) = owner.get(&final_slug) {
            let message = format!(
                "the Terms `{other}` and `{spelled}` of `{name}` would both have the Slug `{final_slug}`"
            );
            diagnostics.push(
                Diagnostic::error(&codes::ENTRY_TERM_COLLISION, message, first)
                    .secondary(other_at.clone(), format!("`{other}` is used here"))
                    .help(format!(
                        "change `slugs` in `[taxonomies.{name}]` so each Term keeps its own Slug"
                    )),
            );
            continue;
        }
        owner.insert(final_slug.clone(), (spelled.clone(), first));
        sort(&mut ids, entries, &t.sort);
        let shown = if t.lowercase {
            spelled.to_lowercase()
        } else {
            spelled
        };
        out.push(Term {
            name: shown,
            slug: final_slug,
            entries: ids,
        });
    }
    out.sort_by(|a, b| a.slug.cmp(&b.slug));
    out
}

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

fn routes(
    collections: &BTreeMap<String, Collection>,
    config: &Config,
    entries: &[Entry],
    lists: &BTreeMap<String, Vec<usize>>,
    taxonomies: &BTreeMap<String, Vec<Term>>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Route> {
    let mut routes = Vec::new();
    // Where each Route came from, for a collision.
    let mut origin: Vec<Span> = Vec::new();
    for (i, e) in entries.iter().enumerate() {
        let url = collections[&e.collection].route.replace("{slug}", &e.slug);
        routes.push((url, RouteKind::Entry(i)));
        origin.push(Span {
            file: e.file,
            range: e.slug_at.clone(),
        });
    }
    for (name, c) in collections {
        let Some(list) = &c.list_route else { continue };
        for (n, page) in pages_of(&lists[name], c.paginate).into_iter().enumerate() {
            let kind = RouteKind::List {
                collection: name.clone(),
                page: n + 1,
                entries: page,
            };
            routes.push((page_url(list, n + 1), kind));
            origin.push(Span {
                file: config.file,
                range: c.dir_at.clone(),
            });
        }
    }
    for (name, t) in &config.taxonomies {
        let index = t
            .route
            .split("{term}")
            .next()
            .expect("split yields one")
            .to_string();
        routes.push((
            index,
            RouteKind::Terms {
                taxonomy: name.clone(),
            },
        ));
        origin.push(Span {
            file: config.file,
            range: 0..0,
        });
        for (ti, term) in taxonomies[name].iter().enumerate() {
            let first = t.route.replace("{term}", &term.slug);
            for (n, page) in pages_of(&term.entries, t.paginate).into_iter().enumerate() {
                let kind = RouteKind::Term {
                    taxonomy: name.clone(),
                    term: ti,
                    page: n + 1,
                    entries: page,
                };
                routes.push((page_url(&first, n + 1), kind));
                origin.push(Span {
                    file: entries[term.entries.first().copied().unwrap_or(0)].file,
                    range: 0..0,
                });
            }
        }
    }

    let mut out: Vec<Route> = Vec::new();
    let mut taken: BTreeMap<String, usize> = BTreeMap::new();
    for ((url, kind), at) in routes.into_iter().zip(origin) {
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
    }
}
