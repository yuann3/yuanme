//! The filters and functions the Engine adds to Jinja's (#19).

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{Timestamp, Zoned};
use minijinja::value::{Kwargs, Value, ValueKind};
use minijinja::{Environment, Error, ErrorKind, State};
use yuanme_content::document::{Document, Kind, Links};
use yuanme_content::{config, markdown, org};
use yuanme_diag::{Code, Diagnostic, Files, Level, codes};

use crate::highlight;
use crate::image::{self, Image, Images, Picture, Request};

/// What the filters read from the Site.
pub(super) struct Site {
    /// Without a trailing `/`.
    pub origin: String,
    pub timezone: TimeZone,
    /// Every path `url()` accepts.
    pub urls: BTreeSet<String>,
}

pub(super) fn add(env: &mut Environment<'_>, site: Site, images: Arc<dyn Images>) {
    let site = Arc::new(site);
    env.add_function("image", move |state: &State, src: &str, kw: Kwargs| {
        self::image(&*images, state, src, kw)
    });
    env.add_filter("picture", picture);
    env.add_filter("sort_by", sort_by);
    let s = site.clone();
    env.add_filter("dateformat", move |v: Value, kw: Kwargs| {
        dateformat(&s, &v, &kw)
    });
    let s = site.clone();
    env.add_function("url", move |path: &str| url(&s, path));
    let s = site.clone();
    env.add_function("absolute_url", move |path: &str| {
        absolute_url(&s.origin, path)
    });
    env.add_filter("pluralize", pluralize);
    env.add_filter("attrs", attrs);
    env.add_filter("markdown", markdown);
    env.add_filter("org", org);
    // Jinja's own `escape` escapes `/`, which the formatter doesn't (#19).
    for name in ["escape", "e"] {
        env.add_filter(name, |v: Value| match v.is_safe() {
            true => v,
            false => Value::from_safe_string(escape(&v.to_string())),
        });
    }
}

/// A Diagnostic code carried through minijinja's error, as its source.
#[derive(Debug)]
pub(super) struct Coded(pub &'static Code);

impl fmt::Display for Coded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.name)
    }
}

impl std::error::Error for Coded {}

pub(super) fn coded(code: &'static Code, message: String) -> Error {
    Error::new(ErrorKind::InvalidOperation, message).with_source(Coded(code))
}

fn bad_argument(message: String) -> Error {
    coded(&codes::TEMPLATE_BAD_FILTER_ARGUMENT, message)
}

/// The standard five characters; `/` is left alone (#19).
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// `items|sort_by(["-date", "title"])`: by each key in turn, `-` for
/// descending; an item missing a key sorts last whichever the direction,
/// and ties break by Slug, as a Collection's `sort` does (#18). Jinja's
/// own `sort` puts missing values first.
fn sort_by(items: Value, keys: Value) -> Result<Value, Error> {
    let keys: Vec<String> = match keys.kind() {
        ValueKind::String => vec![keys.to_string()],
        ValueKind::Seq => keys.try_iter()?.map(|k| k.to_string()).collect(),
        _ => {
            return Err(bad_argument(format!(
                "`sort_by` takes a list of keys, such as [\"-date\"], not {}",
                keys.kind()
            )));
        }
    };
    let keys: Vec<(&str, bool)> = keys
        .iter()
        .map(|k| match k.strip_prefix('-') {
            Some(field) => (field, true),
            None => (k.as_str(), false),
        })
        .collect();
    if let Some((field, _)) = keys.iter().find(|(f, _)| f.is_empty()) {
        return Err(bad_argument(format!(
            "a `sort_by` key needs a field name, such as \"-date\", not \"{field}\""
        )));
    }
    let get = |v: &Value, key: &str| {
        v.get_attr(key)
            .ok()
            .filter(|x| !x.is_undefined() && !x.is_none())
    };
    let mut items: Vec<Value> = items.try_iter()?.collect();
    items.sort_by(|a, b| {
        for &(key, descending) in &keys {
            let order = match (get(a, key), get(b, key)) {
                (Some(x), Some(y)) if descending => y.cmp(&x),
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            };
            if order != Ordering::Equal {
                return order;
            }
        }
        get(a, "slug").cmp(&get(b, "slug"))
    });
    Ok(Value::from(items))
}

/// A date or datetime as Templates see it (see `context::value`), in
/// `site.timezone`. A date is a calendar day and doesn't move between
/// zones: it is that day at 00:00Z (#18).
fn dateformat(site: &Site, v: &Value, kw: &Kwargs) -> Result<String, Error> {
    let format: String = kw.get("format")?;
    kw.assert_all_used()?;
    if v.is_undefined() {
        let message = "`dateformat` got an absent value; guard it with `{% if %}`";
        return Err(Error::new(ErrorKind::UndefinedError, message));
    }
    let s = v.as_str().unwrap_or_default();
    let zoned: Zoned = if let Ok(d) = s.parse::<Date>()
        && s.len() == 10
    {
        d.to_zoned(TimeZone::UTC)
            .map_err(|e| bad_argument(e.to_string()))?
    } else if let Ok(t) = s.parse::<Timestamp>() {
        t.to_zoned(site.timezone.clone())
    } else {
        return Err(bad_argument(format!(
            "`dateformat` takes a date or datetime, not `{v}`"
        )));
    };
    match format.as_str() {
        "rfc2822" => jiff::fmt::rfc2822::DateTimePrinter::new()
            .timestamp_to_rfc9110_string(&zoned.timestamp())
            .map_err(|e| bad_argument(e.to_string())),
        "rfc3339" => Ok(zoned.strftime("%Y-%m-%dT%H:%M:%S%:z").to_string()),
        f if !known_format(f) => Err(bad_argument(unknown_format(f))),
        f => jiff::fmt::strtime::format(f, &zoned).map_err(|_| bad_argument(unknown_format(f))),
    }
}

/// Whether `dateformat` knows a format, which `check` asks of one written
/// in a Template.
pub(super) fn known_format(format: &str) -> bool {
    match format {
        "rfc2822" | "rfc3339" => true,
        // Text with no directive prints as itself, which hides a mistake
        // such as "iso".
        f if !f.contains('%') => false,
        f => jiff::fmt::strtime::format(f, &Timestamp::UNIX_EPOCH.to_zoned(TimeZone::UTC)).is_ok(),
    }
}

pub(super) fn unknown_format(f: &str) -> String {
    format!(
        "`dateformat` doesn't know the format \"{f}\": use strftime directives such as \"%Y-%m-%d\", or \"rfc2822\" or \"rfc3339\""
    )
}

/// A path of the Site, checked against its Routes, feeds and Static files.
fn url(site: &Site, path: &str) -> Result<String, Error> {
    let bare = path.split(['#', '?']).next().unwrap_or_default();
    if site.urls.contains(bare) {
        return Ok(path.to_string());
    }
    let mut message = format!("`{bare}` is no Route, feed or Static file of the Site");
    if let Some(near) = config::similar(bare, site.urls.iter().map(String::as_str)) {
        message.push_str(&format!("; did you mean `{near}`?"));
    }
    Err(coded(&codes::TEMPLATE_UNKNOWN_URL, message))
}

/// A path from the Site root as a full URL; a full URL is left as it is.
pub(super) fn absolute_url(origin: &str, path: &str) -> String {
    if path.starts_with("https://") || path.starts_with("http://") {
        return path.to_string();
    }
    match path.strip_prefix('/') {
        Some(rest) => format!("{origin}/{rest}"),
        None => format!("{origin}/{path}"),
    }
}

/// `n|pluralize("post", "posts")`, or `n|pluralize` for `""` and `"s"`.
fn pluralize(n: i64, singular: Option<String>, plural: Option<String>) -> String {
    if n == 1 {
        singular.unwrap_or_default()
    } else {
        plural.unwrap_or_else(|| "s".into())
    }
}

/// A map as HTML attributes, each with a space before it: an absent,
/// `none` or `false` value is left out, and `true` is a bare attribute.
fn attrs(v: Value) -> Result<Value, Error> {
    if v.is_undefined() || v.is_none() {
        return Ok(Value::from_safe_string(String::new()));
    }
    if v.kind() != ValueKind::Map {
        return Err(bad_argument(format!(
            "`attrs` takes a map of attributes, such as {{\"data-reveal\": true}}, not {}",
            v.kind()
        )));
    }
    let mut out = String::new();
    for key in v.try_iter()? {
        let value = v.get_item(&key)?;
        if value.is_undefined()
            || value.is_none()
            || (value.kind() == ValueKind::Bool && !value.is_true())
        {
            continue;
        }
        let name = key.to_string();
        if name.is_empty() || name.contains(|c: char| c.is_whitespace() || "\"'<>/=".contains(c)) {
            return Err(bad_argument(format!("`{name}` is not an attribute name")));
        }
        out.push(' ');
        out.push_str(&name);
        if value.kind() != ValueKind::Bool {
            out.push_str("=\"");
            out.push_str(&escape(&value.to_string()));
            out.push('"');
        }
    }
    Ok(Value::from_safe_string(out))
}

/// Markdown from a Data file or Site config, written as an Entry's body
/// is. `inline=true` leaves out the paragraph around a single one.
fn markdown(text: &str, kw: Kwargs) -> Result<Value, Error> {
    let mut files = Files::default();
    let file = files.add("<markdown>", text);
    let (doc, diagnostics) = markdown::document(&files, file, 0..text.len());
    fragment("markdown", "Markdown", doc, &diagnostics, kw)
}

fn org(text: &str, kw: Kwargs) -> Result<Value, Error> {
    let mut files = Files::default();
    let file = files.add("<org>", text);
    let (doc, diagnostics) = org::document(&files, file);
    fragment("org", "Org", doc, &diagnostics, kw)
}

/// The HTML of `doc`, the text a `markdown` or `org` filter read: its first
/// error fails the filter. With `inline=true`, a lone paragraph loses its
/// `<p>`.
fn fragment(
    filter: &str,
    format: &str,
    doc: Option<Document>,
    diagnostics: &[Diagnostic],
    kw: Kwargs,
) -> Result<Value, Error> {
    let inline: bool = kw.get::<Option<bool>>("inline")?.unwrap_or(false);
    kw.assert_all_used()?;
    let error = diagnostics.iter().find(|d| d.level == Level::Error);
    let doc = match (doc, error) {
        (Some(doc), None) => doc,
        (_, error) => {
            let message =
                error.map_or(format!("the {format} can't be read"), |d| d.message.clone());
            return Err(coded(
                &codes::TEMPLATE_RENDER,
                format!("`{filter}`: {message}"),
            ));
        }
    };
    let html = crate::html::body(
        &doc,
        &Links::new(),
        &highlight::document(&doc),
        &Default::default(),
    );
    let root = &doc.node(Document::ROOT).children;
    let single = root.len() == 1 && doc.node(root[0]).kind == Kind::Paragraph;
    let html = match html
        .strip_prefix("<p>")
        .and_then(|h| h.strip_suffix("</p>"))
    {
        Some(inner) if inline && single => inner.to_string(),
        _ => html,
    };
    Ok(Value::from_safe_string(html))
}

/// The kinds of image `image()` writes, as `format=` names them.
const FORMATS: [&str; 4] = ["avif", "webp", "jpeg", "png"];

/// `image(src, width=, widths=, format=, formats=, quality=, sizes=)`: an
/// Image, whose outputs are named now and encoded after render (#28).
fn image(images: &dyn Images, state: &State, src: &str, kw: Kwargs) -> Result<Value, Error> {
    let width: Option<u32> = kw.get("width")?;
    let widths: Option<Vec<u32>> = kw.get("widths")?;
    let format: Option<String> = kw.get("format")?;
    let formats: Option<Vec<String>> = kw.get("formats")?;
    let quality: Option<i64> = kw.get("quality")?;
    let sizes: Option<String> = kw.get("sizes")?;
    kw.assert_all_used()?;
    let widths = match (width, widths) {
        (Some(_), Some(_)) => {
            return Err(bad_argument(
                "`image` takes `width` for one width or `widths` for several, not both".into(),
            ));
        }
        (Some(w), None) => Some(vec![w]),
        (None, widths) => widths,
    };
    if let Some(widths) = &widths {
        if widths.is_empty() {
            return Err(bad_argument("`widths` needs at least one width".into()));
        }
        // WebP's limit, as `[images] widths` has.
        if let Some(w) = widths.iter().find(|w| !(1..=16383).contains(*w)) {
            return Err(bad_argument(format!(
                "`{w}` is no width: a width is from 1 to 16383 pixels"
            )));
        }
    }
    if format.is_some() && formats.is_some() {
        return Err(bad_argument(
            "`image` takes `format` for one format or `formats` for the `<source>`s, not both"
                .into(),
        ));
    }
    if let Some(f) = format.as_deref().filter(|f| !FORMATS.contains(f)) {
        return Err(bad_argument(format!(
            "`format` is one of \"avif\", \"webp\", \"jpeg\" and \"png\", not \"{f}\""
        )));
    }
    if let Some(f) = formats
        .iter()
        .flatten()
        .find(|f| !FORMATS[..2].contains(&f.as_str()))
    {
        return Err(bad_argument(format!(
            "`formats` lists the `<source>`s before the fallback, \"avif\" and \"webp\", not \"{f}\"; for one format, use `format=`"
        )));
    }
    let quality = match quality {
        Some(q) => Some(
            u8::try_from(q)
                .ok()
                .filter(|q| (1..=100).contains(q))
                .ok_or_else(|| bad_argument(format!("`quality` is from 1 to 100, not {q}")))?,
        ),
        None => None,
    };
    let request = Request {
        widths,
        formats,
        format,
        quality,
        sizes,
    };
    let image = images
        .image(src, &request, state.name())
        .map_err(|(code, message)| coded(code, format!("`image`: {message}")))?;
    let mut m = BTreeMap::from([
        ("url", Value::from(image.url)),
        ("srcset", Value::from(image.srcset)),
        ("sizes", Value::from(image.sizes)),
    ]);
    if let Some((width, height)) = image.size {
        m.insert("width", Value::from(width));
        m.insert("height", Value::from(height));
    }
    let sources: Vec<Value> = image
        .sources
        .into_iter()
        .map(|(ty, srcset)| {
            Value::from(BTreeMap::from([
                ("type", Value::from(ty)),
                ("srcset", Value::from(srcset)),
            ]))
        })
        .collect();
    m.insert("sources", Value::from(sources));
    Ok(Value::from(m))
}

/// The preloads that `picture(priority=true)` asks for, which go in the
/// Page's head once it is rendered.
#[derive(Default)]
pub(super) struct Preloads(pub Vec<String>);

/// `image | picture(alt=, priority=, class=, loading=, attrs=)`: the
/// `<picture>` an Image becomes.
fn picture(state: &mut State, image: Value, kw: Kwargs) -> Result<Value, Error> {
    let alt: Option<String> = kw.get("alt")?;
    let priority: Option<bool> = kw.get("priority")?;
    let class: Option<String> = kw.get("class")?;
    let loading: Option<String> = kw.get("loading")?;
    let more: Option<Value> = kw.get("attrs")?;
    kw.assert_all_used()?;
    let Some(alt) = alt else {
        return Err(bad_argument(
            "`picture` needs `alt`, which says what the image shows; `alt=\"\"` marks one that is only decoration".into(),
        ));
    };
    let loading = loading.unwrap_or_else(|| "lazy".into());
    if !["lazy", "eager"].contains(&loading.as_str()) {
        return Err(bad_argument(format!(
            "`loading` is \"lazy\" or \"eager\", not \"{loading}\""
        )));
    }
    let image = image_of(&image)?;
    let mut attrs = String::new();
    if let Some(class) = class {
        attrs.push_str(&format!(" class=\"{}\"", escape(&class)));
    }
    if let Some(more) = more {
        attrs.push_str(&self::attrs(more)?.to_string());
    }
    let priority = priority.unwrap_or(false);
    if priority {
        let preload = image::preload(&image);
        let preloads = &mut state.get_or_insert_extension(Preloads::default()).0;
        if !preloads.contains(&preload) {
            preloads.push(preload);
        }
    }
    let p = Picture {
        alt: &alt,
        priority,
        loading: &loading,
        attrs,
    };
    Ok(Value::from_safe_string(image::picture(&image, &p)))
}

/// An Image `image()` returned.
fn image_of(v: &Value) -> Result<Image, Error> {
    let not = || {
        bad_argument(format!(
            "`picture` takes an Image from `image()`, such as `image(entry.cover) | picture(alt=\"…\")`, not {}",
            v.kind()
        ))
    };
    if v.kind() != ValueKind::Map {
        return Err(not());
    }
    let text = |key: &str| -> Result<String, Error> {
        let field = v.get_attr(key)?;
        field.as_str().map(str::to_string).ok_or_else(not)
    };
    let number = |key: &str| v.get_attr(key).ok().and_then(|n| u32::try_from(n).ok());
    let mut sources = Vec::new();
    for s in v.get_attr("sources")?.try_iter().map_err(|_| not())? {
        let field = |key: &str| {
            s.get_attr(key)
                .ok()
                .and_then(|f| f.as_str().map(str::to_string))
        };
        match (field("type"), field("srcset")) {
            (Some(ty), Some(srcset)) => sources.push((ty, srcset)),
            _ => return Err(not()),
        }
    }
    Ok(Image {
        size: number("width").zip(number("height")),
        url: text("url")?,
        srcset: text("srcset")?,
        sizes: text("sizes")?,
        sources,
    })
}
