//! The Site's images (#28): what each body image becomes. A raster is
//! resized and encoded through [`crate::image`], named before render so a
//! Page can point at its variants; an SVG or an animated image is copied
//! with a hash instead.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use yuanme_content::config::{self, Config};
use yuanme_content::document::{Document, Kind, NodeId};
use yuanme_content::entry::SourceFormat;
use yuanme_content::site::Site;
use yuanme_diag::{Diagnostic, Span, codes};
use yuanme_render::html::{self, Pictures};
use yuanme_render::image::{Error, Image, Picture, Request, picture};

use crate::assets::{has_scheme, normalize, percent_decode};
use crate::image::{self, Format, Probe, Settings, Variant};
use crate::link::hashed_path;
use crate::{Body, Cache, Inputs, Output};

/// What the pipeline makes of one image file.
enum Source {
    Raster(Probe),
    /// Copied as it is, with its size when it says: an SVG, an AVIF, which
    /// the pipeline can't decode, or an animated image.
    Copy(Option<(u32, u32)>),
    /// Why it can't be processed.
    Invalid(String),
}

/// What the Site's Pages use, for the build to encode or copy.
#[derive(Default)]
pub(crate) struct Used {
    /// Every variant a Page names, by its path, with the settings that
    /// encode it.
    pub variants: BTreeMap<String, (Variant, Settings)>,
    /// Images copied with a hash, by path, from their source.
    pub copies: BTreeMap<String, String>,
    /// Where each source is first named, for an error encoding it.
    pub at: BTreeMap<String, Span>,
}

/// Every image of the project, probed, and what render has used.
pub(crate) struct Images {
    /// Holds no entries: it names variants with the build's Engine version.
    cache: Cache,
    /// Each image file's hash, which names its variants.
    inputs: Inputs,
    sources: BTreeMap<String, Source>,
    /// Every Static file, which is copied as it is, unprobed.
    statics: BTreeSet<String>,
    config: config::Images,
    used: Mutex<Used>,
}

const RASTERS: &[&str] = &["gif", "jpeg", "jpg", "png", "webp"];
const COPIED: &[&str] = &["avif", "ico", "svg"];

impl Images {
    /// Probes every image of the project but the Static files, before
    /// render, since a probe writes the cache: it reads only a header, and
    /// only on a cache miss.
    pub fn new(inputs: &Inputs, cache: &mut Cache, config: &Config) -> Images {
        let mut names = Inputs::default();
        let mut sources = BTreeMap::new();
        let mut statics = BTreeSet::new();
        for (path, input) in inputs.iter() {
            if path.starts_with("static/") {
                statics.insert(path.to_string());
                continue;
            }
            let ext = path
                .rsplit_once('.')
                .map_or(String::new(), |(_, e)| e.to_ascii_lowercase());
            let source = if RASTERS.contains(&ext.as_str()) {
                match image::probe(cache, inputs, path) {
                    Ok(p) if p.animated => Source::Copy(Some((p.width, p.height))),
                    Ok(p) => Source::Raster(p),
                    Err(e) => Source::Invalid(e.to_string()),
                }
            } else if COPIED.contains(&ext.as_str()) {
                let svg = input.bytes.as_deref().filter(|_| ext == "svg");
                Source::Copy(svg.and_then(svg_size))
            } else {
                continue;
            };
            names.insert_streamed(path, input.hash);
            sources.insert(path.to_string(), source);
        }
        Images {
            cache: Cache::memory(cache.engine_version()),
            inputs: names,
            sources,
            statics,
            config: config.images.clone(),
            used: Mutex::new(Used::default()),
        }
    }

    pub fn into_used(self) -> Used {
        self.used.into_inner().expect("recording doesn't panic")
    }

    /// The Image `path`, from the project root, makes. `at` is where it is
    /// named, if that is known.
    pub fn image(&self, path: &str, request: &Request, at: Option<&Span>) -> Result<Image, Error> {
        let source = self.sources.get(path).ok_or_else(|| {
            let mut message = format!("no image at `{path}`");
            if let Some(near) = config::similar(path, self.sources.keys().map(String::as_str)) {
                message.push_str(&format!("; did you mean `{near}`?"));
            }
            (&codes::IMAGE_NOT_FOUND, message)
        })?;
        let mut used = self.used.lock().expect("recording doesn't panic");
        if let Some(at) = at {
            used.at
                .entry(path.to_string())
                .or_insert_with(|| at.clone());
        }
        let probe = match source {
            Source::Invalid(why) => {
                return Err((&codes::IMAGE_INVALID, format!("`{path}`: {why}")));
            }
            Source::Copy(size) => {
                let name = path.rsplit('/').next().unwrap_or(path);
                let copy = hashed_path(name, &self.inputs.files[path].hash);
                let url = format!("/{copy}");
                used.copies.insert(copy, path.to_string());
                return Ok(Image {
                    size: *size,
                    url,
                    ..Image::default()
                });
            }
            Source::Raster(probe) => *probe,
        };
        let c = &self.config;
        let quality = request.quality;
        let settings = Settings {
            avif_quality: quality.unwrap_or(c.avif_quality),
            avif_speed: c.avif_speed,
            jpeg_quality: quality.unwrap_or(c.jpeg_quality),
            webp_quality: quality.unwrap_or(c.webp_quality),
        };
        // A source that can hold transparency keeps it.
        let fallback = if probe.alpha {
            Format::Png
        } else {
            Format::Jpeg
        };
        let (offered, main) = match &request.format {
            Some(f) => (Vec::new(), format(f)),
            None => {
                let names = request.formats.as_ref().unwrap_or(&c.formats);
                (names.iter().map(|f| format(f)).collect(), fallback)
            }
        };
        let mut formats = offered.clone();
        formats.push(main);
        let widths = request.widths.as_ref().unwrap_or(&c.widths);
        let variants = image::variants(
            &self.cache,
            &self.inputs,
            path,
            probe,
            widths,
            &formats,
            &settings,
        );
        let srcset = |f: Format| {
            let candidates: Vec<String> = variants
                .iter()
                .filter(|v| v.format == f)
                .map(|v| format!("/{} {}w", v.path, v.width))
                .collect();
            candidates.join(", ")
        };
        let largest = variants
            .iter()
            .rfind(|v| v.format == main)
            .expect("every width has the main format");
        let image = Image {
            size: Some((largest.width, largest.height)),
            url: format!("/{}", largest.path),
            srcset: srcset(main),
            sizes: request.sizes.clone().unwrap_or_else(|| c.sizes.clone()),
            sources: offered
                .iter()
                .map(|&f| (mime(f).to_string(), srcset(f)))
                .collect(),
        };
        for v in variants {
            used.variants.insert(v.path.clone(), (v, settings.clone()));
        }
        Ok(image)
    }

    /// The markup of every processed image in each Entry's body. The first
    /// image of a body is eager, as one above the fold must be; the others
    /// are lazy.
    pub fn bodies(&self, site: &Site) -> (Vec<Pictures>, Vec<Diagnostic>) {
        let mut diagnostics = Vec::new();
        let pictures = site
            .entries
            .iter()
            .map(|e| {
                let doc = &e.document;
                let dir = e.path.rsplit_once('/').map_or("", |(d, _)| d);
                let captions = captions(doc);
                let mut pictures = Pictures::new();
                let mut first = true;
                for id in doc.descendants(Document::ROOT) {
                    let node = doc.node(id);
                    let Kind::Image { url, title } = &node.kind else {
                        continue;
                    };
                    let eager = std::mem::replace(&mut first, false);
                    let span = Span {
                        file: e.file,
                        range: node.span.clone(),
                    };
                    let Some(path) = local(url, dir) else {
                        continue;
                    };
                    // A Static file's URL, which is copied unprocessed (#20).
                    if url.starts_with('/')
                        && !self.sources.contains_key(&path)
                        && self.statics.contains(&format!("static/{path}"))
                    {
                        continue;
                    }
                    let image = match self.image(&path, &Request::default(), Some(&span)) {
                        Ok(image) => image,
                        Err((code, message)) => {
                            let mut d = Diagnostic::error(code, message, span);
                            if code == &codes::IMAGE_NOT_FOUND {
                                d = d.help("a relative path starts from the Entry's folder, and one starting with `/` from the project root");
                            }
                            diagnostics.push(d);
                            continue;
                        }
                    };
                    let org = e.format == SourceFormat::Org;
                    let mut alt = html::alt(doc, id);
                    // Emacs's alt text when nothing else gives one.
                    let named = url.rsplit('/').next().unwrap_or(url);
                    if org && alt == named {
                        match captions.get(&id) {
                            Some(caption) => alt = caption.clone(),
                            None => alt.clear(),
                        }
                    }
                    if alt.trim().is_empty() {
                        let help = match org {
                            true => "describe what it shows in a `#+CAPTION:`, or `#+ATTR_HTML: :alt …`, above it",
                            false => "describe what it shows between the brackets: `![a chart of …](…)`",
                        };
                        diagnostics.push(
                            Diagnostic::warning(
                                &codes::IMAGE_MISSING_ALT,
                                format!("`{named}` has no alt text"),
                                Span { file: e.file, range: node.span.clone() },
                            )
                            .help(help),
                        );
                        // Emacs's name, as ox-html writes it.
                        if org {
                            alt = named.to_string();
                        }
                    }
                    let mut attrs = String::new();
                    for (name, value) in title.iter().map(|t| ("title", t)).chain(
                        node.attrs.iter().map(|(n, v)| (n.as_str(), v)),
                    ) {
                        attrs.push_str(&format!(" {name}=\"{}\"", yuanme_render::template::escape(value)));
                    }
                    let markup = picture(
                        &image,
                        &Picture {
                            alt: &alt,
                            priority: false,
                            loading: if eager { "eager" } else { "lazy" },
                            attrs,
                        },
                    );
                    pictures.insert(id, markup);
                }
                pictures
            })
            .collect();
        (pictures, diagnostics)
    }
}

/// The project file a body image's URL names, from the Entry's folder
/// `dir`; `None` for a remote URL, which is left as written.
fn local(url: &str, dir: &str) -> Option<String> {
    let path = url.split(['?', '#']).next().unwrap_or_default();
    if path.is_empty() || url.starts_with("//") || has_scheme(url) {
        return None;
    }
    let absolute = match path.strip_prefix('/') {
        Some(_) => normalize(path),
        None => normalize(&format!("/{dir}/{path}")),
    };
    Some(percent_decode(&absolute[1..]).unwrap_or_else(|| absolute[1..].to_string()))
}

/// The text of the caption of each image in a figure, by the image.
fn captions(doc: &Document) -> BTreeMap<NodeId, String> {
    let mut out = BTreeMap::new();
    for id in doc.descendants(Document::ROOT) {
        if doc.node(id).kind != Kind::Figure {
            continue;
        }
        let children = &doc.node(id).children;
        let caption = children
            .iter()
            .find(|&&c| doc.node(c).kind == Kind::Caption);
        let Some(&caption) = caption else { continue };
        for &c in children {
            if matches!(doc.node(c).kind, Kind::Image { .. }) {
                out.insert(c, html::alt(doc, caption));
            }
        }
    }
    out
}

fn format(name: &str) -> Format {
    match name {
        "avif" => Format::Avif,
        "webp" => Format::Webp,
        "jpeg" => Format::Jpeg,
        "png" => Format::Png,
        _ => unreachable!("checked: a format"),
    }
}

fn mime(format: Format) -> &'static str {
    match format {
        Format::Avif => "image/avif",
        Format::Jpeg => "image/jpeg",
        Format::Png => "image/png",
        Format::Webp => "image/webp",
    }
}

/// An SVG's size, from its root's `width` and `height` in pixels, or else
/// its `viewBox`.
fn svg_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let text = std::str::from_utf8(bytes).ok()?;
    let start = text.find("<svg")?;
    let tag = &text[start..start + text[start..].find('>')?];
    let attribute = |name: &str| {
        let at = tag.find(&format!(" {name}="))? + name.len() + 2;
        let quote = tag[at..].chars().next()?;
        let value = &tag[at + 1..];
        Some(&value[..value.find(quote)?])
    };
    let pixels = |v: &str| {
        let v = v.trim();
        let v = v.strip_suffix("px").unwrap_or(v);
        v.parse::<f64>().ok().filter(|n| *n > 0.0)
    };
    let (width, height) = match (attribute("width"), attribute("height")) {
        (Some(w), Some(h)) if pixels(w).is_some() && pixels(h).is_some() => {
            (pixels(w)?, pixels(h)?)
        }
        _ => {
            let view: Vec<f64> = attribute("viewBox")?
                .split([' ', ','])
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse().ok())
                .collect();
            let [_, _, w, h] = view[..] else { return None };
            (w, h)
        }
    };
    Some((width.round() as u32, height.round() as u32)).filter(|&(w, h)| w > 0 && h > 0)
}

/// Each image copied with a hash, as the assets stage copies a file.
pub(crate) fn copies<'a>(
    inputs: &'a Inputs,
    used: &'a Used,
) -> impl Iterator<Item = (String, Output)> + 'a {
    used.copies.iter().map(|(path, source)| {
        let input = &inputs.files[source];
        let body = match &input.bytes {
            Some(bytes) => Body::Bytes(bytes.clone()),
            None => Body::Input(source.clone()),
        };
        let output = Output {
            body,
            hash: input.hash,
        };
        (path.clone(), output)
    })
}

/// Encodes every variant the Pages name into `files`, each source once,
/// with an error where a source is first named for each that fails.
pub(crate) fn encode(
    inputs: &Inputs,
    cache: &mut Cache,
    used: &Used,
    files: &mut BTreeMap<String, Output>,
    fallback: &Span,
) -> Vec<Diagnostic> {
    // `image()` can set a quality of its own, so not every variant has the
    // same settings.
    let mut groups: Vec<(&Settings, Vec<Variant>)> = Vec::new();
    for (variant, settings) in used.variants.values() {
        match groups.iter_mut().find(|(s, _)| *s == settings) {
            Some((_, group)) => group.push(variant.clone()),
            None => groups.push((settings, vec![variant.clone()])),
        }
    }
    let mut failed = BTreeMap::new();
    for (settings, variants) in groups {
        let outputs = image::encode(cache, inputs, &variants, settings);
        for (variant, output) in variants.into_iter().zip(outputs) {
            match output {
                Ok(output) => {
                    files.insert(variant.path, output);
                }
                Err(e) => {
                    failed.entry(variant.source).or_insert(e);
                }
            }
        }
    }
    failed
        .into_iter()
        .map(|(source, e)| {
            let at = used.at.get(&source).unwrap_or(fallback).clone();
            Diagnostic::error(&codes::IMAGE_INVALID, format!("`{source}`: {e}"), at)
        })
        .collect()
}
