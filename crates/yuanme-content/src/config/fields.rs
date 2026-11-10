//! The fields a Collection or Data file declares, and their types.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;

use jiff::fmt::temporal::{Pieces, PiecesOffset};
use jiff::tz::{Offset, TimeZone};
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
        && let Err(e) = check_default(&kind, default)
    {
        checker.invalid(e.to_string(), e.at);
    }
    Field {
        optional: boolean(spec, "optional") || default.is_some(),
        default: default.map(|d| to_value(d.get_ref())),
        description: string(spec, "description"),
        kind,
    }
}

/// The innermost part of a default that isn't a valid value of its type,
/// and what is wrong with it.
#[derive(Debug)]
struct BadDefault {
    at: Range<usize>,
    problem: Problem,
}

#[derive(Debug)]
enum Problem {
    /// Not `expected`; `found` is the TOML type it is.
    Type {
        expected: String,
        found: &'static str,
    },
    TooSmall {
        what: Measured,
        min: f64,
    },
    TooLarge {
        what: Measured,
        max: f64,
    },
    /// An item of a `unique` list that an earlier one equals.
    Repeated,
    UnknownField(String),
    MissingField(String),
    /// A datetime after 9999-12-30T22:00:00Z, the last instant jiff holds.
    PastTheLastInstant,
}

/// What `min` and `max` bound: a string's length, or the number written.
#[derive(Debug)]
enum Measured {
    Length,
    Number(String),
}

impl fmt::Display for BadDefault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = |m: &Measured| match m {
            Measured::Length => "has a length that".to_string(),
            Measured::Number(n) => format!("`{n}`"),
        };
        match &self.problem {
            Problem::Type { expected, found } => {
                write!(f, "the default must be {expected}, not {found}")
            }
            Problem::TooSmall { what: m, min } => {
                write!(f, "the default {} is less than `min` ({min})", what(m))
            }
            Problem::TooLarge { what: m, max } => {
                write!(f, "the default {} is more than `max` ({max})", what(m))
            }
            Problem::Repeated => f.write_str("the default repeats an item of a `unique` list"),
            Problem::UnknownField(name) => write!(f, "the object has no field `{name}`"),
            Problem::MissingField(name) => {
                write!(f, "the default leaves out the required field `{name}`")
            }
            Problem::PastTheLastInstant => f.write_str(
                "the default comes after 9999-12-30T22:00:00Z, the last instant a datetime can be (without an offset, in some time zone)",
            ),
        }
    }
}

impl std::error::Error for BadDefault {}

/// Whether a datetime default is an instant jiff holds. One without an
/// offset is in `site.timezone`, which config doesn't know here, so it
/// must be one in every offset a zone can have.
fn an_instant(d: &toml::value::Datetime) -> bool {
    let written = d.to_string();
    let Ok(p) = Pieces::parse(&written) else {
        return false;
    };
    let Some(time) = p.time() else {
        return false;
    };
    let offsets = match p.offset() {
        Some(PiecesOffset::Numeric(o)) => vec![o.offset()],
        Some(_) => vec![Offset::UTC],
        None => vec![Offset::MIN, Offset::MAX],
    };
    let dt = p.date().to_datetime(time);
    offsets
        .into_iter()
        .all(|o| dt.to_zoned(TimeZone::fixed(o)).is_ok())
}

/// Whether `value` is a valid value of `kind`.
fn check_default(ty: &FieldType, value: &Value<'_>) -> Result<(), BadDefault> {
    let at = value.span();
    let v = value.get_ref();
    let fail = |problem| {
        Err(BadDefault {
            at: at.clone(),
            problem,
        })
    };
    let not = |expected: &str| {
        fail(Problem::Type {
            expected: expected.to_string(),
            found: kind(v),
        })
    };
    let bounds = |n: f64, min: Option<f64>, max: Option<f64>, what: Measured| match (min, max) {
        (Some(min), _) if n < min => fail(Problem::TooSmall { what, min }),
        (_, Some(max)) if n > max => fail(Problem::TooLarge { what, max }),
        _ => Ok(()),
    };
    match (ty, v) {
        (FieldType::String { min, max }, DeValue::String(s)) => {
            let n = s.chars().count() as f64;
            bounds(
                n,
                min.map(|m| m as f64),
                max.map(|m| m as f64),
                Measured::Length,
            )
        }
        (FieldType::Int { min, max }, DeValue::Integer(_)) => bounds(
            float(value),
            min.map(|m| m as f64),
            max.map(|m| m as f64),
            Measured::Number(text(value)),
        ),
        (FieldType::Float { min, max }, DeValue::Integer(_) | DeValue::Float(_)) => {
            bounds(float(value), *min, *max, Measured::Number(text(value)))
        }
        (FieldType::Bool, DeValue::Boolean(_)) => Ok(()),
        (FieldType::Date, DeValue::Datetime(d)) if d.date.is_some() && d.time.is_none() => Ok(()),
        (FieldType::Date, _) => not("a date such as 2025-12-12"),
        (FieldType::Datetime, DeValue::Datetime(d)) if d.date.is_some() && d.time.is_some() => {
            match an_instant(d) {
                true => Ok(()),
                false => fail(Problem::PastTheLastInstant),
            }
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
                        return Err(BadDefault {
                            at: item.span(),
                            problem: Problem::Repeated,
                        });
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
                        return Err(BadDefault {
                            at: name.span(),
                            problem: Problem::UnknownField(name.get_ref().to_string()),
                        });
                    }
                }
            }
            match fields
                .iter()
                .find(|(name, f)| !f.optional && !table.contains_key(name.as_str()))
            {
                Some((name, _)) => fail(Problem::MissingField(name.clone())),
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
