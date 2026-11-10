//! The stylesheets added once the assets stage is done, each linked only
//! from the HTML Pages that need it: one for the classes of the
//! highlighted code the Pages show (#53), and the math font with its
//! stylesheet (#17, #52).

use std::collections::BTreeSet;
use std::sync::Arc;

use rayon::prelude::*;
use yuanme_assets::{NCM_MATH, subset_font, woff2};
use yuanme_content::site::Site;

use crate::assets::Seen;
use crate::hashing::{Asset, hashed_path, placeholder};
use crate::{Body, Cache, Mode};

/// The stylesheet for every highlighted class the Pages use, as an asset,
/// with a `<link>` to it at the end of the head of each HTML Page that
/// shows code, as math's stylesheet goes only to Pages with math (#17).
/// It comes after the assets stage's stylesheets, as a Sidecar's does.
pub(crate) fn code(
    pages: &mut [(String, Vec<u8>)],
    seen: &[Option<Arc<Seen>>],
    assets: &mut Vec<Asset>,
) {
    let mut classes = BTreeSet::new();
    let mut showing = Vec::new();
    for (i, seen) in seen.iter().enumerate() {
        if let Some(used) = seen.as_ref().and_then(|s| s.code.as_ref()) {
            classes.extend(used.iter().map(String::as_str));
            showing.push(i);
        }
    }
    if showing.is_empty() {
        return;
    }
    let css = yuanme_render::highlight::css(classes);
    let tag = format!(
        "<link rel=\"stylesheet\" href=\"{}\">",
        placeholder(assets.len())
    );
    assets.push(Asset::Linked {
        name: "code.css".into(),
        bytes: css.into_bytes(),
    });
    link(pages, seen, &tag, |s| s.code.is_some());
}

const KIND: &str = "math-font";

/// Characters every subset keeps, whether or not the math shows them (#15).
/// Browsers measure U+00AF, `1` and U+2212 to lay math out and draw a
/// no-break space with U+0020, so without them the math moves; U+221A
/// stands in for every radical. macOS anti-aliases every glyph of a font
/// without `o` and `X` a little differently (found by bisecting a subset,
/// not documented): with them, a subset draws as the whole font does.
const ALWAYS: [char; 7] = [' ', '\u{AF}', '1', 'X', 'o', '\u{2212}', '\u{221A}'];

/// math-core's own fixes for browsers' MathML (MIT, as `vendor/math-core`).
const MATHML_FIXES: &str = include_str!("styles/mathmlfixes.css");

/// The math font, as an asset, and a stylesheet for it, linked at the end
/// of the head of each HTML Page that shows math. A build gets one subset
/// for the whole Site, of every character its Entries' math shows; `yuanme
/// dev` serves the whole font, so an edit never waits for a subset. Returns
/// the font's path, for the math font budget (#15).
pub(crate) fn math(
    site: &Site,
    mode: Mode,
    cache: &mut Cache,
    pages: &mut [(String, Vec<u8>)],
    seen: &[Option<Arc<Seen>>],
    assets: &mut Vec<Asset>,
) -> Option<String> {
    let showing: Vec<usize> = (0..pages.len())
        .filter(|&i| seen[i].as_ref().is_some_and(|s| s.math))
        .collect();
    if showing.is_empty() {
        return None;
    }
    let (name, format, bytes) = if mode == Mode::Dev {
        ("math.otf", "opentype", Arc::from(NCM_MATH))
    } else {
        let mut chars: BTreeSet<char> = ALWAYS.into();
        for entry in &site.entries {
            chars.extend(entry.document.math_chars());
        }
        ("math.woff2", "woff2", subset(cache, &chars))
    };
    let hash = blake3::hash(&bytes);
    let path = hashed_path(name, &hash);
    assets.push(Asset::Copy {
        name: name.into(),
        hash,
        body: Body::Bytes(bytes),
    });
    let css = format!(
        "@font-face{{font-family:\"yuanme NCM Math\";src:url(\"{}\") format(\"{format}\");font-display:block}}\
         math{{font-family:\"yuanme NCM Math\",math}}\
         mtext span.math-core-sans-serif-font{{font-family:sans-serif}}\n{MATHML_FIXES}",
        placeholder(assets.len() - 1)
    );
    let tag = format!(
        "<link rel=\"stylesheet\" href=\"{}\">",
        placeholder(assets.len())
    );
    assets.push(Asset::Linked {
        name: "math.css".into(),
        bytes: css.into_bytes(),
    });
    link(pages, seen, &tag, |s| s.math);
    Some(path)
}

/// Adds `tag` at the end of the head of each HTML Page whose [`Seen`]
/// `shows` what the stylesheet it links is for.
fn link(
    pages: &mut [(String, Vec<u8>)],
    seen: &[Option<Arc<Seen>>],
    tag: &str,
    shows: impl Fn(&Seen) -> bool + Sync,
) {
    // Each moves the rest of its Page, so on threads of their own.
    let showing = pages
        .par_iter_mut()
        .zip(seen)
        .filter(|(_, s)| s.as_deref().is_some_and(&shows));
    showing.for_each(|((_, bytes), _)| {
        let at = bytes
            .windows(7)
            .position(|w| w.eq_ignore_ascii_case(b"</head>"))
            .unwrap_or(0);
        bytes.splice(at..at, tag.bytes());
    });
}

/// The subset WOFF2 for `chars`, through the cache: Brotli at quality 11
/// takes most of a second for a large subset.
fn subset(cache: &mut Cache, chars: &BTreeSet<char>) -> Arc<[u8]> {
    let mut key = cache.key(KIND);
    key.update(blake3::hash(NCM_MATH).as_bytes());
    for c in chars {
        key.update(&u32::from(*c).to_le_bytes());
    }
    let key = key.finalize();
    if let Some(bytes) = cache.get(KIND, &key) {
        return bytes;
    }
    let bytes: Arc<[u8]> = woff2(&subset_font(NCM_MATH, chars)).into();
    cache.insert(KIND, key, bytes.clone());
    bytes
}

/// Whether an HTML Page has a `<math>` element.
pub(crate) fn has_math(page: &[u8]) -> bool {
    memchr::memchr_iter(b'<', page).any(|at| {
        page.get(at + 1..at + 6).is_some_and(|tag| {
            tag[..4].eq_ignore_ascii_case(b"math") && matches!(tag[4], b'>' | b' ' | b'\n' | b'\t')
        })
    })
}
