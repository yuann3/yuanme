//! Field values, checked against the types Site config declares (#18).
//! Front matter and, from #49, Org keywords hand their values in as YAML
//! nodes, so both Source formats share one set of rules.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::ops::Range;

use jiff::Zoned;
use jiff::civil::{Date, DateTime};
use jiff::fmt::temporal::{Pieces, PiecesOffset};
use jiff::tz::{Offset, TimeZone};
use yuanme_diag::{Applicability, Code, Diagnostic, Edit, FileId, Span, codes};

use crate::config::{Field, FieldType, similar};
use crate::yaml::{Node, Yaml};

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Date(Date),
    Datetime(Zoned),
    Url(String),
    Image(Image),
    List(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Image {
    /// A project file, by its path from the project root.
    Local(String),
    Remote(String),
}

/// Orders two values of one field, for Collection `sort`.
pub fn compare(a: &Value, b: &Value) -> Ordering {
    use Value::*;
    match (a, b) {
        (Int(a), Int(b)) => a.cmp(b),
        (Int(_) | Float(_), Int(_) | Float(_)) => number(a).total_cmp(&number(b)),
        (String(a) | Url(a), String(b) | Url(b)) => a.cmp(b),
        (Bool(a), Bool(b)) => a.cmp(b),
        (Date(a), Date(b)) => a.cmp(b),
        (Datetime(a), Datetime(b)) => a.timestamp().cmp(&b.timestamp()),
        // A date sorts as its midnight UTC, as it is written out (#18).
        (Date(a), Datetime(b)) => midnight(*a).cmp(&b.timestamp()),
        (Datetime(a), Date(b)) => a.timestamp().cmp(&midnight(*b)),
        _ => Ordering::Equal,
    }
}

fn number(v: &Value) -> f64 {
    match v {
        Value::Int(i) => *i as f64,
        Value::Float(f) => *f,
        _ => 0.0,
    }
}

fn midnight(d: Date) -> jiff::Timestamp {
    d.to_zoned(TimeZone::UTC)
        .expect("a date is in range")
        .timestamp()
}

/// What checking a value needs besides the value.
pub(crate) struct Cx<'a> {
    pub file: FileId,
    pub diagnostics: &'a mut Vec<Diagnostic>,
    pub timezone: &'a TimeZone,
    /// The folder relative image paths start from, such as the Entry's.
    pub dir: &'a str,
    pub is_file: &'a dyn Fn(&str) -> bool,
}

impl Cx<'_> {
    pub fn span(&self, range: Range<usize>) -> Span {
        Span {
            file: self.file,
            range,
        }
    }

    pub fn error(&mut self, code: &'static Code, message: String, at: Range<usize>) -> Diagnostic {
        Diagnostic::error(code, message, self.span(at))
    }

    pub fn push(&mut self, d: Diagnostic) {
        self.diagnostics.push(d);
    }

    pub fn fix(
        &self,
        d: Diagnostic,
        message: String,
        at: Range<usize>,
        replacement: String,
    ) -> Diagnostic {
        let edit = Edit {
            span: self.span(at),
            replacement,
        };
        d.suggest(message, Applicability::MaybeIncorrect, vec![edit])
    }
}

/// Checks a whole set of fields: unknown keys, missing required fields, and
/// each value. Returns the fields given, with defaults filled in for the
/// ones left out. `extra` names keys handled by the caller, such as
/// built-in fields, which are neither checked nor unknown here.
pub(crate) fn fields(
    schema: &BTreeMap<String, Field>,
    given: &[(String, Range<usize>, &Node)],
    extra: &[&str],
    whole: Range<usize>,
    path: &str,
    cx: &mut Cx<'_>,
) -> Option<BTreeMap<String, Value>> {
    let mut out = BTreeMap::new();
    let mut ok = true;
    for (key, key_span, node) in given {
        if extra.contains(&key.as_str()) {
            continue;
        }
        let Some(field) = schema.get(key) else {
            let message = match path {
                "" => format!("unknown field `{key}`"),
                _ => format!("unknown field `{key}` in `{path}`"),
            };
            let mut d = cx
                .error(&codes::ENTRY_UNKNOWN_KEY, message, key_span.clone())
                .label("unknown field");
            let known = schema
                .keys()
                .map(String::as_str)
                .chain(extra.iter().copied());
            d = match similar(key, known.clone()) {
                Some(near) => cx.fix(
                    d,
                    format!("a field with a similar name exists: `{near}`"),
                    key_span.clone(),
                    near.into(),
                ),
                None => {
                    let mut known: Vec<_> = known.map(|k| format!("`{k}`")).collect();
                    known.sort();
                    d.note(format!("the fields are {}", known.join(", ")))
                }
            };
            cx.push(d);
            ok = false;
            continue;
        };
        if node.value == Yaml::Null {
            continue; // `key:` with nothing after it leaves the field out
        }
        match value(&field.kind, node, &join(path, key), cx) {
            Some(v) => {
                out.insert(key.clone(), v);
            }
            None => ok = false,
        }
    }
    // A field given with a bad value was reported already, not missing.
    let present = |name: &str| {
        given
            .iter()
            .any(|(k, _, n)| k == name && n.value != Yaml::Null)
    };
    for (name, field) in schema {
        if present(name) {
            continue;
        }
        match &field.default {
            Some(default) => {
                out.insert(
                    name.clone(),
                    from_default(&field.kind, default, cx.timezone),
                );
            }
            None if !field.optional => {
                let message = match path {
                    "" => format!("missing field `{name}`"),
                    _ => format!("`{path}` is missing the field `{name}`"),
                };
                let mut d = cx.error(&codes::ENTRY_MISSING_FIELD, message, whole.clone());
                if let Some(description) = &field.description {
                    d = d.note(format!("`{name}`: {description}"));
                }
                cx.push(d);
                ok = false;
            }
            None => {}
        }
    }
    ok.then_some(out)
}

pub(crate) fn join(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

/// Checks one value against its type.
pub(crate) fn value(ty: &FieldType, node: &Node, path: &str, cx: &mut Cx<'_>) -> Option<Value> {
    let at = node.span.clone();
    let wrong = |cx: &mut Cx<'_>, expected: &str| {
        let message = format!("`{path}` must be {expected}, not {}", node.kind());
        let mut d = cx
            .error(&codes::ENTRY_WRONG_TYPE, message, at.clone())
            .label(format!("expected {expected}"));
        // YAML read the Author's text as a number or boolean.
        if expected == "a string"
            && matches!(node.value, Yaml::Int(_) | Yaml::Float(_) | Yaml::Bool(_))
        {
            let text = format!("{:?}", scalar_text(node));
            d = cx.fix(d, "quote it to make it text".into(), at.clone(), text);
        }
        if matches!(ty, FieldType::Bool)
            && matches!(&node.value, Yaml::Str(s) if ["yes", "no", "on", "off", "y", "n"].contains(&s.to_lowercase().as_str()))
        {
            d = d.note("YAML 1.2 reads only `true` and `false` as booleans");
        }
        cx.push(d);
        None
    };
    let range = |cx: &mut Cx<'_>, what: String| {
        cx.push(cx_error(cx, &codes::ENTRY_OUT_OF_RANGE, what, at.clone()));
        None
    };
    match (ty, &node.value) {
        (FieldType::String { min, max }, Yaml::Str(s)) => {
            let n = s.chars().count() as u64;
            match (min, max) {
                (Some(min), _) if n < *min => range(
                    cx,
                    format!("`{path}` must be at least {min} characters, not {n}"),
                ),
                (_, Some(max)) if n > *max => range(
                    cx,
                    format!("`{path}` must be at most {max} characters, not {n}"),
                ),
                _ => Some(Value::String(s.clone())),
            }
        }
        (FieldType::Int { min, max }, Yaml::Int(i)) => match (min, max) {
            (Some(min), _) if i < min => {
                range(cx, format!("`{path}` must be at least {min}, not {i}"))
            }
            (_, Some(max)) if i > max => {
                range(cx, format!("`{path}` must be at most {max}, not {i}"))
            }
            _ => Some(Value::Int(*i)),
        },
        (FieldType::Float { min, max }, Yaml::Int(_) | Yaml::Float(_)) => {
            let f = match node.value {
                Yaml::Int(i) => i as f64,
                Yaml::Float(f) => f,
                _ => unreachable!(),
            };
            match (min, max) {
                (Some(min), _) if f < *min => {
                    range(cx, format!("`{path}` must be at least {min}, not {f}"))
                }
                (_, Some(max)) if f > *max => {
                    range(cx, format!("`{path}` must be at most {max}, not {f}"))
                }
                _ => Some(Value::Float(f)),
            }
        }
        (FieldType::Bool, Yaml::Bool(b)) => Some(Value::Bool(*b)),
        (FieldType::Date, Yaml::Str(s)) => date(s, path, at, cx).map(Value::Date),
        (FieldType::Datetime, Yaml::Str(s)) => datetime(s, path, at, cx).map(Value::Datetime),
        (FieldType::Url, Yaml::Str(s)) => {
            if is_url(s) {
                Some(Value::Url(s.clone()))
            } else {
                let message = format!("`{path}` must be an absolute URL starting with `https://`");
                cx.push(cx_error(cx, &codes::ENTRY_INVALID_VALUE, message, at));
                None
            }
        }
        (FieldType::Image, Yaml::Str(s)) => image(s, path, at, cx).map(Value::Image),
        (FieldType::Enum(values), Yaml::Str(s)) => {
            if values.contains(s) {
                return Some(Value::String(s.clone()));
            }
            let list: Vec<_> = values.iter().map(|v| format!("`{v}`")).collect();
            let message = format!("`{s}` is not one of {}", list.join(", "));
            let mut d = cx
                .error(&codes::ENTRY_INVALID_VALUE, message, at.clone())
                .label(format!("not a valid `{path}`"));
            if let Some(near) = similar(s, values.iter().map(String::as_str)) {
                d = cx.fix(d, format!("did you mean `{near}`?"), at, near.into());
            }
            cx.push(d);
            None
        }
        (FieldType::List { items, unique }, Yaml::Seq(nodes)) => {
            let mut out = Vec::new();
            let mut ok = true;
            for node in nodes {
                match value(items, node, &format!("{path}[]"), cx) {
                    Some(v) if *unique && out.contains(&v) => {
                        let message =
                            format!("`{path}` repeats an item, but its items must be unique");
                        cx.push(cx_error(
                            cx,
                            &codes::ENTRY_INVALID_VALUE,
                            message,
                            node.span.clone(),
                        ));
                        ok = false;
                    }
                    Some(v) => out.push(v),
                    None => ok = false,
                }
            }
            ok.then_some(Value::List(out))
        }
        (FieldType::Object(schema), Yaml::Map(entries)) => {
            let given: Vec<_> = entries
                .iter()
                .map(|(k, v)| (k.name.clone(), k.span.clone(), v))
                .collect();
            fields(schema, &given, &[], at, path, cx).map(Value::Object)
        }
        (FieldType::String { .. } | FieldType::Enum(_), _) => wrong(cx, "a string"),
        (FieldType::Int { .. }, Yaml::Float(_)) => wrong(cx, "a whole number"),
        (FieldType::Int { .. }, _) => wrong(cx, "an integer"),
        (FieldType::Float { .. }, _) => wrong(cx, "a number"),
        (FieldType::Bool, _) => wrong(cx, "`true` or `false`"),
        (FieldType::Date, _) => wrong(cx, "a date such as 2025-12-12"),
        (FieldType::Datetime, _) => wrong(cx, "a datetime such as 2025-12-12T09:30:00+08:00"),
        (FieldType::Url, _) => wrong(cx, "a URL string"),
        (FieldType::Image, _) => wrong(cx, "a path or URL string"),
        (FieldType::List { .. }, _) => wrong(cx, "a list"),
        (FieldType::Object(_), _) => wrong(cx, "a mapping"),
    }
}

fn cx_error(cx: &Cx<'_>, code: &'static Code, message: String, at: Range<usize>) -> Diagnostic {
    Diagnostic::error(code, message, cx.span(at))
}

fn scalar_text(node: &Node) -> String {
    match &node.value {
        Yaml::Int(i) => i.to_string(),
        Yaml::Float(f) => f.to_string(),
        Yaml::Bool(b) => b.to_string(),
        Yaml::Str(s) => s.clone(),
        _ => String::new(),
    }
}

pub(crate) fn is_url(s: &str) -> bool {
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"));
    rest.is_some_and(|r| !r.is_empty() && !r.starts_with('/')) && !s.contains(char::is_whitespace)
}

/// Other ways people write dates, read only to suggest the ISO form.
const DATE_FORMS: &[&str] = &[
    "%b %d %Y",
    "%B %d %Y",
    "%b %d, %Y",
    "%B %d, %Y",
    "%d %b %Y",
    "%d %B %Y",
    "%Y/%m/%d",
    "%Y.%m.%d",
];

fn date(s: &str, path: &str, at: Range<usize>, cx: &mut Cx<'_>) -> Option<Date> {
    if let Ok(d) = s.trim().parse::<Date>()
        && s.trim().len() == 10
    {
        return Some(d);
    }
    let mut d = cx.error(
        &codes::ENTRY_INVALID_DATE,
        format!("`{path}` is not a date in ISO form, such as 2025-12-12"),
        at.clone(),
    );
    if s.trim().parse::<DateTime>().is_ok()
        || Pieces::parse(s.trim()).is_ok_and(|p| p.time().is_some())
    {
        d = d.note("a `date` has no time; declare the field as `datetime` to keep one");
    } else if let Some(iso) = DATE_FORMS
        .iter()
        .find_map(|f| Date::strptime(f, s.trim()).ok())
    {
        d = cx.fix(d, "write it in ISO form".into(), at, iso.to_string());
    }
    cx.push(d);
    None
}

fn datetime(s: &str, path: &str, at: Range<usize>, cx: &mut Cx<'_>) -> Option<Zoned> {
    let s = s.trim();
    let parsed = Pieces::parse(s).ok().and_then(|p| {
        let dt = p.date().to_datetime(p.time()?);
        match p.offset() {
            Some(PiecesOffset::Numeric(o)) => dt.to_zoned(TimeZone::fixed(o.offset())).ok(),
            Some(PiecesOffset::Zulu) => dt.to_zoned(TimeZone::fixed(Offset::UTC)).ok(),
            // No offset: the Site's zone, never the machine's (#18).
            None => dt.to_zoned(cx.timezone.clone()).ok(),
            Some(_) => None,
        }
    });
    if parsed.is_none() {
        let message = format!("`{path}` is not a datetime such as 2025-12-12T09:30:00+08:00");
        let mut d = cx.error(&codes::ENTRY_INVALID_DATE, message, at);
        if s.parse::<Date>().is_ok() {
            d = d.note("a `datetime` needs a time; declare the field as `date` if it has none");
        }
        cx.push(d);
    }
    parsed
}

fn image(s: &str, path: &str, at: Range<usize>, cx: &mut Cx<'_>) -> Option<Image> {
    if is_url(s) {
        return Some(Image::Remote(s.to_string()));
    }
    let local = match s.strip_prefix('/') {
        Some(from_root) => from_root.to_string(),
        None => normalize(&join_dir(cx.dir, s.strip_prefix("./").unwrap_or(s))),
    };
    if (cx.is_file)(&local) {
        return Some(Image::Local(local));
    }
    let message = format!("`{path}`: no image at `{local}`");
    let d = cx
        .error(&codes::ENTRY_INVALID_VALUE, message, at)
        .help("a relative path starts from the Entry's folder, and one starting with `/` from the project root");
    cx.push(d);
    None
}

fn join_dir(dir: &str, path: &str) -> String {
    if dir.is_empty() {
        path.to_string()
    } else {
        format!("{dir}/{path}")
    }
}

/// Resolves `..` and `.` segments.
fn normalize(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            _ => out.push(seg),
        }
    }
    out.join("/")
}

/// A default from Site config, which config checked against its type.
pub(crate) fn from_default(ty: &FieldType, v: &toml::Value, tz: &TimeZone) -> Value {
    match (ty, v) {
        (FieldType::Float { .. }, toml::Value::Integer(i)) => Value::Float(*i as f64),
        (FieldType::Float { .. }, toml::Value::Float(f)) => Value::Float(*f),
        (FieldType::Int { .. }, toml::Value::Integer(i)) => Value::Int(*i),
        (FieldType::Bool, toml::Value::Boolean(b)) => Value::Bool(*b),
        (FieldType::Url, toml::Value::String(s)) => Value::Url(s.clone()),
        (FieldType::Image, toml::Value::String(s)) if is_url(s) => {
            Value::Image(Image::Remote(s.clone()))
        }
        (FieldType::Image, toml::Value::String(s)) => {
            Value::Image(Image::Local(s.trim_start_matches('/').into()))
        }
        (FieldType::Date, toml::Value::Datetime(d)) => {
            let d = d.date.expect("checked: a date");
            Value::Date(
                Date::new(d.year as i16, d.month as i8, d.day as i8).expect("a TOML date is valid"),
            )
        }
        (FieldType::Datetime, toml::Value::Datetime(d)) => {
            let text = d.to_string();
            let p = Pieces::parse(&text).expect("a TOML datetime is RFC 3339");
            let dt = p.date().to_datetime(p.time().expect("checked: has a time"));
            let tz = match p.offset() {
                Some(PiecesOffset::Numeric(o)) => TimeZone::fixed(o.offset()),
                Some(_) => TimeZone::UTC,
                None => tz.clone(),
            };
            Value::Datetime(dt.to_zoned(tz).expect("in range"))
        }
        (FieldType::List { items, .. }, toml::Value::Array(a)) => {
            Value::List(a.iter().map(|v| from_default(items, v, tz)).collect())
        }
        (FieldType::Object(fields), toml::Value::Table(t)) => Value::Object(
            fields
                .iter()
                .filter_map(|(k, f)| match (t.get(k), &f.default) {
                    (Some(v), _) | (None, Some(v)) => {
                        Some((k.clone(), from_default(&f.kind, v, tz)))
                    }
                    (None, None) => None,
                })
                .collect(),
        ),
        (_, toml::Value::String(s)) => Value::String(s.clone()),
        _ => unreachable!("config checked the default against its type"),
    }
}
