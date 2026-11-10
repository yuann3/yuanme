//! `yuanme routes`: every Route and what produces it.

use std::path::Path;

use yuanme_content::site::RouteKind;
use yuanme_engine::Mode;

use crate::events::{self, Event};
use crate::out::{Out, stdout};
use crate::project::{engine_build, site_or_report};
use crate::{Error, SITE_ERRORS};

pub(crate) fn routes(root: &Path, drafts: bool, out: &Out) -> Result<u8, Error> {
    let run = engine_build(root, None, Mode::Check { drafts }, false, false)?;
    let Some((config, site)) = site_or_report(&run, out) else {
        return Ok(SITE_ERRORS);
    };
    let templates = yuanme_render::template::route_templates(config, site);
    let routes: Vec<events::Route> = site
        .routes
        .iter()
        .zip(templates)
        .map(|(r, template)| {
            let mut route = events::Route {
                url: r.url.clone(),
                output: r.output.clone(),
                kind: "",
                template,
                entry: None,
                collection: None,
                taxonomy: None,
                term: None,
                page: None,
                name: None,
            };
            match &r.kind {
                RouteKind::Entry(i) => {
                    route.kind = "entry";
                    route.entry = Some(site.entries[*i].path.clone());
                    route.collection = Some(site.entries[*i].collection.clone());
                }
                RouteKind::List {
                    collection, page, ..
                } => {
                    route.kind = "list";
                    route.collection = Some(collection.clone());
                    route.page = Some(*page);
                }
                RouteKind::Term {
                    taxonomy,
                    term,
                    page,
                    ..
                } => {
                    route.kind = "term";
                    route.term = Some(site.taxonomies[taxonomy][*term].name.clone());
                    route.taxonomy = Some(taxonomy.clone());
                    route.page = Some(*page);
                }
                RouteKind::Terms { taxonomy } => {
                    route.kind = "index";
                    route.taxonomy = Some(taxonomy.clone());
                }
                RouteKind::Template(name) => {
                    route.kind = "template";
                    route.name = Some(name.clone());
                }
                RouteKind::Feed { name, .. } => {
                    route.kind = "feed";
                    route.name = Some(name.clone());
                }
            }
            route
        })
        .collect();
    if out.json {
        for route in routes {
            out.event(Event::Route(route));
        }
        return Ok(0);
    }
    let from = |r: &events::Route| {
        let page = |p: Option<usize>| match p {
            Some(n) if n > 1 => format!(", page {n}"),
            _ => String::new(),
        };
        match r.kind {
            "entry" => r.entry.clone().unwrap_or_default(),
            "list" => format!(
                "{} list{}",
                r.collection.as_deref().unwrap_or(""),
                page(r.page)
            ),
            "term" => format!(
                "{} `{}`{}",
                r.taxonomy.as_deref().unwrap_or(""),
                r.term.as_deref().unwrap_or(""),
                page(r.page)
            ),
            "index" => format!("{} index", r.taxonomy.as_deref().unwrap_or("")),
            _ => r.name.clone().unwrap_or_default(),
        }
    };
    let rows: Vec<[String; 4]> = routes
        .iter()
        .map(|r| {
            [
                r.url.clone(),
                r.kind.to_string(),
                from(r),
                r.template.clone(),
            ]
        })
        .collect();
    let width = |i: usize| rows.iter().map(|r| r[i].chars().count()).max().unwrap_or(0);
    let widths = [width(0), width(1), width(2)];
    for row in &rows {
        let mut line = String::new();
        for (cell, w) in row.iter().zip(widths) {
            line.push_str(&format!("{cell:w$}  "));
        }
        line.push_str(&row[3]);
        stdout(&line);
    }
    Ok(0)
}
