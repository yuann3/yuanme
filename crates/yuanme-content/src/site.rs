//! The resolve stage (#24): every Entry, then what only the whole Site
//! knows: Slugs that collide, sort order, Terms, Routes and output paths.

mod links;
mod routes;

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use jiff::tz::TimeZone;
use yuanme_diag::{Diagnostic, FileId, Files, Level, Span, codes};

use crate::config::{Collection, Config, SortKey, Taxonomy};
use crate::document::Links;
use crate::entry::{self, Entry};
use crate::fields::{Cx, Value, compare};
use crate::slug;
use crate::{data, markdown, org};
use links::links;

#[derive(Debug)]
pub struct Site {
    /// Every Entry the build has: Drafts only when they are shown.
    pub entries: Vec<Entry>,
    /// Each Collection's listed Entries, in its sort order: no Drafts unless
    /// shown, and never an Unlisted Entry.
    pub collections: BTreeMap<String, Vec<usize>>,
    pub taxonomies: BTreeMap<String, Vec<Term>>,
    pub routes: Vec<Route>,
    /// Where each Entry's links to other files go, by Entry.
    pub links: Vec<Links>,
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
    /// A Page made by a Template alone, by its name in Site config.
    Template(String),
    /// A feed's published, listed Entries, newest first.
    Feed {
        name: String,
        entries: Vec<usize>,
    },
}

/// What the build reads, besides Site config.
pub struct Project<'a> {
    pub files: &'a Files,
    /// Every content file, as [`read`] reads it.
    pub entries: Vec<(Option<Entry>, Vec<Diagnostic>)>,
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
        template: None,
        template_at: 0..0,
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

/// Reads the content file `file`, of `collection`, into an Entry: `None`
/// if it has an error. It reads nothing but the file, its path, Site config
/// and what `is_file` answers, so a cache of Entries keys them by those
/// (ADR 0011).
pub fn read(
    config: &Config,
    collection: &str,
    file: FileId,
    files: &Files,
    is_file: &dyn Fn(&str) -> bool,
) -> (Option<Entry>, Vec<Diagnostic>) {
    let path = files.path(file);
    let text = files.text(file);
    let builtin = pages();
    let c = config.collections.get(collection).unwrap_or(&builtin);
    let timezone = TimeZone::get(&config.site.timezone).expect("config checked the zone");
    let taxonomies: Vec<(&str, &Taxonomy)> = config
        .taxonomies
        .iter()
        .filter(|(_, t)| t.collections.iter().any(|n| n == collection))
        .map(|(n, t)| (n.as_str(), t))
        .collect();
    let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
    let mut found = Vec::new();
    // Each file is its own boundary, so a crash in a parser fails that
    // file and the rest are still checked (#15).
    let read = yuanme_diag::catch_panic(files, file, || {
        let mut cx = Cx {
            file,
            diagnostics: &mut found,
            timezone: &timezone,
            dir,
            is_file,
        };
        if path.ends_with(".org") {
            let read = org::read(text, file, &config.math);
            cx.diagnostics.extend(read.diagnostics);
            // A file that didn't parse has no keywords to check.
            read.document.as_ref()?;
            let head = org::head(
                &read.keywords,
                read.preamble,
                text.len(),
                c,
                &taxonomies,
                &mut cx,
            );
            // A body or keyword with an error fails the Entry, once its
            // fields are checked too.
            let document = read.document.filter(|_| !has_errors(cx.diagnostics));
            return entry::read((collection, c), &taxonomies, head, document, path, &mut cx);
        }
        let head = entry::front_matter(text, &mut cx)?;
        let document = markdown::parse(text, head.body.clone(), &config.math, &mut cx);
        entry::read((collection, c), &taxonomies, head, document, path, &mut cx)
    });
    match read {
        Ok(entry) => (entry, found),
        Err(crash) => {
            found.push(crash);
            (None, found)
        }
    }
}

/// Resolves the Site. It returns `None` if anything has an error; every
/// error is reported, not only the first.
pub fn resolve(
    config: &Config,
    mut project: Project<'_>,
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

    // Every check reports its own errors, so the Diagnostics alone say
    // whether anything failed.
    let mut entries = Vec::new();
    for (entry, mut found) in std::mem::take(&mut project.entries) {
        diagnostics.append(&mut found);
        entries.extend(entry);
    }

    duplicate_slugs(&entries, &mut diagnostics);
    let data = data::read(config, &project, &timezone, &mut diagnostics);

    if has_errors(&diagnostics) {
        return (None, sorted(diagnostics));
    }
    // Their files, which a link can name but not reach.
    let mut left_out = BTreeSet::new();
    if !drafts {
        left_out.extend(entries.iter().filter(|e| e.draft).map(|e| e.path.clone()));
        entries.retain(|e| !e.draft);
    }
    // A stable order whatever order the files came in.
    entries.sort_by(|a, b| (&a.collection, &a.slug).cmp(&(&b.collection, &b.slug)));

    let mut lists = BTreeMap::new();
    for (name, c) in &collections {
        let mut ids: Vec<usize> = (0..entries.len())
            .filter(|&i| entries[i].collection == *name && !entries[i].unlisted)
            .collect();
        sort(&mut ids, &entries, &c.sort);
        lists.insert(name.clone(), ids);
    }

    let mut taxonomies = BTreeMap::new();
    for (name, t) in &config.taxonomies {
        taxonomies.insert(name.clone(), terms(name, t, &entries, &mut diagnostics));
    }

    let routes = routes::wanted(&collections, config, &entries, &lists, &taxonomies);
    let routes = routes::claim(routes, &entries, &mut diagnostics);
    if has_errors(&diagnostics) {
        return (None, sorted(diagnostics));
    }
    let links = links(&entries, &routes, &left_out, &mut diagnostics);
    if has_errors(&diagnostics) {
        return (None, sorted(diagnostics));
    }
    let site = Site {
        entries,
        collections: lists,
        taxonomies,
        routes,
        links,
        data,
    };
    (Some(site), sorted(diagnostics))
}

fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.level == Level::Error)
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
    // An Unlisted Entry is in no Term, so a Term only it uses doesn't exist.
    for (i, e) in entries.iter().enumerate().filter(|(_, e)| !e.unlisted) {
        for (spelled, at) in e.terms.get(name).into_iter().flatten() {
            let identity = slug::term_slug(spelled);
            let first = Span {
                file: e.file,
                range: at.clone(),
            };
            let term = by_identity
                .entry(identity)
                .or_insert_with(|| (spelled.clone(), Vec::new(), first));
            // An Entry can spell one Term twice.
            if term.1.last() != Some(&i) {
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
