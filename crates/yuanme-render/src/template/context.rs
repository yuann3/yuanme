//! What a Template can see (#19). Every value is built once and shared by
//! the Pages that show it; an absent optional field is left out of its
//! map, so a Template finds it undefined, never null (#18).

use std::collections::BTreeMap;

use minijinja::Value;
use rayon::prelude::*;
use yuanme_content::config::Config;
use yuanme_content::entry::Entry;
use yuanme_content::fields::{self, Image};
use yuanme_content::site::{Route, RouteKind, Site};
use yuanme_content::slug;

use super::filters::absolute_url;
use crate::highlight::Code;
use crate::html::Pictures;

/// The objects every Page sees, and the ones Pages share.
pub(super) struct Context {
    globals: BTreeMap<&'static str, Value>,
    entries: Vec<Value>,
    /// Each Taxonomy's Terms, as `terms` and `term` show them.
    terms: BTreeMap<String, Vec<Value>>,
    taxonomies: BTreeMap<String, Value>,
    /// Each list's page URLs, by Collection or by Taxonomy and Term.
    lists: BTreeMap<String, Vec<String>>,
    term_pages: BTreeMap<(String, usize), Vec<String>>,
    origin: String,
}

fn map(pairs: impl IntoIterator<Item = (&'static str, Value)>) -> Value {
    Value::from(pairs.into_iter().collect::<BTreeMap<&'static str, Value>>())
}

impl Context {
    pub fn new(
        config: &Config,
        site: &Site,
        code: &[Code],
        pictures: &[Pictures],
        origin: &str,
    ) -> Context {
        let mut lists: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut term_pages: BTreeMap<(String, usize), Vec<String>> = BTreeMap::new();
        let mut index = BTreeMap::new();
        let mut entry_urls = vec![String::new(); site.entries.len()];
        // Resolve writes each list's pages in order.
        for r in &site.routes {
            match &r.kind {
                RouteKind::Entry(i) => entry_urls[*i] = r.url.clone(),
                RouteKind::List { collection, .. } => lists
                    .entry(collection.clone())
                    .or_default()
                    .push(r.url.clone()),
                RouteKind::Term { taxonomy, term, .. } => term_pages
                    .entry((taxonomy.clone(), *term))
                    .or_default()
                    .push(r.url.clone()),
                RouteKind::Terms { taxonomy } => {
                    index.insert(taxonomy.clone(), r.url.clone());
                }
                RouteKind::Template(_) | RouteKind::Feed { .. } => {}
            }
        }

        let bodies: Vec<String> = site
            .entries
            .par_iter()
            .zip(&site.links)
            .zip(code)
            .zip(pictures)
            .map(|(((e, links), code), pictures)| {
                crate::html::body(&e.document, links, code, pictures)
            })
            .collect();
        let entries: Vec<Value> = site
            .entries
            .iter()
            .zip(bodies)
            .zip(&entry_urls)
            .map(|((e, body), url)| entry(config, site, e, url, body))
            .collect();

        let mut terms = BTreeMap::new();
        let mut taxonomies = BTreeMap::new();
        for (name, list) in &site.taxonomies {
            let values: Vec<Value> = list
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    map([
                        ("name", Value::from(t.name.as_str())),
                        ("slug", Value::from(t.slug.as_str())),
                        (
                            "url",
                            Value::from(term_pages[&(name.clone(), i)][0].as_str()),
                        ),
                        ("count", Value::from(t.entries.len())),
                    ])
                })
                .collect();
            let taxonomy = map([
                ("name", Value::from(name.as_str())),
                ("url", Value::from(index[name].as_str())),
                ("terms", Value::from(values.clone())),
            ]);
            terms.insert(name.clone(), values);
            taxonomies.insert(name.clone(), taxonomy);
        }

        let collections: BTreeMap<&str, Value> = site
            .collections
            .iter()
            .map(|(name, ids)| {
                let list: Vec<Value> = ids.iter().map(|&i| entries[i].clone()).collect();
                (name.as_str(), Value::from(list))
            })
            .collect();
        let s = &config.site;
        let mut site_value: BTreeMap<&str, Value> = BTreeMap::new();
        site_value.insert("url", Value::from(s.url.as_str()));
        site_value.insert("timezone", Value::from(s.timezone.as_str()));
        site_value.insert("extra", toml(&toml::Value::Table(s.extra.clone())));
        for (key, v) in [
            ("title", &s.title),
            ("description", &s.description),
            ("language", &s.language),
            ("author", &s.author),
        ] {
            if let Some(v) = v {
                site_value.insert(key, Value::from(v.as_str()));
            }
        }
        let data: BTreeMap<&str, Value> = site
            .data
            .iter()
            .map(|(name, v)| (name.as_str(), value(v)))
            .collect();
        let globals = BTreeMap::from([
            ("site", Value::from(site_value)),
            ("data", Value::from(data)),
            ("collections", Value::from(collections)),
            ("taxonomies", Value::from(taxonomies.clone())),
        ]);
        Context {
            globals,
            entries,
            terms,
            taxonomies,
            lists,
            term_pages,
            origin: origin.to_string(),
        }
    }

    /// The `i`th Entry's `content`.
    pub fn body(&self, i: usize) -> String {
        let content = self.entries[i].get_attr("content");
        content.expect("every Entry has content").to_string()
    }

    /// The context of one Route's Page.
    pub fn page(&self, route: &Route) -> Value {
        let mut ctx = self.globals.clone();
        let entries = |ids: &[usize]| {
            Value::from(
                ids.iter()
                    .map(|&i| self.entries[i].clone())
                    .collect::<Vec<_>>(),
            )
        };
        let kind = match &route.kind {
            RouteKind::Entry(i) => {
                ctx.insert("entry", self.entries[*i].clone());
                "entry"
            }
            RouteKind::List {
                collection,
                page,
                entries: ids,
            } => {
                let pages = &self.lists[collection];
                ctx.insert(
                    "collection",
                    map([
                        ("name", Value::from(collection.as_str())),
                        ("url", Value::from(pages[0].as_str())),
                    ]),
                );
                ctx.insert("entries", entries(ids));
                ctx.insert("paginator", paginator(pages, *page));
                "list"
            }
            RouteKind::Term {
                taxonomy,
                term,
                page,
                entries: ids,
            } => {
                ctx.insert("taxonomy", self.taxonomies[taxonomy].clone());
                ctx.insert("term", self.terms[taxonomy][*term].clone());
                ctx.insert("entries", entries(ids));
                let pages = &self.term_pages[&(taxonomy.clone(), *term)];
                ctx.insert("paginator", paginator(pages, *page));
                "term"
            }
            RouteKind::Terms { taxonomy } => {
                ctx.insert("taxonomy", self.taxonomies[taxonomy].clone());
                ctx.insert("terms", Value::from(self.terms[taxonomy].clone()));
                "index"
            }
            RouteKind::Template(_) => "template",
            RouteKind::Feed { name, entries: ids } => {
                ctx.insert(
                    "feed",
                    map([
                        ("name", Value::from(name.as_str())),
                        ("url", Value::from(route.url.as_str())),
                    ]),
                );
                ctx.insert("entries", entries(ids));
                "feed"
            }
        };
        ctx.insert(
            "page",
            map([
                ("url", Value::from(route.url.as_str())),
                (
                    "canonical",
                    Value::from(absolute_url(&self.origin, &route.url)),
                ),
                ("kind", Value::from(kind)),
            ]),
        );
        Value::from(ctx)
    }
}

/// `prev` and `next` are absent at the ends.
fn paginator(pages: &[String], number: usize) -> Value {
    let mut m = BTreeMap::from([
        ("number", Value::from(number)),
        ("total", Value::from(pages.len())),
    ]);
    if number > 1 {
        m.insert("prev", Value::from(pages[number - 2].as_str()));
    }
    if let Some(next) = pages.get(number) {
        m.insert("next", Value::from(next.as_str()));
    }
    Value::from(m)
}

/// An Entry with its declared fields directly on it, its Taxonomies'
/// fields, and what the Engine derives (#19).
fn entry(config: &Config, site: &Site, e: &Entry, url: &str, body: String) -> Value {
    let mut m: BTreeMap<String, Value> = e
        .fields
        .iter()
        .map(|(k, v)| (k.clone(), value(v)))
        .collect();
    let mut set = |k: &str, v: Value| {
        m.insert(k.to_string(), v);
    };
    set("title", Value::from(e.title.as_str()));
    set("slug", Value::from(e.slug.as_str()));
    set("draft", Value::from(e.draft));
    set("unlisted", Value::from(e.unlisted));
    for (k, d) in [
        ("date", e.date),
        ("updated", e.updated),
        ("modified", e.modified()),
    ] {
        if let Some(d) = d {
            set(k, Value::from(d.to_string()));
        }
    }
    set("url", Value::from(url));
    // Finished HTML, which Templates wrap and never alter.
    set("content", Value::from_safe_string(body));
    let headings: Vec<Value> = e
        .document
        .headings()
        .into_iter()
        .map(|h| {
            map([
                ("id", Value::from(h.id)),
                ("text", Value::from(h.text)),
                ("level", Value::from(h.level)),
            ])
        })
        .collect();
    set("headings", Value::from(headings));
    set("word_count", Value::from(e.word_count));
    set("reading_time", Value::from(e.reading_time()));
    for (name, t) in &config.taxonomies {
        if !t.collections.contains(&e.collection) {
            continue;
        }
        // A Term is shown as its Taxonomy names it; one only Unlisted
        // Entries use has no Term, so it keeps its spelling.
        let shown = |spelled: &str| {
            let identity = slug::term_slug(spelled);
            let final_slug = t
                .slugs
                .iter()
                .find(|(k, _)| slug::term_slug(k) == identity)
                .map_or(identity, |(_, v)| v.clone());
            site.taxonomies[name]
                .iter()
                .find(|term| term.slug == final_slug)
                .map_or_else(
                    || match t.lowercase {
                        true => spelled.to_lowercase(),
                        false => spelled.to_string(),
                    },
                    |term| term.name.clone(),
                )
        };
        let spelled = e.terms.get(name).map(Vec::as_slice).unwrap_or_default();
        match (t.single, spelled.first()) {
            (true, Some((s, _))) => set(name, Value::from(shown(s))),
            (true, None) => {}
            (false, _) => {
                let list: Vec<Value> = spelled.iter().map(|(s, _)| Value::from(shown(s))).collect();
                set(name, Value::from(list));
            }
        }
    }
    Value::from(m)
}

/// A field value. Dates and datetimes are ISO text, which sorts in date
/// order and which `dateformat` reads.
fn value(v: &fields::Value) -> Value {
    use fields::Value as F;
    match v {
        F::String(s) | F::Url(s) => Value::from(s.as_str()),
        F::Int(i) => Value::from(*i),
        F::Float(f) => Value::from(*f),
        F::Bool(b) => Value::from(*b),
        F::Date(d) => Value::from(d.to_string()),
        F::Datetime(z) => Value::from(z.strftime("%Y-%m-%dT%H:%M:%S%:z").to_string()),
        // `image()` turns these into images with #63.
        F::Image(Image::Local(path) | Image::Remote(path)) => Value::from(path.as_str()),
        F::List(items) => Value::from(items.iter().map(value).collect::<Vec<_>>()),
        F::Object(o) => Value::from(
            o.iter()
                .map(|(k, v)| (k.clone(), value(v)))
                .collect::<BTreeMap<String, Value>>(),
        ),
    }
}

/// `[site.extra]`, as it is written.
fn toml(v: &toml::Value) -> Value {
    match v {
        toml::Value::String(s) => Value::from(s.as_str()),
        toml::Value::Integer(i) => Value::from(*i),
        toml::Value::Float(f) => Value::from(*f),
        toml::Value::Boolean(b) => Value::from(*b),
        toml::Value::Datetime(d) => Value::from(d.to_string()),
        toml::Value::Array(a) => Value::from(a.iter().map(toml).collect::<Vec<_>>()),
        toml::Value::Table(t) => Value::from(
            t.iter()
                .map(|(k, v)| (k.clone(), toml(v)))
                .collect::<BTreeMap<String, Value>>(),
        ),
    }
}
