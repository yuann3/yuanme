//! Site config: `yuanme.toml`, read into typed values with a Diagnostic for
//! every mistake in it (#18, #21).

mod check;
mod schema;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use toml::Spanned;
use toml::de::{DeTable, DeValue};
use yuanme_diag::{Diagnostic, FileId, Files, codes};

pub(crate) use check::similar;
use check::{Checker, kind, parse_version};
pub use schema::json_schema;
#[cfg(test)]
use schema::{Key, Shape};

#[derive(Debug)]
pub struct Config {
    /// `yuanme.toml`, which the spans below are in.
    pub file: FileId,
    pub site: Site,
    pub collections: BTreeMap<String, Collection>,
    pub taxonomies: BTreeMap<String, Taxonomy>,
    /// Schemas for Data files, by name.
    pub data: BTreeMap<String, DataSchema>,
}

#[derive(Debug)]
pub struct DataSchema {
    pub kind: FieldType,
    /// Where `[data.<name>]` is named.
    pub at: Range<usize>,
}

#[derive(Debug)]
pub struct Site {
    pub url: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub language: Option<String>,
    pub author: Option<String>,
    pub timezone: String,
    pub extra: toml::Table,
}

#[derive(Clone, Debug)]
pub struct Collection {
    pub dir: String,
    /// Where the folder is set, or the Collection named when it isn't.
    pub dir_at: Range<usize>,
    pub route: String,
    pub list_route: Option<String>,
    pub paginate: Option<u64>,
    pub sort: Vec<SortKey>,
    pub fields: BTreeMap<String, Field>,
}

#[derive(Debug)]
pub struct Taxonomy {
    pub collections: Vec<String>,
    pub route: String,
    pub paginate: Option<u64>,
    pub single: bool,
    pub sort: Vec<SortKey>,
    pub lowercase: bool,
    /// Slugs the Author chose, by Term.
    pub slugs: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortKey {
    pub field: String,
    pub descending: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub kind: FieldType,
    /// Entries may leave it out. A field with a default is optional.
    pub optional: bool,
    pub default: Option<toml::Value>,
    pub description: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FieldType {
    String { min: Option<u64>, max: Option<u64> },
    Int { min: Option<i64>, max: Option<i64> },
    Float { min: Option<f64>, max: Option<f64> },
    Bool,
    Date,
    Datetime,
    Url,
    Enum(Vec<String>),
    List { items: Box<FieldType>, unique: bool },
    Object(BTreeMap<String, Field>),
    Image,
}

/// Fields every Entry has. Only `date` can be declared, to make it required.
const BUILT_IN: &[&str] = &["title", "date", "updated", "slug", "draft", "unlisted"];
/// Fields yuanme derives from each Entry, so no Author field can take their names.
const DERIVED: &[&str] = &["modified", "word_count", "reading_time", "headings"];

/// Reads Site config. It returns `None` if the config has an error; the
/// Diagnostics may hold warnings either way. `engine` is the running
/// Engine's version, which `yuanme = "X.Y"` is checked against.
pub fn parse(files: &Files, file: FileId, engine: &str) -> (Option<Config>, Vec<Diagnostic>) {
    let mut checker = Checker::new(file);
    let Some(mut root) = parse_toml(files.text(file), &mut checker) else {
        return (None, checker.diagnostics);
    };
    // A project written for another Engine reports only that: its other
    // mistakes may be keys the right Engine knows.
    if !engine_version(&root, engine, &mut checker) {
        return (None, checker.diagnostics);
    }
    let keys: Vec<_> = schema::ROOT.iter().collect();
    checker.table(&keys, &mut root, 0..0, "");
    let config = if checker.failed() {
        None
    } else {
        Some(read(&root, &mut checker))
    };
    let mut diagnostics = checker.diagnostics;
    diagnostics.sort_by_key(|d| d.primary().range.start);
    let config = config.filter(|_| {
        !diagnostics
            .iter()
            .any(|d| d.level == yuanme_diag::Level::Error)
    });
    (config, diagnostics)
}

fn parse_toml<'i>(text: &'i str, checker: &mut Checker) -> Option<DeTable<'i>> {
    let (root, errors) = DeTable::parse_recoverable(text);
    for e in &errors {
        let at = e.span().unwrap_or(0..0);
        checker.error(
            &codes::CONFIG_SYNTAX,
            format!("invalid TOML: {}", e.message()),
            at,
        );
    }
    errors.is_empty().then(|| root.into_inner())
}

/// `yuanme = "X.Y"` with Cargo's caret meaning (#37).
fn engine_version(root: &DeTable<'_>, engine: &str, checker: &mut Checker) -> bool {
    let Some(value) = root.get("yuanme") else {
        return true;
    };
    let Some((major, minor)) = value.get_ref().as_str().and_then(parse_version) else {
        return true; // the first pass reports it
    };
    let version: Vec<u64> = engine
        .split(['+', '-'])
        .next()
        .and_then(|v| v.split('.').map(|n| n.parse().ok()).collect())
        .filter(|v: &Vec<u64>| v.len() == 3)
        .expect("the Engine's version is X.Y.Z");
    let (e_major, e_minor) = (version[0], version[1]);
    let ok = match major {
        0 => e_major == 0 && e_minor == minor,
        _ => e_major == major && e_minor >= minor,
    };
    if ok {
        return true;
    }
    let range = if major == 0 {
        format!("{major}.{minor}.x")
    } else {
        format!("{major}.{minor} or a later {major}.x")
    };
    let older = (e_major, e_minor) < (major, minor);
    let help = if older {
        "update yuanme: `yuanme self update`".to_string()
    } else {
        format!(
            "install a {range} release, or set `yuanme = \"{e_major}.{e_minor}\"` once the project builds with this one"
        )
    };
    let message = format!("this project needs yuanme {range}, but this is yuanme {engine}");
    checker
        .error(&codes::CONFIG_ENGINE_VERSION, message, value.span())
        .then(|d| d.label("the project's version").help(help));
    false
}

type Table<'i> = DeTable<'i>;
type Value<'i> = Spanned<DeValue<'i>>;

/// The second pass: the shape is known, so this reads values and checks
/// what they refer to.
fn read(root: &Table<'_>, checker: &mut Checker) -> Config {
    let site = table(root, "site").expect("required");
    let collections = entries(table(root, "collections"));
    let taxonomies = entries(table(root, "taxonomies"));

    let mut names: BTreeSet<&str> = collections
        .iter()
        .map(|(name, _)| name.get_ref().as_ref())
        .collect();
    names.insert("pages");

    let collections: BTreeMap<String, Collection> = collections
        .iter()
        .map(|(key, c)| {
            let name: &str = key.get_ref().as_ref();
            // Each Taxonomy over this Collection adds a field of its name.
            let taxonomy_fields: Vec<&str> = taxonomies
                .iter()
                .filter(|(_, t)| {
                    items(&as_table(t)["collections"])
                        .iter()
                        .any(|c| c.get_ref().as_str() == Some(name))
                })
                .map(|(t, _)| t.get_ref().as_ref())
                .collect();
            (
                name.to_string(),
                collection(key, as_table(c), &taxonomy_fields, checker),
            )
        })
        .collect();
    let taxonomies = taxonomies
        .iter()
        .map(|(name, t)| {
            (
                name.get_ref().to_string(),
                taxonomy(name, as_table(t), &names, &collections, checker),
            )
        })
        .collect();
    let data = entries(table(root, "data"))
        .iter()
        .map(|(name, spec)| {
            let kind = field(as_table(spec), false, checker).kind;
            (
                name.get_ref().to_string(),
                DataSchema {
                    kind,
                    at: name.span(),
                },
            )
        })
        .collect();
    if let Some(tz) = site.get("timezone") {
        let name = tz.get_ref().as_str().expect("checked");
        // The bundled database, so a build never depends on the machine's.
        if jiff::tz::TimeZone::get(name).is_err() {
            checker.invalid(
                format!("`{name}` is not an IANA time zone, such as \"Asia/Singapore\" or \"UTC\""),
                tz.span(),
            );
        }
    }

    let extra = match site.get("extra").map(|e| to_value(e.get_ref())) {
        Some(toml::Value::Table(extra)) => extra,
        Some(_) => unreachable!("checked as a table"),
        None => toml::Table::new(),
    };
    Config {
        file: checker.file(),
        site: Site {
            url: string(site, "url").expect("required"),
            title: string(site, "title"),
            description: string(site, "description"),
            language: string(site, "language"),
            author: string(site, "author"),
            timezone: string(site, "timezone").unwrap_or_else(|| "UTC".into()),
            extra,
        },
        collections,
        taxonomies,
        data,
    }
}

fn collection(
    name: &Spanned<std::borrow::Cow<'_, str>>,
    c: &Table<'_>,
    taxonomy_fields: &[&str],
    checker: &mut Checker,
) -> Collection {
    let key = name;
    let name: &str = key.get_ref().as_ref();
    let mut fields = BTreeMap::new();
    for (field_name, spec) in entries(table(c, "fields")) {
        let at = field_name.span();
        let field_name = field_name.get_ref().as_ref();
        let spec = as_table(spec);
        if DERIVED.contains(&field_name) {
            checker.invalid(format!("`{field_name}` is a field yuanme derives from each Entry, so no field can take its name"), at);
        } else if taxonomy_fields.contains(&field_name) {
            checker.invalid(
                format!("`{field_name}` is the field the Taxonomy `{field_name}` adds to `{name}`"),
                at,
            );
        } else if field_name == "date" && spec["type"].get_ref().as_str() != Some("date") {
            let message =
                "`date` is a built-in field of type `date`: declaring it only makes it required"
                    .to_string();
            checker.invalid(message, spec["type"].span());
        } else if field_name != "date" && BUILT_IN.contains(&field_name) {
            checker.invalid(
                format!("`{field_name}` is a built-in field of every Entry"),
                at,
            );
        }
        fields.insert(field_name.to_string(), field(spec, true, checker));
    }
    let list_route = string(c, "list_route");
    let paginate = c.get("paginate");
    if let Some(paginate) = paginate
        && list_route.is_none()
    {
        checker.invalid(
            format!("`collections.{name}.paginate` needs a `list_route` to paginate"),
            paginate.span(),
        );
    }
    let (sortable, unsortable) = sortable(&fields);
    Collection {
        dir: string(c, "dir").unwrap_or_else(|| format!("content/{name}")),
        dir_at: c.get("dir").map_or(key.span(), |d| d.span()),
        route: string(c, "route").expect("required"),
        list_route,
        paginate: paginate.map(|p| int(p) as u64),
        sort: sort(c.get("sort"), &sortable, &unsortable, checker),
        fields,
    }
}

fn taxonomy(
    name: &Spanned<std::borrow::Cow<'_, str>>,
    t: &Table<'_>,
    names: &BTreeSet<&str>,
    collections: &BTreeMap<String, Collection>,
    checker: &mut Checker,
) -> Taxonomy {
    if BUILT_IN.iter().chain(DERIVED).any(|f| *f == name.get_ref()) {
        let message = format!(
            "`{}` is a built-in field of every Entry, so no Taxonomy can take its name",
            name.get_ref()
        );
        checker.invalid(message, name.span());
    }
    let no_fields = BTreeMap::new();
    let (mut sortable, mut unsortable) = sortable(&no_fields);
    for collection in items(&t["collections"]) {
        let s = collection.get_ref().as_str().expect("checked");
        if let Some(c) = collections.get(s) {
            let (yes, no) = self::sortable(&c.fields);
            sortable.extend(yes);
            unsortable.extend(no);
        } else if !names.contains(s) {
            checker.invalid(format!("there is no Collection `{s}`"), collection.span());
            if let Some(near) = similar(s, names.iter().copied()) {
                checker.suggest(
                    format!("did you mean `{near}`?"),
                    collection.span(),
                    format!("\"{near}\""),
                );
            }
        }
    }
    Taxonomy {
        collections: items(&t["collections"])
            .iter()
            .map(|c| c.get_ref().as_str().expect("checked").to_string())
            .collect(),
        route: string(t, "route").expect("required"),
        paginate: t.get("paginate").map(|p| int(p) as u64),
        single: boolean(t, "single"),
        sort: sort(t.get("sort"), &sortable, &unsortable, checker),
        lowercase: boolean(t, "lowercase"),
        slugs: entries(table(t, "slugs"))
            .iter()
            .map(|(term, slug)| {
                let s = slug.get_ref().as_str().expect("checked");
                if !crate::slug::is_valid(s) {
                    checker.invalid(format!("`{s}` is not a valid Slug"), slug.span());
                    let fixed = crate::slug::corrected(s);
                    if !fixed.is_empty() {
                        checker.suggest(
                            format!("use `{fixed}`"),
                            slug.span(),
                            format!("\"{fixed}\""),
                        );
                    }
                }
                (term.get_ref().to_string(), s.to_string())
            })
            .collect(),
    }
}

/// The fields Entries can sort by, and the ones they can't: lists and objects
/// have no order.
fn sortable(fields: &BTreeMap<String, Field>) -> (Vec<&str>, Vec<&str>) {
    let mut yes: Vec<&str> = BUILT_IN
        .iter()
        .chain(DERIVED)
        .copied()
        .filter(|f| *f != "headings")
        .collect();
    let mut no = vec!["headings"];
    for (name, field) in fields {
        match field.kind {
            FieldType::List { .. } | FieldType::Object(_) => no.push(name),
            _ => yes.push(name),
        }
    }
    (yes, no)
}

fn sort(
    keys: Option<&Value<'_>>,
    sortable: &[&str],
    unsortable: &[&str],
    checker: &mut Checker,
) -> Vec<SortKey> {
    let Some(keys) = keys else {
        return vec![SortKey {
            field: "date".into(),
            descending: true,
        }];
    };
    items(keys)
        .iter()
        .map(|key| {
            let s = key.get_ref().as_str().expect("checked");
            let (field, descending) = match s.strip_prefix('-') {
                Some(field) => (field, true),
                None => (s, false),
            };
            if field.is_empty() {
                checker.invalid(
                    "a sort key needs a field name, such as \"-date\"".into(),
                    key.span(),
                );
            } else if unsortable.contains(&field) && !sortable.contains(&field) {
                checker.invalid(
                    format!("Entries can't sort by `{field}`: it is a list or an object"),
                    key.span(),
                );
            } else if !sortable.contains(&field) {
                checker.invalid(
                    format!("there is no field `{field}` to sort by"),
                    key.span(),
                );
                if let Some(near) = similar(field, sortable.iter().copied()) {
                    let sign = if descending { "-" } else { "" };
                    checker.suggest(
                        format!("did you mean `{near}`?"),
                        key.span(),
                        format!("\"{sign}{near}\""),
                    );
                }
            }
            SortKey {
                field: field.to_string(),
                descending,
            }
        })
        .collect()
}

fn field(spec: &Table<'_>, in_table: bool, checker: &mut Checker) -> Field {
    let kind = match spec["type"].get_ref().as_str().expect("checked") {
        "string" => FieldType::String {
            min: spec.get("min").map(|v| int(v) as u64),
            max: spec.get("max").map(|v| int(v) as u64),
        },
        "int" => FieldType::Int {
            min: spec.get("min").map(int),
            max: spec.get("max").map(int),
        },
        "float" => FieldType::Float {
            min: spec.get("min").map(float),
            max: spec.get("max").map(float),
        },
        "bool" => FieldType::Bool,
        "date" => FieldType::Date,
        "datetime" => FieldType::Datetime,
        "url" => FieldType::Url,
        "image" => FieldType::Image,
        "enum" => {
            let values = &spec["values"];
            if items(values).is_empty() {
                checker.invalid("an `enum` needs at least one value".into(), values.span());
            }
            FieldType::Enum(
                items(values)
                    .iter()
                    .map(|v| v.get_ref().as_str().expect("checked").to_string())
                    .collect(),
            )
        }
        "list" => FieldType::List {
            items: Box::new(field(as_table(&spec["items"]), false, checker).kind),
            unique: boolean(spec, "unique"),
        },
        "object" => FieldType::Object(
            entries(table(spec, "fields"))
                .iter()
                .map(|(name, spec)| {
                    (
                        name.get_ref().to_string(),
                        field(as_table(spec), true, checker),
                    )
                })
                .collect(),
        ),
        _ => unreachable!("checked against FIELD_TYPES"),
    };
    if let (Some(min), Some(max)) = (spec.get("min"), spec.get("max"))
        && float(min) > float(max)
    {
        checker.invalid(
            format!("`min` ({}) is more than `max` ({})", text(min), text(max)),
            min.span(),
        );
    }
    let default = spec.get("default").filter(|_| in_table);
    if let Some(default) = default
        && let Err((at, message)) = check_default(&kind, default)
    {
        checker.invalid(message, at);
    }
    Field {
        optional: boolean(spec, "optional") || default.is_some(),
        default: default.map(|d| to_value(d.get_ref())),
        description: string(spec, "description"),
        kind,
    }
}

/// Whether `value` is a valid value of `kind`, or the innermost part that isn't.
fn check_default(ty: &FieldType, value: &Value<'_>) -> Result<(), (Range<usize>, String)> {
    let at = value.span();
    let v = value.get_ref();
    let fail = |message: String| Err((at.clone(), message));
    let not = |expected: &str| {
        Err((
            at.clone(),
            format!("the default must be {expected}, not {}", kind(v)),
        ))
    };
    let bounds = |n: f64, min: Option<f64>, max: Option<f64>, what: &str| match (min, max) {
        (Some(min), _) if n < min => fail(format!("the default {what} is less than `min` ({min})")),
        (_, Some(max)) if n > max => fail(format!("the default {what} is more than `max` ({max})")),
        _ => Ok(()),
    };
    match (ty, v) {
        (FieldType::String { min, max }, DeValue::String(s)) => {
            let n = s.chars().count() as f64;
            bounds(
                n,
                min.map(|m| m as f64),
                max.map(|m| m as f64),
                "has a length that",
            )
        }
        (FieldType::Int { min, max }, DeValue::Integer(_)) => bounds(
            float(value),
            min.map(|m| m as f64),
            max.map(|m| m as f64),
            &format!("`{}`", text(value)),
        ),
        (FieldType::Float { min, max }, DeValue::Integer(_) | DeValue::Float(_)) => {
            bounds(float(value), *min, *max, &format!("`{}`", text(value)))
        }
        (FieldType::Bool, DeValue::Boolean(_)) => Ok(()),
        (FieldType::Date, DeValue::Datetime(d)) if d.date.is_some() && d.time.is_none() => Ok(()),
        (FieldType::Date, _) => not("a date such as 2025-12-12"),
        (FieldType::Datetime, DeValue::Datetime(d)) if d.date.is_some() && d.time.is_some() => {
            Ok(())
        }
        (FieldType::Datetime, _) => not("a datetime such as 2025-12-12T09:30:00+08:00"),
        (FieldType::Url, DeValue::String(s))
            if s.starts_with("https://") || s.starts_with("http://") =>
        {
            Ok(())
        }
        (FieldType::Url, _) => not("an absolute URL"),
        (FieldType::Image, DeValue::String(_)) => Ok(()),
        (FieldType::Enum(values), DeValue::String(s)) if values.iter().any(|v| v == s) => Ok(()),
        (FieldType::Enum(values), _) => {
            let values: Vec<_> = values.iter().map(|v| format!("\"{v}\"")).collect();
            not(&format!("one of {}", values.join(", ")))
        }
        (FieldType::List { items, unique }, DeValue::Array(list)) => {
            for item in list.iter() {
                check_default(items, item)?;
            }
            if *unique {
                for (i, item) in list.iter().enumerate() {
                    if list[..i]
                        .iter()
                        .any(|before| to_value(before.get_ref()) == to_value(item.get_ref()))
                    {
                        return Err((
                            item.span(),
                            "the default repeats an item of a `unique` list".into(),
                        ));
                    }
                }
            }
            Ok(())
        }
        (FieldType::Object(fields), DeValue::Table(table)) => {
            for (name, value) in table.iter() {
                match fields.get(name.get_ref().as_ref()) {
                    Some(field) => check_default(&field.kind, value)?,
                    None => {
                        return Err((
                            name.span(),
                            format!("the object has no field `{}`", name.get_ref()),
                        ));
                    }
                }
            }
            match fields
                .iter()
                .find(|(name, f)| !f.optional && !table.contains_key(name.as_str()))
            {
                Some((name, _)) => fail(format!(
                    "the default leaves out the required field `{name}`"
                )),
                None => Ok(()),
            }
        }
        (FieldType::String { .. }, _) => not("a string"),
        (FieldType::Int { .. }, _) => not("an integer"),
        (FieldType::Float { .. }, _) => not("a number"),
        (FieldType::Bool, _) => not("a boolean"),
        (FieldType::Image, _) => not("a path or URL string"),
        (FieldType::List { .. }, _) => not("an array"),
        (FieldType::Object(_), _) => not("a table"),
    }
}

fn entries<'a, 'i>(
    table: Option<&'a Table<'i>>,
) -> Vec<(&'a Spanned<std::borrow::Cow<'i, str>>, &'a Value<'i>)> {
    table.map_or_else(Vec::new, |t| t.iter().collect())
}

fn table<'a, 'i>(t: &'a Table<'i>, key: &str) -> Option<&'a Table<'i>> {
    t.get(key).map(as_table)
}

fn as_table<'a, 'i>(v: &'a Value<'i>) -> &'a Table<'i> {
    match v.get_ref() {
        DeValue::Table(t) => t,
        _ => unreachable!("checked as a table"),
    }
}

fn items<'a, 'i>(v: &'a Value<'i>) -> &'a [Value<'i>] {
    match v.get_ref() {
        DeValue::Array(a) => a,
        _ => unreachable!("checked as an array"),
    }
}

fn string(t: &Table<'_>, key: &str) -> Option<String> {
    t.get(key).map(|v| {
        v.get_ref()
            .as_str()
            .expect("checked as a string")
            .to_string()
    })
}

fn boolean(t: &Table<'_>, key: &str) -> bool {
    matches!(
        t.get(key).map(|v| v.get_ref()),
        Some(DeValue::Boolean(true))
    )
}

fn int(v: &Value<'_>) -> i64 {
    match v.get_ref() {
        DeValue::Integer(i) => i64::from_str_radix(i.as_str(), i.radix()).expect("checked to fit"),
        _ => unreachable!("checked as an integer"),
    }
}

fn float(v: &Value<'_>) -> f64 {
    match v.get_ref() {
        DeValue::Integer(_) => int(v) as f64,
        DeValue::Float(f) => f.as_str().parse().expect("TOML floats parse"),
        _ => unreachable!("checked as a number"),
    }
}

/// A number as the Author wrote it, for messages.
fn text(v: &Value<'_>) -> String {
    match v.get_ref() {
        DeValue::Integer(_) => int(v).to_string(),
        _ => float(v).to_string(),
    }
}

fn to_value(v: &DeValue<'_>) -> toml::Value {
    match v {
        DeValue::String(s) => toml::Value::String(s.to_string()),
        DeValue::Integer(i) => toml::Value::Integer(
            i64::from_str_radix(i.as_str(), i.radix()).expect("checked to fit"),
        ),
        DeValue::Float(f) => toml::Value::Float(f.as_str().parse().expect("TOML floats parse")),
        DeValue::Boolean(b) => toml::Value::Boolean(*b),
        DeValue::Datetime(d) => toml::Value::Datetime(*d),
        DeValue::Array(a) => toml::Value::Array(a.iter().map(|v| to_value(v.get_ref())).collect()),
        DeValue::Table(t) => toml::Value::Table(
            t.iter()
                .map(|(k, v)| (k.get_ref().to_string(), to_value(v.get_ref())))
                .collect(),
        ),
    }
}
