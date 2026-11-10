//! The fields a Collection or Data file declares, and their types.

use std::collections::BTreeMap;
use std::ops::Range;

use toml::de::DeValue;

use super::check::{Checker, kind};
use super::{
    Table, Value, as_str, as_table, boolean, entries, float, int, items, string, table, text,
    to_value,
};

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

pub(super) fn field(spec: &Table<'_>, in_table: bool, checker: &mut Checker) -> Field {
    let kind = match as_str(&spec["type"]) {
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
                    .map(|v| as_str(v).to_string())
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
