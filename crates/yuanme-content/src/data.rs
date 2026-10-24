//! Data files: `data/<name>.toml|yaml|yml|json`, which Templates read as
//! `data.<name>` (#18). A file with a `[data.<name>]` schema is checked
//! against it with the same rules as an Entry's fields.

use std::collections::BTreeMap;
use std::ops::Range;

use jiff::tz::TimeZone;
use toml::de::{DeTable, DeValue};
use yuanme_diag::{Diagnostic, Span, codes};

use crate::config::Config;
use crate::entry::yaml_error;
use crate::fields::{self, Cx, Value};
use crate::site::Project;
use crate::yaml::{self, Key, Node, Yaml};

/// Every Data file's value, by name. A file with an error is reported and
/// left out.
pub(crate) fn read(
    config: &Config,
    project: &Project<'_>,
    timezone: &TimeZone,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    let mut seen: BTreeMap<String, yuanme_diag::FileId> = BTreeMap::new();
    for &file in &project.data {
        let path = project.files.path(file);
        let text = project.files.text(file);
        let file_name = path.strip_prefix("data/").expect("a Data file");
        let (name, ext) = file_name.rsplit_once('.').expect("has an extension");
        if let Some(&first) = seen.get(name) {
            let message = format!(
                "`{}` and `{path}` are both `data.{name}`",
                project.files.path(first)
            );
            let span = |file| Span { file, range: 0..0 };
            diagnostics.push(
                Diagnostic::error(&codes::ENTRY_DUPLICATE_DATA_FILE, message, span(file))
                    .secondary(span(first), "the other file"),
            );
            continue;
        }
        seen.insert(name.to_string(), file);
        let mut cx = Cx {
            file,
            diagnostics,
            timezone,
            dir: "",
            is_file: project.is_file,
        };
        let node = match ext {
            "toml" => toml_node(text, &mut cx),
            _ => match yaml::parse(text, 0) {
                Ok(node) => Some(node),
                Err(e) => {
                    cx.push(yaml_error(e, &cx));
                    None
                }
            },
        };
        let Some(node) = node else {
            continue;
        };
        let value = match config.data.get(name) {
            Some(schema) => fields::value(&schema.kind, &node, &format!("data.{name}"), &mut cx),
            None => untyped(&node, &mut cx),
        };
        if let Some(v) = value {
            out.insert(name.to_string(), v);
        }
    }
    for (name, schema) in &config.data {
        if !seen.contains_key(name) {
            let message = format!(
                "`[data.{name}]` has no Data file: expected `data/{name}.toml`, `.yaml` or `.json`"
            );
            diagnostics.push(Diagnostic::error(
                &codes::CONFIG_INVALID_VALUE,
                message,
                Span {
                    file: config.file,
                    range: schema.at.clone(),
                },
            ));
        }
    }
    out
}

/// A Data file without a schema, read as it is written.
fn untyped(node: &Node, cx: &mut Cx<'_>) -> Option<Value> {
    Some(match &node.value {
        Yaml::Null => {
            let message = "an empty value: leave the key out instead".to_string();
            cx.push(cx.error(&codes::ENTRY_WRONG_TYPE, message, node.span.clone()));
            return None;
        }
        Yaml::Bool(b) => Value::Bool(*b),
        Yaml::Int(i) => Value::Int(*i),
        Yaml::Float(f) => Value::Float(*f),
        Yaml::Str(s) => Value::String(s.clone()),
        Yaml::Seq(items) => Value::List(
            items
                .iter()
                .map(|n| untyped(n, cx))
                .collect::<Option<_>>()?,
        ),
        Yaml::Map(entries) => Value::Object(
            entries
                .iter()
                .map(|(k, v)| Some((k.name.clone(), untyped(v, cx)?)))
                .collect::<Option<_>>()?,
        ),
    })
}

/// TOML read into the same spanned tree as YAML, so one checker serves both.
/// A TOML date or time becomes its text, which a `date` field then reads.
fn toml_node(text: &str, cx: &mut Cx<'_>) -> Option<Node> {
    let (root, errors) = DeTable::parse_recoverable(text);
    for e in &errors {
        let at = e.span().unwrap_or(0..0);
        cx.push(cx.error(
            &codes::ENTRY_SYNTAX,
            format!("invalid TOML: {}", e.message()),
            at,
        ));
    }
    if !errors.is_empty() {
        return None;
    }
    // The root table's own span is empty; a whole-file error covers the file.
    Some(Node {
        value: table(root.get_ref()),
        span: 0..text.len(),
    })
}

fn table(t: &DeTable<'_>) -> Yaml {
    Yaml::Map(
        t.iter()
            .map(|(k, v)| {
                (
                    Key {
                        name: k.get_ref().to_string(),
                        span: k.span(),
                    },
                    node(v.get_ref(), v.span()),
                )
            })
            .collect(),
    )
}

fn node(v: &DeValue<'_>, span: Range<usize>) -> Node {
    let value = match v {
        DeValue::String(s) => Yaml::Str(s.to_string()),
        DeValue::Integer(i) => i64::from_str_radix(i.as_str(), i.radix())
            .map_or_else(|_| Yaml::Str(i.as_str().into()), Yaml::Int),
        DeValue::Float(f) => Yaml::Float(f.as_str().parse().unwrap_or(f64::NAN)),
        DeValue::Boolean(b) => Yaml::Bool(*b),
        DeValue::Datetime(d) => Yaml::Str(d.to_string()),
        DeValue::Array(a) => Yaml::Seq(a.iter().map(|v| node(v.get_ref(), v.span())).collect()),
        DeValue::Table(t) => table(t),
    };
    Node { value, span }
}
