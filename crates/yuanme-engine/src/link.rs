//! The link pass: a Page or asset names another asset through a placeholder
//! until that asset's bytes, and so its hashed name, are final (#24).

use std::collections::BTreeMap;

use rayon::prelude::*;

use crate::{Body, Output};

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
/// its fonts, because its own name depends on theirs.
pub(crate) fn link(pages: Vec<(String, Vec<u8>)>, assets: Vec<Asset>) -> BTreeMap<String, Output> {
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
    let pages: Vec<_> = pages
        .into_par_iter()
        .map(|(path, bytes)| {
            let bytes = substitute(&bytes, &urls);
            let hash = blake3::hash(&bytes);
            let body = Body::Bytes(bytes.into());
            (path, Output { body, hash })
        })
        .collect();
    files.extend(pages);
    files
}

fn substitute(bytes: &[u8], urls: &[String]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut rest = bytes;
    while let Some(start) = rest.iter().position(|&b| b == 0) {
        out.extend_from_slice(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after
            .iter()
            .position(|&b| b == 0)
            .expect("a placeholder is closed");
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
