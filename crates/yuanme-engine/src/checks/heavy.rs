//! #15's rule on unoptimized images: a raster over 50 KB that a Page or
//! stylesheet loads as it is, because no variant was made of it, is a
//! warning, unless `[images] allow_unoptimized` lists it.

use std::collections::BTreeMap;

use yuanme_content::config;
use yuanme_diag::{Diagnostic, Span, codes};

use crate::image::resolve::{RASTERS, Used};
use crate::{Body, Inputs, Output};

/// #15's rule: a raster over this many bytes that a Page loads as it is.
const UNOPTIMIZED: u64 = 50_000;

/// Why a raster is served as it is.
#[derive(Clone, Copy)]
pub(crate) enum Unprocessed {
    Static,
    Animated,
    /// Copied with a hash because a stylesheet's `url()` or a `<link>`
    /// names it.
    Copied,
}

/// A raster over [`UNOPTIMIZED`] that the build serves as it is.
pub(crate) struct Heavy {
    /// What `[images] allow_unoptimized` names it by: a Static file's URL,
    /// or else its source's path from the project root, after a `/`.
    url: String,
    size: u64,
    why: Unprocessed,
    /// Where an animated image is first named, which its Page's text
    /// can't show: the Page names its copy.
    at: Option<Span>,
}

/// Every raster of `files` over [`UNOPTIMIZED`] that no variant made and
/// `allow_unoptimized` doesn't list, by its path in the output directory.
pub(crate) fn heavy(
    inputs: &Inputs,
    config: &config::Images,
    files: &BTreeMap<String, Output>,
    used: &Used,
) -> BTreeMap<String, Heavy> {
    let mut by_hash = BTreeMap::new();
    for (path, input) in inputs.iter() {
        by_hash.entry(*input.hash.as_bytes()).or_insert(path);
    }
    let mut out = BTreeMap::new();
    for (path, output) in files {
        let ext = path
            .rsplit_once('.')
            .map_or(String::new(), |(_, e)| e.to_ascii_lowercase());
        if !RASTERS.contains(&ext.as_str()) || used.variants.contains_key(path) {
            continue;
        }
        let (why, source) = if let Some(source) = used.copies.get(path) {
            (Unprocessed::Animated, source.as_str())
        } else if inputs.files.contains_key(&format!("static/{path}")) {
            (Unprocessed::Static, path.as_str())
        } else if path.starts_with("assets/") {
            let source = by_hash.get(output.hash.as_bytes()).copied();
            (Unprocessed::Copied, source.unwrap_or(path))
        } else {
            continue;
        };
        let size = match &output.body {
            Body::Bytes(bytes) => bytes.len() as u64,
            Body::Input(p) => inputs
                .root
                .as_ref()
                .and_then(|root| std::fs::metadata(root.join(p)).ok())
                .map_or(0, |m| m.len()),
            Body::Cached(p) => std::fs::metadata(p).map_or(0, |m| m.len()),
        };
        let url = format!("/{source}");
        if size > UNOPTIMIZED && !config.allow_unoptimized.contains(&url) {
            let at = used.at.get(source).cloned();
            out.insert(path.clone(), Heavy { url, size, why, at });
        }
    }
    out
}

/// The warning for a Page that loads `heavy`, which it names as `url` at
/// `span`.
pub(crate) fn unoptimized(url: &str, heavy: &Heavy, span: Span) -> Diagnostic {
    let (url, span) = match &heavy.at {
        Some(at) => (heavy.url.as_str(), at.clone()),
        None => (url, span),
    };
    let why = match heavy.why {
        Unprocessed::Static => "a Static file is served as it is",
        Unprocessed::Animated => "an animated image is copied as it is",
        Unprocessed::Copied => "an image a stylesheet or a `<link>` names is copied as it is",
    };
    let message = format!(
        "`{url}` loads {} KB at full size: {why}",
        heavy.size.div_ceil(1000)
    );
    let fix = match heavy.why {
        Unprocessed::Static => {
            "move it out of `static/` and write it with `image()`, which makes smaller copies"
        }
        Unprocessed::Animated => "make it a video, or a smaller animation",
        Unprocessed::Copied => "make it smaller, or write it in a Template with `image()`",
    };
    Diagnostic::warning(&codes::IMAGE_UNOPTIMIZED, message, span).help(format!(
        "{fix}; or, if it must be served as it is, list \"{}\" in `[images] allow_unoptimized`",
        heavy.url
    ))
}

/// The warnings for the stylesheets that load a raster of `heavy`, each at
/// the first stylesheet of the project that names its file, or else at
/// Site config.
pub(crate) fn in_css(
    inputs: &Inputs,
    files: &BTreeMap<String, Output>,
    heavy: &BTreeMap<String, Heavy>,
    sources: &mut yuanme_diag::Files,
    fallback: &Span,
) -> Vec<Diagnostic> {
    let stylesheets: Vec<(&String, &str)> = files
        .iter()
        .filter(|(p, _)| p.ends_with(".css"))
        .filter_map(|(p, o)| match &o.body {
            Body::Bytes(b) => Some((p, std::str::from_utf8(b).ok()?)),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    for (path, h) in heavy {
        let url = format!("/{path}");
        let named = |css: &str| {
            css.match_indices(&url)
                .any(|(i, _)| css[..i].ends_with(['(', '"', '\'']))
        };
        let Some((stylesheet, _)) = stylesheets.iter().find(|(_, css)| named(css)) else {
            continue;
        };
        let name = h.url.rsplit('/').next().unwrap_or(&h.url);
        let written = inputs
            .iter()
            .filter(|(p, _)| p.ends_with(".css"))
            .find_map(|(p, input)| {
                let text = std::str::from_utf8(input.bytes.as_deref()?).ok()?;
                Some((p, text, text.find(name)?))
            });
        let d = match written {
            Some((p, text, at)) => {
                let file = sources.add(p, text);
                let span = Span {
                    file,
                    range: at..at + name.len(),
                };
                unoptimized(name, h, span)
            }
            None => unoptimized(&url, h, fallback.clone()),
        };
        out.push(d.note(format!("in the stylesheet written to `{stylesheet}`")));
    }
    out
}
