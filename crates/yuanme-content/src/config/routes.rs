//! `[routes]` and `[feeds]`: Pages made by a Template alone.

use std::collections::BTreeMap;
use std::ops::Range;

use super::check::Checker;
use super::collections::{self, Collection, collection_name};
use super::{Table, as_table, entries, items, string, strings};

/// A Page made by a Template alone (#19).
#[derive(Debug)]
pub struct TemplateRoute {
    pub path: String,
    pub template: String,
    pub template_at: Range<usize>,
}

#[derive(Debug)]
pub struct Feed {
    pub collections: Vec<String>,
    pub path: String,
    pub template: String,
    /// Where `template` is set, or the feed named when it isn't.
    pub template_at: Range<usize>,
}

pub(super) fn read(routes: Option<&Table<'_>>) -> BTreeMap<String, TemplateRoute> {
    entries(routes)
        .iter()
        .map(|(name, r)| {
            let r = as_table(r);
            let route = TemplateRoute {
                path: string(r, "path").expect("the first pass requires `path`"),
                template: string(r, "template").expect("the first pass requires `template`"),
                template_at: r["template"].span(),
            };
            (name.get_ref().to_string(), route)
        })
        .collect()
}

/// `[feeds]`, whose Collections must be ones the Site has.
pub(super) fn feeds(
    feeds: Option<&Table<'_>>,
    collections: &BTreeMap<String, Collection>,
    checker: &mut Checker,
) -> BTreeMap<String, Feed> {
    let names = collections::names(collections);
    entries(feeds)
        .iter()
        .map(|(name, f)| {
            let f = as_table(f);
            for c in items(&f["collections"]) {
                collection_name(c, &names, checker);
            }
            let feed = Feed {
                collections: strings(&f["collections"]),
                path: string(f, "path").expect("the first pass requires `path`"),
                template: string(f, "template").unwrap_or_else(|| "rss.xml".into()),
                template_at: f.get("template").map_or(name.span(), |t| t.span()),
            };
            (name.get_ref().to_string(), feed)
        })
        .collect()
}
