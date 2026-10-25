//! What `image()` returns and the `<picture>` it becomes (#28). The Engine
//! names and sizes an image's outputs; this writes them as markup, for
//! body images and the `picture` filter alike.

use yuanme_diag::Code;

use crate::template::escape;

/// One image's outputs, as `image()` returns them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Image {
    /// The size of `url`. `None` only for an SVG that doesn't say.
    pub size: Option<(u32, u32)>,
    /// The largest fallback, from the Site root.
    pub url: String,
    /// Empty for an image copied as it is, such as an SVG.
    pub srcset: String,
    pub sizes: String,
    /// Each `<source>`, by MIME type, before the fallback.
    pub sources: Vec<(String, String)>,
}

/// The keyword arguments of `image()` that choose its outputs. `None` is
/// `[images]`'s setting.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Request {
    pub widths: Option<Vec<u32>>,
    /// The `<source>` formats.
    pub formats: Option<Vec<String>>,
    /// One format for every output, with no `<source>`s.
    pub format: Option<String>,
    pub quality: Option<u8>,
    pub sizes: Option<String>,
}

/// Why `image()` can't give an Image: a Diagnostic's code and message.
pub type Error = (&'static Code, String);

/// How the `picture` filter or a body writes one Image.
pub struct Picture<'a> {
    pub alt: &'a str,
    /// `fetchpriority="high"` and no `loading`, for the LCP image.
    pub priority: bool,
    /// `lazy` or `eager`; ignored with `priority`.
    pub loading: &'a str,
    /// More attributes for the `<img>`, such as `class`, already escaped.
    pub attrs: String,
}

/// `<picture>` with a `<source>` per format, or `<img>` alone when there
/// is one format. `width` and `height` keep the layout from shifting.
pub fn picture(image: &Image, p: &Picture<'_>) -> String {
    let mut out = String::new();
    if !image.sources.is_empty() {
        out.push_str("<picture>");
        for (ty, srcset) in &image.sources {
            out.push_str(&format!(
                "<source type=\"{}\" srcset=\"{}\" sizes=\"{}\">",
                escape(ty),
                escape(srcset),
                escape(&image.sizes)
            ));
        }
    }
    out.push_str(&format!("<img src=\"{}\"", escape(&image.url)));
    if !image.srcset.is_empty() {
        out.push_str(&format!(
            " srcset=\"{}\" sizes=\"{}\"",
            escape(&image.srcset),
            escape(&image.sizes)
        ));
    }
    if let Some((width, height)) = image.size {
        out.push_str(&format!(" width=\"{width}\" height=\"{height}\""));
    }
    out.push_str(&format!(" alt=\"{}\"", escape(p.alt)));
    match p.priority {
        true => out.push_str(" fetchpriority=\"high\""),
        false => out.push_str(&format!(" loading=\"{}\"", escape(p.loading))),
    }
    out.push_str(" decoding=\"async\"");
    out.push_str(&p.attrs);
    out.push('>');
    if !image.sources.is_empty() {
        out.push_str("</picture>");
    }
    out
}

/// The `<link rel="preload">` that fetches a priority Image from the head,
/// in the format a browser picks first.
pub fn preload(image: &Image) -> String {
    let (ty, srcset) = match image.sources.first() {
        Some((ty, srcset)) => (format!(" type=\"{}\"", escape(ty)), srcset),
        None => (String::new(), &image.srcset),
    };
    match srcset.is_empty() {
        true => format!(
            "<link rel=\"preload\" as=\"image\" href=\"{}\" fetchpriority=\"high\">",
            escape(&image.url)
        ),
        false => format!(
            "<link rel=\"preload\" as=\"image\"{ty} imagesrcset=\"{}\" imagesizes=\"{}\" fetchpriority=\"high\">",
            escape(srcset),
            escape(&image.sizes)
        ),
    }
}
