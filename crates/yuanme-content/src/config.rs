//! Site config: `yuanme.toml`, read into typed values with a Diagnostic for
//! every mistake in it (#18, #21).

mod budgets;
mod build;
mod check;
mod collections;
mod data;
mod fields;
mod host;
mod images;
mod math;
mod routes;
mod schema;
mod site;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use toml::Spanned;
use toml::de::{DeTable, DeValue};
use yuanme_diag::{Diagnostic, FileId, Files, codes};

pub use budgets::{Budget, Budgets};
pub use build::RouterMode;
pub(crate) use build::{Agents, Build, Tailwind, TypeScript};
pub use check::similar;
use check::{Checker, parse_version};
pub use collections::Collection;
pub(crate) use collections::{SortKey, Taxonomy};
pub(crate) use data::DataSchema;
pub use fields::{Field, FieldType};
pub use host::{Adapter, HeaderRule, Host};
pub use images::Images;
pub(crate) use math::{Math, TheoremKind};
pub(crate) use routes::{Feed, TemplateRoute};
#[cfg(test)]
use schema::{Key, Shape};
pub use schema::{data_schema, entry_schema, json_schema};
pub(crate) use site::Site;

#[derive(Debug)]
pub struct Config {
    /// `yuanme.toml`, which the spans below are in.
    pub file: FileId,
    pub site: Site,
    pub collections: BTreeMap<String, Collection>,
    pub taxonomies: BTreeMap<String, Taxonomy>,
    /// Schemas for Data files, by name.
    pub data: BTreeMap<String, DataSchema>,
    pub routes: BTreeMap<String, TemplateRoute>,
    pub feeds: BTreeMap<String, Feed>,
    pub build: Build,
    /// `[router] mode`.
    pub router: RouterMode,
    pub tailwind: Tailwind,
    pub typescript: TypeScript,
    pub agents: Agents,
    pub host: Host,
    pub math: Math,
    pub images: Images,
    /// `None` for `budgets = false`.
    pub budgets: Option<Budgets>,
}

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
/// what they refer to. Each section reads its own table.
fn read(root: &Table<'_>, checker: &mut Checker) -> Config {
    let (collections, taxonomies) = collections::read(
        table(root, "collections"),
        table(root, "taxonomies"),
        checker,
    );
    let routes = routes::read(table(root, "routes"));
    let feeds = routes::feeds(table(root, "feeds"), &collections, checker);
    let data = data::read(table(root, "data"), checker);
    let math = math::read(table(root, "math"), checker);
    let site = site::read(
        table(root, "site").expect("the first pass requires `[site]`"),
        checker,
    );
    let build = build::read(table(root, "build"));
    let router = build::router(table(root, "router"));
    let tailwind = build::tailwind(table(root, "tailwind"));
    let typescript = build::typescript(table(root, "typescript"));
    let agents = build::agents(table(root, "agents"));
    let host = host::read(table(root, "host"), checker);
    let images = images::read(table(root, "images"), checker);
    let budgets = match root.get("budgets") {
        Some(v) if matches!(v.get_ref(), DeValue::Boolean(false)) => None,
        v => Some(budgets::read(v.map(as_table), checker)),
    };
    Config {
        file: checker.file(),
        site,
        collections,
        taxonomies,
        data,
        routes,
        feeds,
        build,
        router,
        tailwind,
        typescript,
        agents,
        host,
        math,
        images,
        budgets,
    }
}

// The helpers below read values the first pass checked against the schema,
// so a value of another type or a required key left out is a bug in it.

fn strings(v: &Value<'_>) -> Vec<String> {
    items(v).iter().map(|s| as_str(s).to_string()).collect()
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

fn as_str<'a>(v: &'a Value<'_>) -> &'a str {
    v.get_ref()
        .as_str()
        .expect("the first pass checked it is a string")
}

fn string(t: &Table<'_>, key: &str) -> Option<String> {
    t.get(key).map(|v| as_str(v).to_string())
}

fn required(t: &Table<'_>, key: &str) -> String {
    string(t, key).expect("the first pass checked a required key is there")
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
