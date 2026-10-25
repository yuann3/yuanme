//! An Org file's keywords as its Entry's fields (#16). A keyword's name is
//! a field's name, and `#+FILETAGS` fills `tags`. Field keywords go in the
//! preamble; every other keyword is Org's own, accepted, warned about or
//! rejected by what it does.

use std::collections::BTreeMap;
use std::ops::Range;

use jiff::civil::{Date, Weekday};
use yuanme_diag::{Diagnostic, codes};

use super::Keyword;
use crate::config::{Collection, FieldType, Taxonomy, similar};
use crate::entry::{self, Head};
use crate::fields::Cx;
use crate::yaml::{self, Key, Node, Yaml};

/// Keywords for Emacs, which change nothing on the Site or only how the
/// file parses.
const EDITOR: &[&str] = &[
    "ARCHIVE",
    "CATEGORY",
    "COLUMNS",
    "CONSTANTS",
    "EXCLUDE_TAGS",
    "FILETAGS",
    "LINK",
    "PRIORITIES",
    "PROPERTY",
    "SELECT_TAGS",
    "SEQ_TODO",
    "STARTUP",
    "TBLFM",
    "TODO",
    "TYP_TODO",
];

/// Keywords for Org's exporters, and what does their job in yuanme.
const EXPORTER: &[(&str, &str)] = &[
    (
        "AUTHOR",
        "the Site's author is set in `[site]` in yuanme.toml",
    ),
    ("BIND", "yuanme sets no Emacs variables"),
    ("CREATOR", "yuanme writes no creator line"),
    (
        "DESCRIPTION",
        "declare a `description` field for the Collection to give Entries one",
    ),
    (
        "EMAIL",
        "the Site's author is set in `[site]` in yuanme.toml",
    ),
    (
        "EXPORT_FILE_NAME",
        "an Entry's file name and Slug decide where it is written",
    ),
    (
        "KEYWORDS",
        "declare a `keywords` field for the Collection to give Entries one",
    ),
    (
        "LANGUAGE",
        "the Site's language is set in `[site]` in yuanme.toml",
    ),
    (
        "LATEX_CLASS",
        "math macros are set in `[math]` in yuanme.toml",
    ),
    (
        "LATEX_CLASS_OPTIONS",
        "math macros are set in `[math]` in yuanme.toml",
    ),
    (
        "LATEX_COMPILER",
        "yuanme renders math itself, without LaTeX",
    ),
    (
        "LATEX_HEADER_EXTRA",
        "math macros are set in `[math]` in yuanme.toml",
    ),
    (
        "SUBTITLE",
        "declare a `subtitle` field for the Collection to give Entries one",
    ),
    (
        "TOC",
        "a table of contents is a Template's, from `entry.headings`",
    ),
];

/// Keywords that belong to the element after them, written where none is.
const AFFILIATED: &[&str] = &[
    "ATTR_HTML",
    "CAPTION",
    "DATA",
    "HEADER",
    "HEADERS",
    "LABEL",
    "NAME",
    "PLOT",
    "RESNAME",
    "RESULTS",
    "SOURCE",
    "SRCNAME",
    "TBLNAME",
];

/// ox-hugo's keywords and the yuanme keywords that do their job.
const HUGO: &[(&str, &str)] = &[
    ("HUGO_DRAFT", "DRAFT"),
    ("HUGO_LASTMOD", "UPDATED"),
    ("HUGO_PUBLISHDATE", "DATE"),
    ("HUGO_SLUG", "SLUG"),
    ("HUGO_TAGS", "FILETAGS"),
];

/// Each `#+OPTIONS` option, the values that ask for what yuanme does
/// anyway, and what yuanme does. `^` is honoured, and `H` is numeric.
const OPTIONS: &[(&str, &[&str], &str)] = &[
    ("'", &["t"], "yuanme always makes quotes curly"),
    ("*", &["t"], "yuanme always shows emphasis"),
    (
        "-",
        &["t"],
        "yuanme always makes `--`, `---`, `...` and `\\-` punctuation",
    ),
    ("::", &["t"], "yuanme always shows fixed-width lines"),
    ("<", &["t"], "yuanme always shows timestamps"),
    (
        "\\n",
        &["nil"],
        "yuanme joins a paragraph's lines, as Org does by default",
    ),
    ("^", &["t", "nil", "{}"], ""),
    (
        "arch",
        &["headline"],
        "yuanme shows an archived headline without what is under it",
    ),
    ("author", &["nil"], "Templates show the author"),
    (
        "broken-links",
        &["nil"],
        "a link that leads nowhere fails the build",
    ),
    ("c", &["nil"], "yuanme never shows clocks"),
    ("creator", &["nil"], "yuanme writes no creator line"),
    (
        "d",
        &["(not \"LOGBOOK\")"],
        "yuanme shows every drawer's contents but a LOGBOOK's",
    ),
    ("date", &["nil"], "Templates show the date"),
    ("e", &["t"], "yuanme always shows entities"),
    ("email", &["nil"], "Templates show the author"),
    ("expand-links", &["t"], "yuanme always expands links"),
    ("f", &["t"], "yuanme always shows footnotes"),
    ("H", &[], "every headline to level 5 is a heading"),
    (
        "html-link-use-abs-url",
        &["nil"],
        "yuanme writes links as they are written",
    ),
    (
        "html-postamble",
        &["nil"],
        "Templates write what goes around the body",
    ),
    (
        "html-preamble",
        &["nil"],
        "Templates write what goes around the body",
    ),
    ("html-scripts", &["nil"], "Templates write a Page's scripts"),
    ("html-style", &["nil"], "Templates write a Page's styles"),
    ("html5-fancy", &["t"], "yuanme writes HTML5"),
    (
        "inline",
        &["t"],
        "yuanme shows inline tasks' errors, not the tasks",
    ),
    ("num", &["nil"], "yuanme never numbers headings"),
    ("p", &["nil"], "yuanme never shows planning lines"),
    ("pri", &["nil"], "yuanme never shows priorities"),
    ("prop", &["nil"], "yuanme never shows properties"),
    ("stat", &["t"], "yuanme always shows statistics cookies"),
    ("tags", &["nil"], "yuanme never shows tags in headings"),
    (
        "tasks",
        &["t"],
        "yuanme shows every headline, whatever its TODO keyword",
    ),
    ("tex", &["t"], "yuanme always renders math"),
    ("timestamp", &["nil"], "yuanme writes no creation time"),
    ("title", &["t"], "Templates show the title"),
    (
        "toc",
        &["nil"],
        "a table of contents is a Template's, from `entry.headings`",
    ),
    ("todo", &["nil"], "yuanme never shows TODO keywords"),
    ("|", &["t"], "yuanme always shows tables"),
];

/// The fields `keywords` give an Entry of `collection`, with every mistake
/// in them reported. The preamble is where they are, for a field left out.
pub(crate) fn head(
    keywords: &[Keyword],
    preamble: Range<usize>,
    len: usize,
    collection: &Collection,
    taxonomies: &[(&str, &Taxonomy)],
    cx: &mut Cx<'_>,
) -> Head {
    let tags = taxonomies.iter().any(|(n, _)| *n == "tags");
    let types = field_types(collection, taxonomies);
    let mut fields: Vec<(Key, Node)> = Vec::new();
    let mut first: BTreeMap<String, Range<usize>> = BTreeMap::new();
    for k in keywords {
        let upper = k.key.to_ascii_uppercase();
        let name = if upper == "FILETAGS" && tags {
            "tags".to_string()
        } else {
            k.key.to_ascii_lowercase()
        };
        let Some(ty) = types
            .get(name.as_str())
            .filter(|_| name != "tags" || upper == "FILETAGS")
        else {
            other(k, &upper, &types, tags, cx);
            continue;
        };
        if !k.preamble {
            let message = format!(
                "`#+{}` sets a field, so it goes at the top of the file",
                k.key
            );
            cx.push(
                cx.error(&codes::ORG_MISPLACED_KEYWORD, message, k.key_at.clone())
                    .help("move it before the first headline and any text"),
            );
            continue;
        }
        if let Some(at) = first.get(&name) {
            let message = format!("`#+{}` is given twice", k.key);
            let d = cx
                .error(&codes::ENTRY_DUPLICATE_KEY, message, k.key_at.clone())
                .label("given again here")
                .secondary(cx.span(at.clone()), "first given here")
                .note("Org joins the two values; yuanme takes one (Org divergence 5)");
            cx.push(d);
            continue;
        }
        first.insert(name.clone(), k.key_at.clone());
        let key = Key {
            name: name.clone(),
            span: k.key_at.clone(),
        };
        let node = if name == "tags" {
            Some(filetags(k))
        } else {
            value(k, &name, ty, cx)
        };
        if let Some(node) = node {
            fields.push((key, node));
        }
    }
    Head {
        fields,
        at: preamble,
        body: 0..len,
    }
}

/// The type of every field a keyword can set. A Taxonomy holds strings.
fn field_types<'c>(
    collection: &'c Collection,
    taxonomies: &[(&'c str, &Taxonomy)],
) -> BTreeMap<&'c str, FieldType> {
    let string = FieldType::String {
        min: None,
        max: None,
    };
    let mut types: BTreeMap<&str, FieldType> = BTreeMap::new();
    for (name, ty) in [
        ("title", string.clone()),
        ("slug", string.clone()),
        ("date", FieldType::Date),
        ("updated", FieldType::Date),
        ("draft", FieldType::Bool),
        ("unlisted", FieldType::Bool),
    ] {
        types.insert(name, ty);
    }
    for (name, field) in &collection.fields {
        types.entry(name).or_insert_with(|| field.kind.clone());
    }
    for (name, _) in taxonomies {
        types.insert(
            name,
            FieldType::List {
                items: Box::new(string.clone()),
                unique: false,
            },
        );
    }
    types
}

/// `:a:b:` as a list of Terms, each with its span.
fn filetags(k: &Keyword) -> Node {
    let mut items = Vec::new();
    let mut at = k.value_at.start;
    for part in k.value.split(':') {
        let trimmed = part.trim();
        if !trimmed.is_empty() {
            let start = at + part.find(trimmed).unwrap_or(0);
            items.push(Node {
                value: Yaml::Str(trimmed.to_string()),
                span: start..start + trimmed.len(),
            });
        }
        at += part.len() + 1;
    }
    Node {
        value: Yaml::Seq(items),
        span: k.value_at.clone(),
    }
}

/// A keyword's value as front matter's would be: a string field's text as
/// it is written, anything else a YAML 1.2 flow value, and a date may be an
/// Org timestamp.
fn value(k: &Keyword, name: &str, ty: &FieldType, cx: &mut Cx<'_>) -> Option<Node> {
    let text = k.value.trim();
    let start = k.value_at.start + k.value.find(text).unwrap_or(0);
    let at = start..start + text.len();
    let node = |value| Node {
        value,
        span: at.clone(),
    };
    if text.is_empty() {
        return Some(node(Yaml::Null));
    }
    match ty {
        FieldType::String { .. } => return Some(node(Yaml::Str(text.to_string()))),
        FieldType::Bool if text == "t" || text == "nil" => {
            let fixed = if text == "t" { "true" } else { "false" };
            let message = format!("`{name}` must be `true` or `false`, not `{text}`");
            let d = cx.error(&codes::ENTRY_WRONG_TYPE, message, at.clone());
            let d = cx.fix(d, format!("write `{fixed}`"), at, fixed.to_string());
            cx.push(d.note("field values are YAML, not Emacs Lisp"));
            return None;
        }
        FieldType::Date | FieldType::Datetime if text.starts_with(['<', '[']) => {
            return timestamp(
                text,
                at.clone(),
                name,
                matches!(ty, FieldType::Datetime),
                cx,
            )
            .map(node);
        }
        _ => {}
    }
    match yaml::parse(text, start) {
        Ok(n) => Some(n),
        Err(e) => {
            cx.push(entry::yaml_error(e, cx));
            None
        }
    }
}

/// An Org timestamp, `[2025-12-12 Fri]` or `<2025-12-12 Fri 10:00>`, as an
/// ISO date or datetime. Org ignores the day name, so a wrong one is only a
/// warning.
fn timestamp(
    text: &str,
    at: Range<usize>,
    name: &str,
    datetime: bool,
    cx: &mut Cx<'_>,
) -> Option<Yaml> {
    let inner = &text[1..text.len().saturating_sub(1)];
    let closes = matches!(
        (text.as_bytes()[0], text.as_bytes()[text.len() - 1]),
        (b'<', b'>') | (b'[', b']')
    );
    let mut words = inner.split_whitespace();
    let date = words
        .next()
        .filter(|d| d.len() == 10)
        .and_then(|d| d.parse::<Date>().ok())
        .filter(|_| closes);
    let Some(date) = date else {
        let message = format!("`{name}` is not an Org timestamp such as [2025-12-12 Fri]");
        cx.push(cx.error(&codes::ENTRY_INVALID_DATE, message, at));
        return None;
    };
    let mut day = None;
    let mut time = None;
    for w in words {
        if w.chars().all(char::is_alphabetic) && day.is_none() && time.is_none() {
            day = Some(w);
        } else if let Some((h, m)) = w.split_once(':')
            && h.len() <= 2
            && m.len() >= 2
            && time.is_none()
        {
            time = Some((h, &m[..2]));
        }
    }
    if let Some(day) = day
        && let Some(written) = weekday(day)
        && written != date.weekday()
    {
        let full = format!("{:?}", date.weekday());
        let right = if day.len() <= 3 { &full[..3] } else { &full };
        let message = format!("{date} is a {:?}, not a {written:?}", date.weekday());
        let start = at.start + text.find(day).unwrap_or(0);
        let day_at = start..start + day.len();
        let d = Diagnostic::warning(&codes::ORG_WRONG_WEEKDAY, message, cx.span(day_at.clone()));
        cx.push(cx.fix(d, format!("write `{right}`"), day_at, right.to_string()));
    }
    match (time, datetime) {
        (Some((h, m)), true) => Some(Yaml::Str(format!("{date}T{h:0>2}:{m}:00"))),
        (Some(_), false) => {
            let message = format!("`{name}` is a date, but this timestamp has a time");
            cx.push(
                cx.error(&codes::ENTRY_INVALID_DATE, message, at)
                    .note("a `date` has no time; declare the field as `datetime` to keep one"),
            );
            None
        }
        (None, _) => Some(Yaml::Str(date.to_string())),
    }
}

/// An English day name, as Org writes it in an English locale. Other
/// languages' names aren't checked.
fn weekday(name: &str) -> Option<Weekday> {
    let lower = name.to_lowercase();
    let days = [
        ("monday", Weekday::Monday),
        ("tuesday", Weekday::Tuesday),
        ("wednesday", Weekday::Wednesday),
        ("thursday", Weekday::Thursday),
        ("friday", Weekday::Friday),
        ("saturday", Weekday::Saturday),
        ("sunday", Weekday::Sunday),
    ];
    days.iter()
        .find(|(full, _)| lower.len() >= 3 && full.starts_with(&lower))
        .map(|(_, d)| *d)
}

/// A keyword that sets no field: accepted, warned about or rejected.
fn other(k: &Keyword, upper: &str, types: &BTreeMap<&str, FieldType>, tags: bool, cx: &mut Cx<'_>) {
    if upper == "TAGS" && tags {
        let message = "`#+TAGS` lists the tags Emacs offers, not the Entry's".to_string();
        let d = cx.error(&codes::ENTRY_UNKNOWN_KEY, message, k.key_at.clone());
        let d = cx.fix(
            d,
            "an Entry's tags are its `#+FILETAGS`, written `:a:b:`".into(),
            k.key_at.clone(),
            "FILETAGS".into(),
        );
        cx.push(d);
        return;
    }
    // The body's reader reads `#+LATEX_HEADER`'s macros (#17).
    if EDITOR.contains(&upper) || upper == "TAGS" || upper == "LATEX_HEADER" {
        return;
    }
    if upper == "OPTIONS" {
        return options(k, cx);
    }
    if let Some((_, instead)) = EXPORTER.iter().find(|(e, _)| *e == upper) {
        let message = format!("`#+{}` has no effect in yuanme", k.key);
        cx.push(
            Diagnostic::warning(&codes::ORG_NO_EFFECT, message, cx.span(k.key_at.clone()))
                .note(*instead),
        );
        return;
    }
    if upper.starts_with("HTML_") {
        let message = format!("`#+{}` has no effect in yuanme", k.key);
        let note = "a Page's `<head>` and the markup around its body come from Templates";
        cx.push(
            Diagnostic::warning(&codes::ORG_NO_EFFECT, message, cx.span(k.key_at.clone()))
                .note(note),
        );
        return;
    }
    if AFFILIATED.contains(&upper) {
        let message = format!("`#+{}` has no effect: no element follows it", k.key);
        let note = "it belongs to the element on the line after it";
        cx.push(
            Diagnostic::warning(&codes::ORG_NO_EFFECT, message, cx.span(k.key_at.clone()))
                .note(note),
        );
        return;
    }
    // Other back ends' attributes, which ox-html ignores too.
    if upper.starts_with("ATTR_") {
        return;
    }
    if upper.starts_with("HUGO_") {
        let message = format!("`#+{}` is ox-hugo's, which yuanme doesn't read", k.key);
        let mut d = cx.error(&codes::ENTRY_UNKNOWN_KEY, message, k.key_at.clone());
        d = match HUGO.iter().find(|(h, _)| *h == upper) {
            Some((_, instead)) => cx.fix(
                d,
                format!("yuanme's keyword is `#+{instead}`"),
                k.key_at.clone(),
                instead.to_string(),
            ),
            None => d.note("one Org file is one Entry, and its keywords are its fields"),
        };
        cx.push(d);
        return;
    }
    let message = format!("unknown keyword `#+{}`", k.key);
    let mut d = cx
        .error(&codes::ENTRY_UNKNOWN_KEY, message, k.key_at.clone())
        .label("unknown keyword");
    let fields: Vec<String> = types
        .keys()
        .filter(|n| **n != "tags" || !tags)
        .map(|n| n.to_ascii_uppercase())
        .chain(tags.then(|| "FILETAGS".to_string()))
        .collect();
    let known = fields
        .iter()
        .map(String::as_str)
        .chain(EDITOR.iter().copied())
        .chain(["OPTIONS"]);
    d = match similar(upper, known) {
        Some(near) => cx.fix(
            d,
            format!("a keyword with a similar name exists: `#+{near}`"),
            k.key_at.clone(),
            near.to_string(),
        ),
        None => {
            let mut names: Vec<String> = fields.iter().map(|f| format!("`#+{f}`")).collect();
            names.sort();
            d.note(format!(
                "the keywords that set fields are {}",
                names.join(", ")
            ))
        }
    };
    cx.push(d);
}

/// `#+OPTIONS`: `^:` is honoured, a value yuanme does anyway is accepted,
/// one asking for what it doesn't do is a warning, and an option Org
/// doesn't have is an error.
fn options(k: &Keyword, cx: &mut Cx<'_>) {
    for (token, at) in option_tokens(&k.value, k.value_at.start) {
        let (name, value) = match token.strip_prefix("::") {
            Some(rest) => ("::", rest),
            None => match token.split_once(':') {
                Some(pair) => pair,
                None => (token, ""),
            },
        };
        let Some((_, accepted, does)) = OPTIONS.iter().find(|(o, _, _)| *o == name) else {
            let message = format!("`{name}` is not an Org export option");
            let mut d = cx.error(&codes::ORG_UNKNOWN_OPTION, message, at.clone());
            if let Some(near) = similar(name, OPTIONS.iter().map(|(o, _, _)| *o)) {
                let fixed = format!("{near}:{value}");
                d = cx.fix(d, format!("did you mean `{near}`?"), at, fixed);
            }
            cx.push(d);
            continue;
        };
        let fine = match name {
            "H" => value.parse::<u32>().is_ok_and(|n| n >= 5),
            _ => accepted.contains(&value),
        };
        if !fine {
            let message = format!("`{token}` has no effect in yuanme");
            cx.push(Diagnostic::warning(&codes::ORG_NO_EFFECT, message, cx.span(at)).note(*does));
        }
    }
}

/// `#+OPTIONS`'s `name:value` items, where a value in parentheses or
/// quotes may hold spaces, each with its span.
fn option_tokens(value: &str, base: usize) -> Vec<(&str, Range<usize>)> {
    let bytes = value.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        let start = i;
        let mut depth = 0usize;
        let mut quoted = false;
        while i < bytes.len() && (depth > 0 || quoted || !bytes[i].is_ascii_whitespace()) {
            match bytes[i] {
                b'"' => quoted = !quoted,
                b'(' if !quoted => depth += 1,
                b')' if !quoted => depth = depth.saturating_sub(1),
                _ => {}
            }
            i += 1;
        }
        out.push((&value[start..i], base + start..base + i));
    }
    out
}
