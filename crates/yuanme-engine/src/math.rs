//! The math font and stylesheet, for the Pages that show math (#17, #52).

use std::collections::BTreeSet;
use std::sync::Arc;

use yuanme_assets::font::{self, NCM_MATH};
use yuanme_content::site::Site;

use crate::link::Asset;
use crate::{Body, Cache, Mode};

const KIND: &str = "math-font";

/// Characters every subset keeps, whether or not the math shows them (#15).
/// Browsers measure U+00AF, `1` and U+2212 to lay math out and draw a
/// no-break space with U+0020, so without them the math moves; U+221A
/// stands in for every radical. macOS anti-aliases every glyph of a font
/// without `o` and `X` a little differently (found by bisecting a subset,
/// not documented): with them, a subset draws as the whole font does.
const ALWAYS: [char; 7] = [' ', '\u{AF}', '1', 'X', 'o', '\u{2212}', '\u{221A}'];

/// math-core's own fixes for browsers' MathML (MIT, as `vendor/math-core`).
const MATHML_FIXES: &str = include_str!("math/mathmlfixes.css");

/// The math font, as an asset, and a stylesheet for it, linked at the end
/// of the head of each HTML Page that shows math. A build gets one subset
/// for the whole Site, of every character its Entries' math shows; `yuanme
/// dev` serves the whole font, so an edit never waits for a subset.
pub(crate) fn stylesheet(
    site: &Site,
    mode: Mode,
    cache: &mut Cache,
    pages: &mut [(String, Vec<u8>)],
    assets: &mut Vec<Asset>,
) {
    let showing: Vec<usize> = (0..pages.len())
        .filter(|&i| pages[i].0.ends_with(".html") && has_math(&pages[i].1))
        .collect();
    if showing.is_empty() {
        return;
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
    assets.push(Asset::Copy {
        name: name.into(),
        hash: blake3::hash(&bytes),
        body: Body::Bytes(bytes),
    });
    let css = format!(
        "@font-face{{font-family:\"yuanme NCM Math\";src:url(\"\0asset:{}\0\") format(\"{format}\");font-display:block}}\
         math{{font-family:\"yuanme NCM Math\",math}}\
         mtext span.math-core-sans-serif-font{{font-family:sans-serif}}\n{MATHML_FIXES}",
        assets.len() - 1
    );
    let tag = format!(
        "<link rel=\"stylesheet\" href=\"\0asset:{}\0\">",
        assets.len()
    );
    assets.push(Asset::Linked {
        name: "math.css".into(),
        bytes: css.into_bytes(),
    });
    for i in showing {
        let bytes = &mut pages[i].1;
        let at = bytes
            .windows(7)
            .position(|w| w.eq_ignore_ascii_case(b"</head>"))
            .unwrap_or(0);
        bytes.splice(at..at, tag.bytes());
    }
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
    let bytes: Arc<[u8]> = font::woff2(&font::subset(NCM_MATH, chars)).into();
    cache.insert(KIND, key, bytes.clone());
    bytes
}

/// Whether an HTML Page has a `<math>` element.
fn has_math(page: &[u8]) -> bool {
    page.windows(6).any(|w| {
        w[..5].eq_ignore_ascii_case(b"<math") && matches!(w[5], b'>' | b' ' | b'\n' | b'\t')
    })
}
