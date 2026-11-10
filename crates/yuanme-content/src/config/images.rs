//! `[images]`: the widths, formats and qualities of image variants.

use super::check::Checker;
use super::{Table, int, items, string, strings, table};

/// `[images]` (#28). Each quality is on its encoder's own scale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Images {
    /// Ascending, each once.
    pub widths: Vec<u32>,
    pub sizes: String,
    /// The `<source>` formats, `"avif"` or `"webp"`, in order: each once.
    pub formats: Vec<String>,
    pub avif_speed: u8,
    pub avif_quality: u8,
    pub jpeg_quality: u8,
    pub webp_quality: u8,
    /// URLs from the root, such as `/favicon.png`, of rasters a Page may
    /// load as they are, which `image/unoptimized` doesn't warn of.
    pub allow_unoptimized: Vec<String>,
}

pub(super) fn read(images: Option<&Table<'_>>, checker: &mut Checker) -> Images {
    let get = |key: &str| images.and_then(|t| t.get(key));
    let quality = images.and_then(|t| table(t, "quality"));
    let number = |t: Option<&Table<'_>>, key: &str, default: u8| {
        t.and_then(|t| t.get(key)).map_or(default, |v| {
            u8::try_from(int(v)).expect("checked: 1 to 100")
        })
    };
    let mut widths: Vec<u32> = match get("widths") {
        Some(v) => items(v)
            .iter()
            .map(|w| u32::try_from(int(w)).expect("checked: 1 to 16383"))
            .collect(),
        None => vec![480, 800, 1200, 1600, 2400],
    };
    if let Some(v) = get("widths")
        && widths.is_empty()
    {
        checker.invalid("`images.widths` needs at least one width".into(), v.span());
    }
    widths.sort_unstable();
    widths.dedup();
    let mut formats: Vec<String> = Vec::new();
    match get("formats") {
        Some(v) => {
            for f in items(v) {
                let name = f.get_ref().as_str().expect("checked: a choice").to_string();
                if formats.contains(&name) {
                    checker.invalid(format!("`images.formats` lists \"{name}\" twice"), f.span());
                }
                formats.push(name);
            }
        }
        None => formats.push("avif".into()),
    }
    Images {
        widths,
        sizes: images
            .and_then(|t| string(t, "sizes"))
            .unwrap_or_else(|| "100vw".into()),
        formats,
        avif_speed: number(images, "avif_speed", 6),
        avif_quality: number(quality, "avif", 72),
        jpeg_quality: number(quality, "jpeg", 80),
        webp_quality: number(quality, "webp", 80),
        allow_unoptimized: get("allow_unoptimized").map(strings).unwrap_or_default(),
    }
}
