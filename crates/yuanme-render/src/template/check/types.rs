//! The types `check` gives values: what Site config declares, what a
//! Data file holds, and what a Template builds.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::rc::Rc;

use minijinja::value::ValueKind;

use super::Sig;

#[derive(Clone)]
pub(super) enum Ty {
    /// Unchecked: `object`, or what `check` can't follow.
    Any,
    None,
    Str,
    Int,
    Float,
    Bool,
    Date,
    Datetime,
    Url,
    Image,
    Enum(Rc<Vec<String>>),
    List(Rc<Ty>),
    Obj(Rc<Obj>),
    /// Finished HTML: `entry.content`, a `{% set %}` block, a macro's output.
    Slot,
    Attrs,
    Macro(Rc<Sig>),
    /// `{% import "x.html" as ui %}`, by the Template's name.
    Module(Rc<str>),
}

pub(super) struct Obj {
    /// As a type is written, such as `entry<posts>` or `data.nav`.
    pub(super) name: String,
    pub(super) fields: BTreeMap<String, Field>,
    /// A map literal, whose other keys are unknown rather than absent.
    pub(super) open: bool,
}

#[derive(Clone)]
pub(super) struct Field {
    pub(super) ty: Ty,
    pub(super) optional: bool,
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Any => f.write_str("object"),
            Ty::None => f.write_str("none"),
            Ty::Str => f.write_str("string"),
            Ty::Int => f.write_str("int"),
            Ty::Float => f.write_str("float"),
            Ty::Bool => f.write_str("bool"),
            Ty::Date => f.write_str("date"),
            Ty::Datetime => f.write_str("datetime"),
            Ty::Url => f.write_str("url"),
            Ty::Image => f.write_str("image"),
            Ty::Enum(values) => write!(f, "enum({})", values.join("|")),
            Ty::List(t) => write!(f, "list<{t}>"),
            Ty::Obj(o) => f.write_str(&o.name),
            Ty::Slot => f.write_str("slot"),
            Ty::Attrs => f.write_str("attrs"),
            Ty::Macro(m) => write!(f, "macro `{}`", m.name),
            Ty::Module(name) => write!(f, "the import of `{name}`"),
        }
    }
}

pub(super) fn obj(
    name: impl Into<String>,
    fields: impl IntoIterator<Item = (&'static str, Ty, bool)>,
) -> Ty {
    Ty::Obj(Rc::new(Obj {
        name: name.into(),
        fields: fields
            .into_iter()
            .map(|(k, ty, optional)| (k.to_string(), Field { ty, optional }))
            .collect(),
        open: false,
    }))
}

/// Whether a value of type `arg` may be passed where `param` is declared.
pub(super) fn assignable(arg: &Ty, param: &Ty) -> bool {
    use Ty::*;
    match (arg, param) {
        (Any, _) | (_, Any) | (None, _) => true,
        (Str | Url | Enum(_) | Image | Slot, Str) => true,
        (Str | Url | Image, Url | Image) => true,
        (Str | Enum(_), Enum(_)) => true,
        (Int, Int) | (Int | Float, Float) | (Bool, Bool) => true,
        (Date, Date) | (Date | Datetime, Datetime) => true,
        (Str | Slot, Slot) => true,
        (Obj(_) | Attrs, Attrs) => true,
        (List(a), List(p)) => assignable(a, p),
        (Obj(a), Obj(p)) => Rc::ptr_eq(a, p) || a.name == p.name,
        _ => false,
    }
}

/// One type for two values a name can hold, such as the Entries of two
/// Collections that share a Template: a field only one has is optional.
pub(super) fn merge(a: &Ty, b: &Ty) -> Ty {
    match (a, b) {
        (Ty::Obj(x), Ty::Obj(y)) if Rc::ptr_eq(x, y) => a.clone(),
        (Ty::Obj(x), Ty::Obj(y)) => {
            let mut fields = BTreeMap::new();
            for name in x.fields.keys().chain(y.fields.keys()) {
                let field = match (x.fields.get(name), y.fields.get(name)) {
                    (Some(p), Some(q)) => Field {
                        ty: merge(&p.ty, &q.ty),
                        optional: p.optional || q.optional,
                    },
                    (Some(p), None) | (None, Some(p)) => Field {
                        ty: p.ty.clone(),
                        optional: true,
                    },
                    (None, None) => unreachable!("a key of one of them"),
                };
                fields.insert(name.clone(), field);
            }
            // Each type named once, however many merges it went through.
            let names: BTreeSet<&str> = x.name.split(" or ").chain(y.name.split(" or ")).collect();
            let name = names.into_iter().collect::<Vec<_>>().join(" or ");
            Ty::Obj(Rc::new(Obj {
                name,
                fields,
                open: x.open || y.open,
            }))
        }
        (Ty::List(x), Ty::List(y)) => Ty::List(Rc::new(merge(x, y))),
        (Ty::Enum(x), Ty::Enum(y)) => {
            let all: BTreeSet<String> = x.iter().chain(y.iter()).cloned().collect();
            Ty::Enum(Rc::new(all.into_iter().collect()))
        }
        (Ty::None, other) | (other, Ty::None) => other.clone(),
        _ if std::mem::discriminant(a) == std::mem::discriminant(b) => a.clone(),
        _ => Ty::Any,
    }
}

pub(super) fn list_of(items: impl Iterator<Item = Ty>) -> Ty {
    let item = items.reduce(|a, b| merge(&a, &b)).unwrap_or(Ty::Any);
    Ty::List(Rc::new(item))
}

pub(super) fn closed<'k>(path: &str, fields: impl Iterator<Item = (&'k String, Ty)>) -> Ty {
    Ty::Obj(Rc::new(Obj {
        name: path.into(),
        fields: fields
            .map(|(k, ty)| {
                (
                    k.clone(),
                    Field {
                        ty,
                        optional: false,
                    },
                )
            })
            .collect(),
        open: false,
    }))
}

pub(super) fn const_ty(value: &minijinja::Value) -> Ty {
    match value.kind() {
        ValueKind::String => Ty::Str,
        ValueKind::Bool => Ty::Bool,
        ValueKind::Number if value.is_integer() => Ty::Int,
        ValueKind::Number => Ty::Float,
        ValueKind::None => Ty::None,
        _ => Ty::Any,
    }
}
