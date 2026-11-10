//! Each Entry's body as HTML, a leaf cache (ADR 0011), so a prose edit in
//! `yuanme dev` renders only the edited Entry's body. Only `yuanme dev`
//! keeps it, in memory, for its next build: on disk it would be another
//! pack the size of every body, rewritten after each save, and a build
//! that runs once would only hold every body twice.

use std::sync::Arc;

use rayon::prelude::*;
use yuanme_content::{Document, Links, Site};
use yuanme_render::html::{self, Pictures};

use super::highlight;
use crate::Cache;

/// One body and everything [`html::body`] read to write it, but its code
/// blocks, which are highlighted from the Document by this Engine.
pub(crate) struct Rendered {
    /// The very Document, not an equal one: a Document is immutable, and
    /// `yuanme dev` keeps each Entry it didn't read again, and so its
    /// Document, between builds (see `entries`). Comparing pointers costs
    /// nothing where hashing every Document would cost a whole read.
    document: Arc<Document>,
    links: Links,
    pictures: Pictures,
    html: String,
}

/// Each of the Site's Entries' bodies, in the order of `site.entries`,
/// from the cache when nothing it read changed. `keep` keeps them for the
/// next build.
pub(crate) fn bodies(
    site: &Site,
    code: &[highlight::Blocks],
    pictures: Vec<Pictures>,
    cache: &mut Cache,
    keep: bool,
) -> Vec<String> {
    // Only this build's, so the cache holds as many bodies as the Site has.
    let mut kept = std::mem::take(&mut cache.bodies);
    let read = &*cache;
    let fresh: Vec<Option<String>> = site
        .entries
        .par_iter()
        .zip(&site.links)
        .zip(code)
        .zip(&pictures)
        .map(|(((e, links), code), pictures)| {
            let same = kept.get(&e.path).is_some_and(|r| {
                Arc::ptr_eq(&r.document, &e.document)
                    && r.links == *links
                    && r.pictures == *pictures
            });
            (!same).then(|| {
                let code = highlight::of(read, code);
                html::body(&e.document, links, &code, pictures)
            })
        })
        .collect();
    let mut out = Vec::with_capacity(fresh.len());
    for (((e, links), pictures), fresh) in site
        .entries
        .iter()
        .zip(&site.links)
        .zip(pictures)
        .zip(fresh)
    {
        let html = match fresh {
            Some(html) => html,
            None => kept.remove(&e.path).expect("found above").html,
        };
        if keep {
            let rendered = Rendered {
                document: e.document.clone(),
                links: links.clone(),
                pictures,
                html: html.clone(),
            };
            cache.bodies.insert(e.path.clone(), rendered);
        }
        out.push(html);
    }
    out
}
