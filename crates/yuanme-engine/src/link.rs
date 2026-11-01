//! The link pass: a Page or asset names another asset through a placeholder
//! until that asset's bytes, and so its hashed name, are final (#24).

use std::collections::{BTreeMap, HashMap};

use rayon::prelude::*;

use crate::{Body, Cache, Output};

/// A file the Engine processed, such as a CSS bundle, which is named by the
/// hash of its bytes under `/assets/`.
pub(crate) enum Asset {
    /// Final already, such as a font. Its bytes may hold NUL, so they are
    /// never searched for placeholders.
    Copy {
        /// The name before hashing, such as `geist.woff2`.
        name: String,
        body: Body,
        hash: blake3::Hash,
    },
    /// Holds placeholders, as a stylesheet does for its fonts.
    Linked { name: String, bytes: Vec<u8> },
}

/// Swaps every placeholder for its asset's hashed URL and hashes every file.
/// Render and the assets stage write `\0asset:N\0` where the URL of
/// `assets[N]` goes; NUL can't come from an Author, since HTML and CSS
/// parsers replace it.
/// An asset may refer only to assets before it, as a stylesheet refers to
/// its fonts, because its own name depends on theirs. With `keep`, each
/// Page is kept in `cache` for the next build, which links again only a
/// Page whose bytes or assets' URLs changed.
pub(crate) fn link(
    pages: Vec<(String, Vec<u8>)>,
    assets: Vec<Asset>,
    cache: &mut Cache,
    keep: bool,
) -> BTreeMap<String, Output> {
    let mut urls = Vec::with_capacity(assets.len());
    let mut files = BTreeMap::new();
    for asset in assets {
        let (name, body, hash) = match asset {
            Asset::Copy { name, body, hash } => (name, body, hash),
            Asset::Linked { name, bytes } => {
                let bytes = substitute(&bytes, &urls);
                let hash = blake3::hash(&bytes);
                (name, Body::Bytes(bytes.into()), hash)
            }
        };
        let path = hashed_path(&name, &hash);
        urls.push(format!("/{path}"));
        files.insert(path, Output { body, hash });
    }
    let kept = std::mem::take(&mut cache.linked);
    let same = kept.urls == urls;
    let pages: Vec<_> = pages
        .into_par_iter()
        .map(|(path, before)| {
            let output = match kept.pages.get(&path) {
                Some((was, output)) if same && *was == before => output.clone(),
                _ => {
                    let bytes = substitute(&before, &urls);
                    let hash = blake3::hash(&bytes);
                    let body = Body::Bytes(bytes.into());
                    Output { body, hash }
                }
            };
            (path, before, output)
        })
        .collect();
    drop(kept);
    for (path, before, output) in pages {
        if keep {
            let page = (before, output.clone());
            cache.linked.pages.insert(path.clone(), page);
        }
        files.insert(path, output);
    }
    if keep {
        cache.linked.urls = urls;
    }
    files
}

/// The last dev build's Pages, by path, each as it was before and after
/// the link pass, and the assets' URLs they were linked with.
#[derive(Default)]
pub(crate) struct Linked {
    urls: Vec<String>,
    pages: HashMap<String, (Vec<u8>, Output)>,
}

fn substitute(bytes: &[u8], urls: &[String]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut rest = bytes;
    while let Some(start) = memchr::memchr(0, rest) {
        out.extend_from_slice(&rest[..start]);
        let after = &rest[start + 1..];
        let end = memchr::memchr(0, after).expect("a placeholder is closed");
        let id: usize = std::str::from_utf8(&after[..end])
            .ok()
            .and_then(|p| p.strip_prefix("asset:"))
            .and_then(|id| id.parse().ok())
            .expect("a placeholder names an asset");
        let url = urls
            .get(id)
            .expect("an asset refers only to assets before it");
        out.extend_from_slice(url.as_bytes());
        rest = &after[end + 1..];
    }
    out.extend_from_slice(rest);
    out
}

/// `site.css` → `assets/site-0123456789abcdef.css`. 64 bits keeps collisions
/// out of reach for any one Site.
pub(crate) fn hashed_path(name: &str, hash: &blake3::Hash) -> String {
    let hash = &hash.to_hex()[..16];
    match name.rsplit_once('.') {
        Some((stem, ext)) => format!("assets/{stem}-{hash}.{ext}"),
        None => format!("assets/{name}-{hash}"),
    }
}
