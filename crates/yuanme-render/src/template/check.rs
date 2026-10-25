//! `check` (#19): every Template walked against the schema of what its
//! Pages see, before any Page renders. It finds what rendering can't: a
//! typo on a path no Entry reaches or one `| default` hides, a Component
//! reading what it wasn't given, and a prop of the wrong type. minijinja's
//! AST has no stability promise, so all that reads it is here (#24).
//!
//! It reads each Template as the Author wrote it, so its spans need no
//! SourceMap; render errors go through one once scoped styles rewrite
//! Templates (ADR 0006).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;
use std::rc::Rc;

use minijinja::machinery::ast::{self, CallArg, Expr, Spanned, Stmt};
use minijinja::machinery::{Token, parse, tokenize};
use minijinja::syntax::SyntaxConfig;
use minijinja::value::ValueKind;
use minijinja::{Environment, context};
use yuanme_content::config::{Config, Field as Declared, FieldType, similar};
use yuanme_content::fields;
use yuanme_content::site::{self, RouteKind, Site};
use yuanme_diag::{Applicability, Code, Diagnostic, Edit, FileId, Files, Span, codes};

use super::filters::{known_format, unknown_format};

/// What every Page sees, and so what a Component may read besides its props.
const GLOBALS: [&str; 5] = ["site", "data", "collections", "taxonomies", "page"];

/// For "did you mean": minijinja can't list its filters and tests, so these
/// are only suggestions. Whether a name exists is asked of the Environment.
#[rustfmt::skip]
const FILTERS: &[&str] = &[
    "abs", "attr", "batch", "bool", "capitalize", "chain", "count", "d", "default", "dictsort", "e",
    "escape", "first", "float", "format", "groupby", "indent", "int", "items", "join", "last",
    "length", "lines", "list", "lower", "map", "max", "min", "pprint", "reject", "rejectattr",
    "replace", "reverse", "round", "safe", "select", "selectattr", "slice", "sort", "split",
    "string", "sum", "title", "tojson", "trim", "unique", "upper", "urlencode", "zip",
    "sort_by", "dateformat", "pluralize", "attrs", "markdown", "org",
];
#[rustfmt::skip]
const TESTS: &[&str] = &[
    "defined", "undefined", "none", "safe", "escaped", "boolean", "odd", "even", "divisibleby",
    "number", "integer", "float", "string", "sequence", "iterable", "mapping", "startingwith",
    "endingwith", "lower", "upper", "sameas", "eq", "equalto", "ne", "lt", "lessthan", "le", "gt",
    "greaterthan", "ge", "in", "true", "false", "filter", "test",
];
const TYPES: &[&str] = &[
    "string",
    "int",
    "float",
    "bool",
    "date",
    "datetime",
    "url",
    "image",
    "enum",
    "list",
    "object",
    "slot",
    "attrs",
    "term",
    "paginator",
];

#[derive(Clone)]
enum Ty {
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

struct Obj {
    /// As a type is written, such as `entry<posts>` or `data.nav`.
    name: String,
    fields: BTreeMap<String, Field>,
    /// A map literal, whose other keys are unknown rather than absent.
    open: bool,
}

#[derive(Clone)]
struct Field {
    ty: Ty,
    optional: bool,
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

fn obj(name: impl Into<String>, fields: impl IntoIterator<Item = (&'static str, Ty, bool)>) -> Ty {
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
fn assignable(arg: &Ty, param: &Ty) -> bool {
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
fn merge(a: &Ty, b: &Ty) -> Ty {
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
            let name = match x.name == y.name {
                true => x.name.clone(),
                false => format!("{} or {}", x.name, y.name),
            };
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

/// A macro, as its signature and props comment declare it.
struct Sig {
    name: String,
    /// Its Template, such as `components/row.html`.
    file: String,
    params: Vec<Param>,
    /// From the props comment. A macro outside `components/` may have
    /// none, and then only its arity is checked.
    props: Option<Vec<Prop>>,
    slot: Slot,
}

struct Param {
    name: String,
    has_default: bool,
    /// `=none`, which leaves the prop absent to the macro body.
    none_default: bool,
}

#[derive(Clone)]
struct Prop {
    name: String,
    ty: Ty,
    optional: bool,
    /// The entry in its comment.
    at: Range<usize>,
}

/// Whether a Component takes a `{% call %}` body, from its use of `caller`.
#[derive(Clone, Copy, PartialEq)]
enum Slot {
    No,
    Optional,
    Required,
}

#[derive(Clone)]
struct Bind {
    ty: Ty,
    optional: bool,
    /// A string literal it was set to, for a Layout's `enum` vars.
    literal: Option<String>,
    /// Where it was set, for a Layout's vars.
    at: Option<Span>,
}

impl Bind {
    fn new(ty: Ty, optional: bool) -> Bind {
        Bind {
            ty,
            optional,
            literal: None,
            at: None,
        }
    }
}

/// What a use of a value requires of it.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    /// Printed, looped over, filtered: an absent value fails the Page.
    Use,
    /// `==`, `<`, `in`: an absent value fails the Page, even under SemiStrict.
    Compare,
    /// Tested for truth: an absent value is false.
    Guard,
    /// Bound or passed on, where it stays optional.
    Pass,
}

/// The types of what Templates see, from Site config and the Site (#19).
struct Schema {
    entries: BTreeMap<String, Ty>,
    term: Ty,
    taxonomy: Ty,
    paginator: Ty,
    collection: Ty,
    feed: Ty,
    globals: Vec<(&'static str, Ty)>,
}

impl Schema {
    fn new(config: &Config, site: &Site) -> Schema {
        let collections = site::collections(config);
        let mut entries = BTreeMap::new();
        for (name, c) in &collections {
            let mut fields: BTreeMap<String, Field> = c
                .fields
                .iter()
                .map(|(k, f)| (k.clone(), declared(&format!("entry<{name}>.{k}"), f)))
                .collect();
            let dated = fields.get("date").is_some_and(|f| !f.optional);
            let heading = obj(
                "heading",
                [
                    ("id", Ty::Str, false),
                    ("text", Ty::Str, false),
                    ("level", Ty::Int, false),
                ],
            );
            for (k, ty, optional) in [
                ("title", Ty::Str, false),
                ("slug", Ty::Str, false),
                ("draft", Ty::Bool, false),
                ("unlisted", Ty::Bool, false),
                ("date", Ty::Date, !dated),
                ("updated", Ty::Date, true),
                ("modified", Ty::Date, !dated),
                ("url", Ty::Url, false),
                ("content", Ty::Slot, false),
                ("headings", Ty::List(Rc::new(heading)), false),
                ("word_count", Ty::Int, false),
                ("reading_time", Ty::Int, false),
            ] {
                fields.insert(k.into(), Field { ty, optional });
            }
            for (t, taxonomy) in &config.taxonomies {
                if taxonomy.collections.contains(name) {
                    let field = match taxonomy.single {
                        true => Field {
                            ty: Ty::Str,
                            optional: true,
                        },
                        false => Field {
                            ty: Ty::List(Rc::new(Ty::Str)),
                            optional: false,
                        },
                    };
                    fields.insert(t.clone(), field);
                }
            }
            let entry = Ty::Obj(Rc::new(Obj {
                name: format!("entry<{name}>"),
                fields,
                open: false,
            }));
            entries.insert(name.clone(), entry);
        }
        let term = obj(
            "term",
            [
                ("name", Ty::Str, false),
                ("slug", Ty::Str, false),
                ("url", Ty::Url, false),
                ("count", Ty::Int, false),
            ],
        );
        let taxonomy = obj(
            "taxonomy",
            [
                ("name", Ty::Str, false),
                ("url", Ty::Url, false),
                ("terms", Ty::List(Rc::new(term.clone())), false),
            ],
        );
        let paginator = obj(
            "paginator",
            [
                ("number", Ty::Int, false),
                ("total", Ty::Int, false),
                ("prev", Ty::Url, true),
                ("next", Ty::Url, true),
            ],
        );
        let named =
            |name: &'static str| obj(name, [("name", Ty::Str, false), ("url", Ty::Url, false)]);
        let s = &config.site;
        let mut site_fields = vec![
            ("url", Ty::Url, false),
            ("timezone", Ty::Str, false),
            (
                "extra",
                toml("site.extra", &toml::Value::Table(s.extra.clone())),
                false,
            ),
        ];
        for (k, v) in [
            ("title", &s.title),
            ("description", &s.description),
            ("language", &s.language),
            ("author", &s.author),
        ] {
            site_fields.push((k, Ty::Str, v.is_none()));
        }
        let data = closed(
            "data",
            site.data.iter().map(|(name, v)| {
                let path = format!("data.{name}");
                let ty = match config.data.get(name) {
                    Some(schema) => field_ty(&path, &schema.kind),
                    None => infer(&path, v),
                };
                (name, ty)
            }),
        );
        let collections_ty = closed(
            "collections",
            collections
                .keys()
                .map(|n| (n, Ty::List(Rc::new(entries[n].clone())))),
        );
        let taxonomies_ty = closed(
            "taxonomies",
            config.taxonomies.keys().map(|n| (n, taxonomy.clone())),
        );
        let kinds = ["entry", "list", "term", "index", "template", "feed"];
        let page = obj(
            "page",
            [
                ("url", Ty::Url, false),
                ("canonical", Ty::Url, false),
                (
                    "kind",
                    Ty::Enum(Rc::new(kinds.map(String::from).to_vec())),
                    false,
                ),
            ],
        );
        let globals = vec![
            ("site", obj("site", site_fields)),
            ("data", data),
            ("collections", collections_ty),
            ("taxonomies", taxonomies_ty),
            ("page", page),
        ];
        Schema {
            entries,
            term,
            taxonomy,
            paginator,
            collection: named("collection"),
            feed: named("feed"),
            globals,
        }
    }

    /// The types `props` and `vars` comments can name besides #18's.
    fn named(&self) -> BTreeMap<String, Ty> {
        let mut named: BTreeMap<String, Ty> = self
            .entries
            .iter()
            .map(|(c, ty)| (format!("entry<{c}>"), ty.clone()))
            .collect();
        named.insert("term".into(), self.term.clone());
        named.insert("paginator".into(), self.paginator.clone());
        named
    }

    /// What a Route's Page sees besides the globals: its kind, how a
    /// Diagnostic names it, and its values.
    fn route(
        &self,
        config: &Config,
        site: &Site,
        kind: &RouteKind,
    ) -> (&'static str, String, Vec<(&'static str, Ty)>) {
        let list = |ty: &Ty| Ty::List(Rc::new(ty.clone()));
        // A Taxonomy or feed over several Collections lists Entries of each.
        let of = |collections: &[String]| {
            let mut tys = collections.iter().map(|c| self.entries[c].clone());
            let first = tys.next().unwrap_or(Ty::Any);
            list(&tys.fold(first, |a, b| merge(&a, &b)))
        };
        match kind {
            RouteKind::Entry(i) => {
                let c = &site.entries[*i].collection;
                (
                    "entry",
                    format!("an Entry of `{c}`"),
                    vec![("entry", self.entries[c].clone())],
                )
            }
            RouteKind::List { collection, .. } => (
                "list",
                format!("the `{collection}` list"),
                vec![
                    ("collection", self.collection.clone()),
                    ("entries", list(&self.entries[collection])),
                    ("paginator", self.paginator.clone()),
                ],
            ),
            RouteKind::Term { taxonomy, .. } => (
                "term",
                format!("a Term of `{taxonomy}`"),
                vec![
                    ("taxonomy", self.taxonomy.clone()),
                    ("term", self.term.clone()),
                    ("entries", of(&config.taxonomies[taxonomy].collections)),
                    ("paginator", self.paginator.clone()),
                ],
            ),
            RouteKind::Terms { taxonomy } => (
                "index",
                format!("the `{taxonomy}` index"),
                vec![
                    ("taxonomy", self.taxonomy.clone()),
                    ("terms", list(&self.term)),
                ],
            ),
            RouteKind::Template(name) => ("template", format!("the Route `{name}`"), Vec::new()),
            RouteKind::Feed { name, .. } => (
                "feed",
                format!("the feed `{name}`"),
                vec![
                    ("feed", self.feed.clone()),
                    ("entries", of(&config.feeds[name].collections)),
                ],
            ),
        }
    }
}

/// A declared field, which a Template finds absent only when it has no
/// default.
fn declared(path: &str, f: &Declared) -> Field {
    Field {
        ty: field_ty(path, &f.kind),
        optional: f.optional && f.default.is_none(),
    }
}

fn field_ty(path: &str, kind: &FieldType) -> Ty {
    match kind {
        FieldType::String { .. } => Ty::Str,
        FieldType::Int { .. } => Ty::Int,
        FieldType::Float { .. } => Ty::Float,
        FieldType::Bool => Ty::Bool,
        FieldType::Date => Ty::Date,
        FieldType::Datetime => Ty::Datetime,
        FieldType::Url => Ty::Url,
        FieldType::Image => Ty::Image,
        FieldType::Enum(values) => Ty::Enum(Rc::new(values.clone())),
        FieldType::List { items, .. } => Ty::List(Rc::new(field_ty(path, items))),
        FieldType::Object(fields) => Ty::Obj(Rc::new(Obj {
            name: path.into(),
            fields: fields
                .iter()
                .map(|(k, f)| (k.clone(), declared(&format!("{path}.{k}"), f)))
                .collect(),
            open: false,
        })),
    }
}

/// The type of a Data file with no schema, from the values in it (#19).
fn infer(path: &str, v: &fields::Value) -> Ty {
    use fields::Value as V;
    match v {
        V::String(_) => Ty::Str,
        V::Url(_) => Ty::Url,
        V::Int(_) => Ty::Int,
        V::Float(_) => Ty::Float,
        V::Bool(_) => Ty::Bool,
        V::Date(_) => Ty::Date,
        V::Datetime(_) => Ty::Datetime,
        V::Image(_) => Ty::Image,
        V::List(items) => list_of(items.iter().map(|i| infer(&format!("{path}[]"), i))),
        V::Object(o) => closed(
            path,
            o.iter().map(|(k, v)| (k, infer(&format!("{path}.{k}"), v))),
        ),
    }
}

/// `[site.extra]`, from the values in it.
fn toml(path: &str, v: &toml::Value) -> Ty {
    match v {
        toml::Value::String(_) | toml::Value::Datetime(_) => Ty::Str,
        toml::Value::Integer(_) => Ty::Int,
        toml::Value::Float(_) => Ty::Float,
        toml::Value::Boolean(_) => Ty::Bool,
        toml::Value::Array(items) => list_of(items.iter().map(|i| toml(&format!("{path}[]"), i))),
        toml::Value::Table(t) => closed(
            path,
            t.iter().map(|(k, v)| (k, toml(&format!("{path}.{k}"), v))),
        ),
    }
}

fn list_of(items: impl Iterator<Item = Ty>) -> Ty {
    let item = items.reduce(|a, b| merge(&a, &b)).unwrap_or(Ty::Any);
    Ty::List(Rc::new(item))
}

fn closed<'k>(path: &str, fields: impl Iterator<Item = (&'k String, Ty)>) -> Ty {
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

/// The byte range an expression covers. minijinja starts some spans late,
/// such as an attribute's at its dot and a method call's at its name, and
/// an operator's early, so they are rebuilt from their parts.
fn range(e: &Expr<'_>) -> Range<usize> {
    let own = e.span().start_offset as usize..e.span().end_offset as usize;
    let from = |inner: &Expr<'_>| range(inner).start.min(own.start)..own.end;
    match e {
        Expr::BinOp(b) => range(&b.left).start..range(&b.right).end,
        Expr::Compare(c) => {
            let end = c
                .ops
                .last()
                .map_or(range(&c.expr).end, |o| range(&o.expr).end);
            range(&c.expr).start..end
        }
        Expr::GetAttr(g) => from(&g.expr),
        Expr::GetItem(g) => from(&g.expr),
        Expr::Call(c) => from(&c.expr),
        Expr::Test(t) => from(&t.expr),
        Expr::Filter(f) => match &f.expr {
            Some(inner) => from(inner),
            None => own,
        },
        Expr::UnaryOp(u) => own.start.min(range(&u.expr).start)..own.end.max(range(&u.expr).end),
        _ => own,
    }
}

/// `entry.hero.src` for an expression that is a plain path.
fn path_of(e: &Expr<'_>) -> Option<String> {
    match e {
        Expr::Var(v) => Some(v.id.to_string()),
        Expr::GetAttr(g) => path_of(&g.expr).map(|p| format!("{p}.{}", g.name)),
        Expr::GetItem(g) => {
            let key = g.subscript_expr.as_const()?;
            Some(format!("{}.{}", path_of(&g.expr)?, key.as_str()?))
        }
        _ => None,
    }
}

/// The paths a condition proves present when it is true.
fn narrows(e: &Expr<'_>) -> Vec<String> {
    match e {
        Expr::Var(_) | Expr::GetAttr(_) | Expr::GetItem(_) => path_of(e).into_iter().collect(),
        Expr::BinOp(b) if matches!(b.op, ast::BinOpKind::ScAnd) => {
            let mut paths = narrows(&b.left);
            paths.extend(narrows(&b.right));
            paths
        }
        Expr::Test(t) if t.name == "defined" => path_of(&t.expr).into_iter().collect(),
        Expr::UnaryOp(u) if matches!(u.op, ast::UnaryOpKind::Not) => narrows_false(&u.expr),
        _ => Vec::new(),
    }
}

/// The paths a condition proves present when it is false, for `else`.
fn narrows_false(e: &Expr<'_>) -> Vec<String> {
    match e {
        Expr::UnaryOp(u) if matches!(u.op, ast::UnaryOpKind::Not) => narrows(&u.expr),
        Expr::Test(t) if t.name == "undefined" => path_of(&t.expr).into_iter().collect(),
        Expr::BinOp(b) if matches!(b.op, ast::BinOpKind::ScOr) => {
            let mut paths = narrows_false(&b.left);
            paths.extend(narrows_false(&b.right));
            paths
        }
        _ => Vec::new(),
    }
}

fn const_ty(value: &minijinja::Value) -> Ty {
    match value.kind() {
        ValueKind::String => Ty::Str,
        ValueKind::Bool => Ty::Bool,
        ValueKind::Number if value.is_integer() => Ty::Int,
        ValueKind::Number => Ty::Float,
        ValueKind::None => Ty::None,
        _ => Ty::Any,
    }
}

/// A `{# props: … #}` or `{# vars: … #}` comment.
struct Header<'a> {
    /// The whole comment.
    at: Range<usize>,
    props: bool,
    /// Its entries, and where each starts in the file.
    body: &'a str,
    body_at: usize,
}

/// minijinja's lexer drops comments, so headers are found in the source.
fn headers(source: &str) -> Vec<Header<'_>> {
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(start) = source[at..].find("{#").map(|i| at + i) {
        let Some(end) = source[start..].find("#}").map(|i| start + i + 2) else {
            break;
        };
        let inner = &source[start + 2..end - 2];
        let trimmed = inner.trim_start_matches(['-', '+']).trim_start();
        for (props, keyword) in [(true, "props:"), (false, "vars:")] {
            if let Some(body) = trimmed.strip_prefix(keyword) {
                let body = body.trim_end_matches(['-', '+']);
                let body_at = start + 2 + (inner.len() - trimmed.len()) + keyword.len();
                out.push(Header {
                    at: start..end,
                    props,
                    body,
                    body_at,
                });
            }
        }
        at = end;
    }
    out
}

/// Splits a header at the commas outside `<>` and `()`, with the range of
/// each entry relative to the header, trimmed.
fn entries(body: &str) -> Vec<(Range<usize>, &str)> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0i32, 0);
    for (i, c) in body.char_indices().chain([(body.len(), ',')]) {
        match c {
            '<' | '(' => depth += 1,
            '>' | ')' => depth -= 1,
            ',' if depth <= 0 => {
                let part = &body[start..i];
                let lead = part.len() - part.trim_start().len();
                let text = part.trim();
                if !text.is_empty() {
                    out.push((start + lead..start + lead + text.len(), text));
                }
                start = i + 1;
            }
            _ => {}
        }
    }
    out
}

/// Whether a macro takes a `{% call %}` body: it reads `caller`, and
/// optionally if it asks `caller is defined` or tests `if caller` first.
fn slot(source: &str) -> Slot {
    let tokens: Vec<Token<'_>> = tokenize(source, false, SyntaxConfig::default())
        .map_while(Result::ok)
        .map(|(t, _)| t)
        .collect();
    let caller = |t: &Token<'_>| matches!(t, Token::Ident("caller"));
    if !tokens.iter().any(caller) {
        return Slot::No;
    }
    let guarded = tokens.windows(3).any(|w| {
        (caller(&w[1]) && matches!(w[0], Token::Ident("if")))
            || (caller(&w[0]) && matches!(w[1], Token::Ident("is")))
    });
    match guarded {
        true => Slot::Optional,
        false => Slot::Required,
    }
}

/// One `(Template, kind of Route)` to check, and what its Pages see.
struct Group {
    template: String,
    kind: &'static str,
    labels: Vec<String>,
    vars: Vec<(&'static str, Ty)>,
}

/// Checks every Template. `chosen` is each Route's Template, in the order
/// of `site.routes`. Only Templates that parse are checked: `render`
/// reports the others.
pub(super) fn check(
    env: &Environment<'_>,
    config: &Config,
    site: &Site,
    files: &Files,
    templates: &BTreeMap<String, FileId>,
    chosen: &[(String, Span)],
) -> Vec<Diagnostic> {
    let syntax = env.syntax().clone();
    let asts: BTreeMap<&str, Stmt<'_>> = templates
        .iter()
        .filter_map(|(name, &file)| {
            let ast = parse(files.text(file), name, syntax.clone()).ok()?;
            Some((name.as_str(), ast))
        })
        .collect();
    let is_filter = env
        .compile_expression("n is filter")
        .expect("a valid expression");
    let is_test = env
        .compile_expression("n is test")
        .expect("a valid expression");
    let known = |test: bool, name: &str| {
        let e = if test { &is_test } else { &is_filter };
        e.eval(context! { n => name }).is_ok_and(|v| v.is_true())
    };
    let schema = Schema::new(config, site);
    let mut c = Checker {
        files,
        templates,
        asts: &asts,
        known: &known,
        functions: env.globals().map(|(n, _)| n.to_string()).collect(),
        named: schema.named(),
        globals: schema.globals.clone(),
        macros: BTreeMap::new(),
        vars: BTreeMap::new(),
        out: Vec::new(),
        stack: Vec::new(),
        scopes: Vec::new(),
        narrowed: Vec::new(),
        component: None,
        overrides: Vec::new(),
        active: Vec::new(),
        seen_blocks: BTreeSet::new(),
        chain_broken: false,
        route: String::new(),
    };
    c.signatures();
    c.components();

    let mut groups: Vec<Group> = Vec::new();
    for (route, (template, _)) in site.routes.iter().zip(chosen) {
        // The built-in feed Template is the Engine's, and a missing one is
        // reported by `render`.
        if !asts.contains_key(template.as_str()) {
            continue;
        }
        let (kind, label, vars) = schema.route(config, site, &route.kind);
        match groups
            .iter_mut()
            .find(|g| g.template == *template && g.kind == kind)
        {
            Some(g) => {
                if !g.labels.contains(&label) {
                    g.labels.push(label);
                }
                for ((_, a), (_, b)) in g.vars.iter_mut().zip(vars) {
                    *a = merge(a, &b);
                }
            }
            None => groups.push(Group {
                template: template.clone(),
                kind,
                labels: vec![label],
                vars,
            }),
        }
    }
    for g in &groups {
        c.route(g);
    }
    let mut out = c.out;
    out.sort_by(|a, b| {
        let key = |d: &Diagnostic| {
            (
                files.path(d.primary().file).to_string(),
                d.primary().range.start,
            )
        };
        key(a).cmp(&key(b))
    });
    out
}

struct Checker<'a> {
    files: &'a Files,
    templates: &'a BTreeMap<String, FileId>,
    asts: &'a BTreeMap<&'a str, Stmt<'a>>,
    /// Whether the Environment has a filter, or with `true` a test.
    known: &'a dyn Fn(bool, &str) -> bool,
    functions: BTreeSet<String>,
    named: BTreeMap<String, Ty>,
    globals: Vec<(&'static str, Ty)>,
    /// Each Template's top-level macros.
    macros: BTreeMap<&'a str, BTreeMap<&'a str, Rc<Sig>>>,
    /// Each Layout's vars comment.
    vars: BTreeMap<&'a str, Vec<Prop>>,
    out: Vec<Diagnostic>,

    /// The Templates being walked, innermost last.
    stack: Vec<&'a str>,
    scopes: Vec<BTreeMap<String, Bind>>,
    /// Paths a guard proves present here.
    narrowed: Vec<String>,
    /// The Component whose body is being walked.
    component: Option<Rc<Sig>>,
    /// Blocks the Templates being walked override, by their Template.
    overrides: Vec<(&'a str, &'a Spanned<ast::Block<'a>>)>,
    /// The overrides being walked, so a block can't walk itself.
    active: Vec<*const ast::Block<'a>>,
    seen_blocks: BTreeSet<&'a str>,
    /// An `extends` that couldn't be followed, so overrides can't be judged.
    chain_broken: bool,
    /// What the walk is checking, such as "an Entry of `posts`".
    route: String,
}

impl<'a> Checker<'a> {
    fn file(&self) -> FileId {
        self.templates[*self.stack.last().expect("inside a Template")]
    }

    fn source(&self) -> &'a str {
        self.files.text(self.file())
    }

    fn span(&self, range: Range<usize>) -> Span {
        Span {
            file: self.file(),
            range,
        }
    }

    fn text(&self, range: Range<usize>) -> &'a str {
        self.source().get(range).unwrap_or_default()
    }

    /// A Diagnostic in the Template being walked.
    fn error(&self, code: &'static Code, message: String, range: Range<usize>) -> Diagnostic {
        self.error_at(code, message, self.span(range))
    }

    fn error_at(&self, code: &'static Code, message: String, span: Span) -> Diagnostic {
        let d = Diagnostic::error(code, message, span);
        match self.route.is_empty() {
            true => d,
            false => d.note(format!("checked as the Template of {}", self.route)),
        }
    }

    /// One Diagnostic for each mistake, though many Routes reach it.
    fn push(&mut self, d: Diagnostic) {
        let key = |d: &Diagnostic| (d.code.name, d.message.clone(), d.primary().clone());
        if !self.out.iter().any(|seen| key(seen) == key(&d)) {
            self.out.push(d);
        }
    }

    /// Adds "did you mean" to `d`, as an edit of `span`.
    fn near<'c>(
        d: Diagnostic,
        span: Span,
        name: &str,
        candidates: impl IntoIterator<Item = &'c str>,
    ) -> Diagnostic {
        match similar(name, candidates) {
            Some(near) => {
                let edit = Edit {
                    span,
                    replacement: near.to_string(),
                };
                d.suggest(
                    format!("did you mean `{near}`?"),
                    Applicability::MaybeIncorrect,
                    vec![edit],
                )
            }
            None => d,
        }
    }

    /// The range of `name` at or after `from`, for the names minijinja
    /// gives no span: a kwarg's, a block's, a macro's.
    fn find(&self, from: usize, name: &str) -> Range<usize> {
        match self.source().get(from..).and_then(|s| s.find(name)) {
            Some(i) => from + i..from + i + name.len(),
            None => from..from,
        }
    }

    // Pass 1: every macro's signature and props comment, and every Layout's
    // vars comment.

    fn signatures(&mut self) {
        for (&name, ast) in self.asts {
            let Stmt::Template(t) = ast else { continue };
            self.stack = vec![name];
            let source = self.source();
            let headers = headers(source);
            let mut paired = vec![false; headers.len()];
            for (i, h) in headers.iter().enumerate() {
                if !h.props {
                    paired[i] = true;
                    let vars = self.header(h);
                    self.vars.insert(name, vars);
                }
            }
            let mut sigs = BTreeMap::new();
            for s in &t.children {
                let Stmt::Macro(m) = s else { continue };
                let start = m.span().start_offset as usize;
                // Only the tag's opening may come between the comment and
                // `macro`.
                let above = headers.iter().position(|h| {
                    h.props
                        && h.at.end <= start
                        && source[h.at.end..start]
                            .chars()
                            .all(|c| c.is_whitespace() || "{%-+".contains(c))
                });
                let props = above.map(|i| {
                    paired[i] = true;
                    self.header(&headers[i])
                });
                let sig = self.signature(name, m, props);
                sigs.insert(m.name, Rc::new(sig));
            }
            for (h, paired) in headers.iter().zip(paired) {
                if !paired {
                    let d = self
                        .error(
                            &codes::TEMPLATE_BAD_HEADER,
                            "this props comment isn't directly above a `{% macro %}`".into(),
                            h.at.clone(),
                        )
                        .help("write it on the line before the macro it declares, with nothing between them");
                    self.push(d);
                }
            }
            self.macros.insert(name, sigs);
        }
        self.stack.clear();
    }

    fn header(&mut self, h: &Header<'_>) -> Vec<Prop> {
        let kind = if h.props { "props" } else { "vars" };
        let mut out = Vec::new();
        for (at, entry) in entries(h.body) {
            let at = h.body_at + at.start..h.body_at + at.end;
            let Some((name, ty)) = entry.split_once(':') else {
                let d = self.error(
                    &codes::TEMPLATE_BAD_HEADER,
                    format!("`{entry}` in a {kind} comment isn't `name: type`"),
                    at,
                );
                self.push(d.help("write `name: type`, or `name?: type` for an optional one"));
                continue;
            };
            let (name, optional) = match name.trim().strip_suffix('?') {
                Some(name) => (name.trim(), true),
                None => (name.trim(), false),
            };
            let ty_at = at.end - ty.len()..at.end;
            let ty = match self.parse_ty(ty.trim()) {
                Ok(ty) => ty,
                Err(unknown) => {
                    let ty_at = self.find(ty_at.start, &unknown);
                    let d = self.error(
                        &codes::TEMPLATE_BAD_HEADER,
                        format!("there is no type `{unknown}`, in the {kind} comment's `{name}`"),
                        ty_at.clone(),
                    );
                    let candidates: Vec<&str> = TYPES
                        .iter()
                        .copied()
                        .chain(self.named.keys().map(String::as_str))
                        .collect();
                    let d = Self::near(d, self.span(ty_at), &unknown, candidates);
                    self.push(d);
                    Ty::Any
                }
            };
            out.push(Prop {
                name: name.to_string(),
                ty,
                optional,
                at,
            });
        }
        out
    }

    /// A type as a header writes it; the error is the unknown part.
    fn parse_ty(&self, s: &str) -> Result<Ty, String> {
        Ok(match s {
            "string" => Ty::Str,
            "int" => Ty::Int,
            "float" => Ty::Float,
            "bool" => Ty::Bool,
            "date" => Ty::Date,
            "datetime" => Ty::Datetime,
            "url" => Ty::Url,
            "image" => Ty::Image,
            "slot" => Ty::Slot,
            "attrs" => Ty::Attrs,
            "object" => Ty::Any,
            _ => {
                if let Some(item) = s.strip_prefix("list<").and_then(|r| r.strip_suffix('>')) {
                    Ty::List(Rc::new(self.parse_ty(item.trim())?))
                } else if let Some(values) =
                    s.strip_prefix("enum(").and_then(|r| r.strip_suffix(')'))
                {
                    Ty::Enum(Rc::new(
                        values.split('|').map(|v| v.trim().to_string()).collect(),
                    ))
                } else if let Some(ty) = self.named.get(s) {
                    ty.clone()
                } else {
                    return Err(s.to_string());
                }
            }
        })
    }

    fn signature(
        &mut self,
        file: &'a str,
        m: &'a Spanned<ast::Macro<'a>>,
        props: Option<Vec<Prop>>,
    ) -> Sig {
        let first_default = m.args.len() - m.defaults.len();
        let params: Vec<Param> = m
            .args
            .iter()
            .enumerate()
            .map(|(i, a)| Param {
                name: match a {
                    Expr::Var(v) => v.id.to_string(),
                    _ => String::new(),
                },
                has_default: i >= first_default,
                none_default: i >= first_default
                    && m.defaults[i - first_default]
                        .as_const()
                        .is_some_and(|v| v.is_none()),
            })
            .collect();
        let start = m.span().start_offset as usize;
        let name_at = self.find(start + "macro".len(), m.name);
        match &props {
            None if file.starts_with("components/") => {
                let d = self
                    .error(
                        &codes::TEMPLATE_BAD_HEADER,
                        format!("the Component `{}` has no props comment", m.name),
                        name_at,
                    )
                    .help("declare its props directly above it, such as `{# props: title: string, href?: url #}`");
                self.push(d);
            }
            None => {}
            Some(props) => {
                for (p, arg) in params.iter().zip(&m.args) {
                    let at = range(arg);
                    match props.iter().find(|x| x.name == p.name) {
                        // A misspelt entry gets its fix on the comment's side.
                        None => {
                            let d = self.error(
                                &codes::TEMPLATE_BAD_HEADER,
                                format!(
                                    "the props comment doesn't declare `{}`, a parameter of `{}`",
                                    p.name, m.name
                                ),
                                at,
                            );
                            self.push(d);
                        }
                        Some(x) if !x.optional && p.has_default => {
                            let d = self
                                .error(
                                    &codes::TEMPLATE_BAD_HEADER,
                                    format!(
                                        "`{}` gives the required prop `{}` a default",
                                        m.name, p.name
                                    ),
                                    at,
                                )
                                .help(format!(
                                    "mark it optional in the props comment: `{}?: {}`",
                                    p.name, x.ty
                                ));
                            self.push(d);
                        }
                        Some(_) => {}
                    }
                }
                for x in props {
                    if !params.iter().any(|p| p.name == x.name) {
                        let at = x.at.start..x.at.start + x.name.len();
                        let d = self.error(
                            &codes::TEMPLATE_BAD_HEADER,
                            format!(
                                "the props comment declares `{}`, which `{}` has no parameter for",
                                x.name, m.name
                            ),
                            at.clone(),
                        );
                        let d = Self::near(
                            d,
                            self.span(at),
                            &x.name,
                            params.iter().map(|p| p.name.as_str()),
                        );
                        self.push(d);
                    }
                }
            }
        }
        let body = &self.source()[start..m.span().end_offset as usize];
        Sig {
            name: m.name.to_string(),
            file: file.to_string(),
            params,
            props,
            slot: slot(body),
        }
    }

    // Pass 2: each Component's body, which sees only its props and the
    // globals.

    fn components(&mut self) {
        let files: Vec<&'a str> = self
            .macros
            .keys()
            .copied()
            .filter(|f| f.starts_with("components/"))
            .collect();
        for file in files {
            let Some(Stmt::Template(t)) = self.asts.get(file) else {
                continue;
            };
            self.reset();
            self.stack.push(file);
            self.scopes.push(BTreeMap::new());
            for s in &t.children {
                match s {
                    Stmt::Import(_) | Stmt::FromImport(_) | Stmt::Set(_) | Stmt::SetBlock(_) => {
                        self.stmt(s)
                    }
                    Stmt::Macro(m) => {
                        let sig = self.macros[file][m.name].clone();
                        self.bind(m.name, Bind::new(Ty::Macro(sig), false));
                    }
                    _ => {}
                }
            }
            for s in &t.children {
                if let Stmt::Macro(m) = s {
                    let sig = self.macros[file][m.name].clone();
                    self.component = Some(sig.clone());
                    self.macro_body(m, &sig);
                    self.component = None;
                }
            }
        }
    }

    fn macro_body(&mut self, m: &'a Spanned<ast::Macro<'a>>, sig: &Sig) {
        for d in &m.defaults {
            self.expr(d, Mode::Pass);
        }
        self.scopes.push(BTreeMap::new());
        for p in &sig.params {
            let prop = sig.props.iter().flatten().find(|x| x.name == p.name);
            let (ty, optional) = match prop {
                Some(x) => (
                    x.ty.clone(),
                    x.optional && (!p.has_default || p.none_default),
                ),
                None => (Ty::Any, false),
            };
            self.bind(&p.name, Bind::new(ty, optional));
        }
        self.bind("caller", Bind::new(Ty::Any, false));
        self.body(&m.body);
        self.scopes.pop();
    }

    // Pass 3: each Route's Template, with what its Pages see.

    fn reset(&mut self) {
        self.stack.clear();
        self.narrowed.clear();
        self.overrides.clear();
        self.active.clear();
        self.seen_blocks.clear();
        self.chain_broken = false;
        self.component = None;
        self.route.clear();
        let globals = self
            .globals
            .iter()
            .map(|(n, ty)| (n.to_string(), Bind::new(ty.clone(), false)))
            .collect();
        self.scopes = vec![globals];
    }

    fn route(&mut self, g: &Group) {
        self.reset();
        self.route = g.labels.join(" or ");
        let vars = g
            .vars
            .iter()
            .map(|(n, ty)| (n.to_string(), Bind::new(ty.clone(), false)))
            .collect();
        self.scopes.push(vars);
        self.scopes.push(BTreeMap::new());
        let template = self
            .asts
            .get_key_value(g.template.as_str())
            .expect("checked above")
            .0;
        self.walk(template, None);
        if self.chain_broken {
            return;
        }
        let overrides = std::mem::take(&mut self.overrides);
        for (file, b) in overrides {
            if self.seen_blocks.contains(b.name) {
                continue;
            }
            self.stack = vec![file];
            let at = self.find(b.span().start_offset as usize + "block".len(), b.name);
            let d = self.error(
                &codes::TEMPLATE_UNKNOWN_BLOCK,
                format!("no Layout `{file}` extends has a block `{}`", b.name),
                at.clone(),
            );
            let d = Self::near(d, self.span(at), b.name, self.seen_blocks.iter().copied());
            self.push(d);
        }
    }

    /// Walks a Template in the current scopes. `extended` is where the
    /// Template that extends it names it.
    fn walk(&mut self, name: &'a str, extended: Option<Span>) {
        let Some(Stmt::Template(t)) = self.asts.get(name) else {
            return;
        };
        // An include cycle renders forever; twice is enough to check it.
        if self.stack.iter().filter(|f| **f == name).count() >= 2 {
            return;
        }
        self.stack.push(name);
        let extends = t.children.iter().find_map(|s| match s {
            Stmt::Extends(e) => Some(e),
            _ => None,
        });
        match extends {
            Some(e) => {
                for s in &t.children {
                    match s {
                        Stmt::Block(b) => self.overrides.push((name, b)),
                        Stmt::Extends(_) | Stmt::EmitRaw(_) => {}
                        other => self.stmt(other),
                    }
                }
                match self.template_names(&e.name, "extends").first() {
                    Some(&parent) if self.asts.contains_key(parent) => {
                        let at = self.span(range(&e.name));
                        self.walk(parent, Some(at));
                    }
                    _ => self.chain_broken = true,
                }
            }
            None => {
                if let Some(vars) = self.vars.get(name).cloned() {
                    self.layout_vars(name, &vars, extended);
                }
                self.body(&t.children);
            }
        }
        self.stack.pop();
    }

    /// A Layout's vars, against what the Templates extending it set.
    fn layout_vars(&mut self, layout: &str, vars: &[Prop], extended: Option<Span>) {
        for v in vars {
            let set = self.scopes[1..]
                .iter()
                .rev()
                .find_map(|s| s.get(&v.name))
                .cloned();
            let declared = self.span(v.at.clone());
            match set {
                Some(b) => {
                    let at =
                        b.at.clone()
                            .or(extended.clone())
                            .unwrap_or(declared.clone());
                    let wrong = if !assignable(&b.ty, &v.ty) {
                        Some(format!(
                            "the Layout `{layout}` takes `{}` of type `{}`, not `{}`",
                            v.name, v.ty, b.ty
                        ))
                    } else if b.optional && !v.optional {
                        Some(format!(
                            "the Layout `{layout}` requires `{}`, but it is set to an optional value",
                            v.name
                        ))
                    } else {
                        None
                    };
                    if let Some(message) = wrong {
                        let d = self
                            .error_at(&codes::TEMPLATE_BAD_VAR, message, at)
                            .secondary(declared, "declared here");
                        self.push(d);
                        continue;
                    }
                    if let (Ty::Enum(values), Some(literal)) = (&v.ty, &b.literal)
                        && !values.contains(literal)
                    {
                        let d = self
                            .error_at(
                                &codes::TEMPLATE_BAD_VAR,
                                format!(
                                    "the Layout `{layout}` takes `{}` as one of {}, not \"{literal}\"",
                                    v.name,
                                    values.iter().map(|v| format!("\"{v}\"")).collect::<Vec<_>>().join(", ")
                                ),
                                at.clone(),
                            )
                            .secondary(declared, "declared here");
                        let d = match similar(literal, values.iter().map(String::as_str)) {
                            Some(near) => {
                                let quote = self
                                    .files
                                    .text(at.file)
                                    .get(at.range.start..at.range.start + 1)
                                    .unwrap_or("\"");
                                d.suggest(
                                    format!("did you mean \"{near}\"?"),
                                    Applicability::MaybeIncorrect,
                                    vec![Edit {
                                        span: at,
                                        replacement: format!("{quote}{near}{quote}"),
                                    }],
                                )
                            }
                            None => d,
                        };
                        self.push(d);
                    }
                }
                None if v.optional => self.bind(&v.name, Bind::new(v.ty.clone(), true)),
                None => {
                    let message = format!(
                        "the Layout `{layout}` requires `{}`, which is never set",
                        v.name
                    );
                    let d = match &extended {
                        Some(at) => self
                            .error_at(&codes::TEMPLATE_BAD_VAR, message, at.clone())
                            .secondary(declared, "declared here"),
                        None => self.error_at(&codes::TEMPLATE_BAD_VAR, message, declared),
                    };
                    let d = d.help(format!(
                        "set it before the first block: `{{% set {} = … %}}`",
                        v.name
                    ));
                    self.push(d);
                    self.bind(&v.name, Bind::new(v.ty.clone(), false));
                }
            }
        }
    }

    /// The Templates an `extends`, `include` or `import` names, which must
    /// be plain strings (#19) that exist.
    fn template_names(&mut self, e: &'a Expr<'a>, tag: &str) -> Vec<&'a str> {
        let names: Vec<(&'a str, Range<usize>)> = match e {
            Expr::Const(c) if c.value.as_str().is_some() => vec![(self.text(range(e)), range(e))],
            Expr::List(l)
                if l.items
                    .iter()
                    .all(|i| matches!(i, Expr::Const(c) if c.value.as_str().is_some())) =>
            {
                l.items
                    .iter()
                    .map(|i| (self.text(range(i)), range(i)))
                    .collect()
            }
            _ => {
                let d = self
                    .error(
                        &codes::TEMPLATE_NON_CONSTANT_NAME,
                        format!("`{tag}` names its Template with an expression"),
                        range(e),
                    )
                    .help("name it by its path from `templates/` as a plain string, such as \"partials/nav.html\", and choose between several with `{% if %}`");
                self.push(d);
                self.expr(e, Mode::Use);
                return Vec::new();
            }
        };
        // The quoted literal's text, without its quotes.
        let names: Vec<(&'a str, Range<usize>)> = names
            .into_iter()
            .map(|(text, at)| {
                (
                    text.get(1..text.len().saturating_sub(1))
                        .unwrap_or_default(),
                    at,
                )
            })
            .collect();
        if let Some((name, _)) = names.iter().find(|(n, _)| self.templates.contains_key(*n)) {
            return vec![*name];
        }
        let Some((name, at)) = names.first().cloned() else {
            return Vec::new();
        };
        let d = self.error(
            &codes::TEMPLATE_NOT_FOUND,
            format!("the Template `{name}` doesn't exist"),
            at.clone(),
        );
        let near = similar(name, self.templates.keys().map(String::as_str));
        let d = match near {
            Some(near) => {
                let quote = self.text(at.start..at.start + 1);
                let edit = Edit {
                    span: self.span(at),
                    replacement: format!("{quote}{near}{quote}"),
                };
                d.suggest(
                    format!("did you mean `{near}`?"),
                    Applicability::MaybeIncorrect,
                    vec![edit],
                )
            }
            None => d.help(format!("create `templates/{name}`")),
        };
        self.push(d);
        Vec::new()
    }

    fn bind(&mut self, name: &str, b: Bind) {
        self.narrowed
            .retain(|p| p != name && !p.starts_with(&format!("{name}.")));
        self.scopes
            .last_mut()
            .expect("a scope")
            .insert(name.to_string(), b);
    }

    fn lookup(&self, name: &str) -> Option<&Bind> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    fn bind_target(&mut self, target: &Expr<'a>, b: Bind) {
        match target {
            Expr::Var(v) => self.bind(v.id, b),
            Expr::List(l) => {
                for i in &l.items {
                    self.bind_target(i, Bind::new(Ty::Any, false));
                }
            }
            Expr::Tuple(t) => {
                for i in &t.items {
                    self.bind_target(i, Bind::new(Ty::Any, false));
                }
            }
            _ => {}
        }
    }

    fn body(&mut self, stmts: &'a [Stmt<'a>]) {
        for s in stmts {
            self.stmt(s);
        }
    }

    /// Walks a body with `paths` proven present.
    fn narrowed_body(&mut self, paths: Vec<String>, stmts: &'a [Stmt<'a>]) {
        let mark = self.narrowed.len();
        self.narrowed.extend(paths);
        self.body(stmts);
        self.narrowed.truncate(mark);
    }

    fn stmt(&mut self, s: &'a Stmt<'a>) {
        match s {
            Stmt::Template(t) => self.body(&t.children),
            Stmt::EmitRaw(_) | Stmt::Continue(_) | Stmt::Break(_) | Stmt::Extends(_) => {}
            Stmt::EmitExpr(e) => {
                self.expr(&e.expr, Mode::Use);
            }
            Stmt::ForLoop(f) => {
                let (iter, _) = self.expr(&f.iter, Mode::Use);
                let item = match iter {
                    Ty::List(item) => (*item).clone(),
                    // A map loops over its keys.
                    Ty::Obj(_) | Ty::Attrs => Ty::Str,
                    Ty::Any => Ty::Any,
                    other => {
                        let at = range(&f.iter);
                        let what =
                            path_of(&f.iter).unwrap_or_else(|| self.text(at.clone()).to_string());
                        let d = self.error(
                            &codes::TEMPLATE_WRONG_TYPE,
                            format!("`{what}` has type `{other}`, which can't be looped over"),
                            at,
                        );
                        self.push(d);
                        Ty::Any
                    }
                };
                self.scopes.push(BTreeMap::new());
                self.bind_target(&f.target, Bind::new(item, false));
                self.bind("loop", Bind::new(Ty::Any, false));
                let mut paths = Vec::new();
                if let Some(filter) = &f.filter_expr {
                    paths = self.cond(filter);
                }
                self.narrowed_body(paths, &f.body);
                self.scopes.pop();
                self.body(&f.else_body);
            }
            Stmt::IfCond(i) => {
                let paths = self.cond(&i.expr);
                self.narrowed_body(paths, &i.true_body);
                self.narrowed_body(narrows_false(&i.expr), &i.false_body);
            }
            Stmt::WithBlock(w) => {
                self.scopes.push(BTreeMap::new());
                for (target, value) in &w.assignments {
                    let (ty, optional) = self.expr(value, Mode::Pass);
                    self.bind_target(target, Bind::new(ty, optional));
                }
                self.body(&w.body);
                self.scopes.pop();
            }
            Stmt::Set(set) => {
                let (ty, optional) = self.expr(&set.expr, Mode::Pass);
                let literal = set
                    .expr
                    .as_const()
                    .and_then(|v| v.as_str().map(String::from));
                let at = Some(self.span(range(&set.expr)));
                self.bind_target(
                    &set.target,
                    Bind {
                        ty,
                        optional,
                        literal,
                        at,
                    },
                );
            }
            Stmt::SetBlock(set) => {
                if let Some(f) = &set.filter {
                    self.expr(f, Mode::Use);
                }
                self.body(&set.body);
                self.bind_target(&set.target, Bind::new(Ty::Slot, false));
            }
            Stmt::AutoEscape(a) => {
                self.expr(&a.enabled, Mode::Use);
                self.body(&a.body);
            }
            Stmt::FilterBlock(f) => {
                self.expr(&f.filter, Mode::Use);
                self.body(&f.body);
            }
            Stmt::Block(b) => self.block(b),
            Stmt::Include(i) => {
                let names = match i.ignore_missing {
                    // `ignore missing` may name what doesn't exist.
                    true => match &i.name {
                        Expr::Const(c) => c
                            .value
                            .as_str()
                            .filter(|n| self.templates.contains_key(*n))
                            .into_iter()
                            .collect(),
                        _ => self.template_names(&i.name, "include"),
                    },
                    false => self.template_names(&i.name, "include"),
                };
                for name in names {
                    let name = self.asts.get_key_value(name).map(|(k, _)| *k);
                    if let Some(name) = name {
                        // A Partial sees what the including Template sees.
                        self.scopes.push(BTreeMap::new());
                        self.walk(name, None);
                        self.scopes.pop();
                    }
                }
            }
            Stmt::Import(i) => {
                let ty = match self.template_names(&i.expr, "import").first() {
                    Some(name) => Ty::Module(Rc::from(*name)),
                    None => Ty::Any,
                };
                self.bind_target(&i.name, Bind::new(ty, false));
            }
            Stmt::FromImport(i) => {
                let file = self.template_names(&i.expr, "import").first().copied();
                for (name, alias) in &i.names {
                    let Expr::Var(v) = name else { continue };
                    let target = alias.as_ref().unwrap_or(name);
                    let sigs = file.and_then(|f| self.macros.get(f));
                    let ty = match sigs.map(|m| m.get(v.id)) {
                        Some(Some(sig)) => Ty::Macro(sig.clone()),
                        Some(None) => {
                            let file = file.expect("its macros were found");
                            let at = range(name);
                            let d = self.error(
                                &codes::TEMPLATE_UNKNOWN_FILTER,
                                format!("`{file}` has no macro `{}`", v.id),
                                at.clone(),
                            );
                            let candidates: Vec<&str> = self.macros[file].keys().copied().collect();
                            let d = Self::near(d, self.span(at), v.id, candidates);
                            self.push(d);
                            Ty::Any
                        }
                        // A Template that didn't parse; `render` reports it.
                        None => Ty::Any,
                    };
                    self.bind_target(target, Bind::new(ty, false));
                }
            }
            Stmt::Macro(m) => {
                let file = *self.stack.last().expect("inside a Template");
                let sig = self.macros.get(file).and_then(|s| s.get(m.name)).cloned();
                match sig {
                    Some(sig) => {
                        self.bind(m.name, Bind::new(Ty::Macro(sig.clone()), false));
                        // A Component's body is checked once, on its own; a
                        // local macro's in the Template that defines it.
                        if !file.starts_with("components/") {
                            self.macro_body(m, &sig);
                        }
                    }
                    None => self.bind(m.name, Bind::new(Ty::Any, false)),
                }
            }
            Stmt::CallBlock(c) => {
                self.call(&c.call, true);
                self.scopes.push(BTreeMap::new());
                for a in &c.macro_decl.args {
                    self.bind_target(a, Bind::new(Ty::Any, false));
                }
                self.body(&c.macro_decl.body);
                self.scopes.pop();
            }
            Stmt::Do(d) => {
                self.call(&d.call, false);
            }
        }
    }

    /// A block: its own body, then each override of it in the Templates
    /// that extend this one, in theirs.
    fn block(&mut self, b: &'a Spanned<ast::Block<'a>>) {
        self.seen_blocks.insert(b.name);
        self.scopes.push(BTreeMap::new());
        self.body(&b.body);
        self.scopes.pop();
        let overrides: Vec<(&'a str, &'a Spanned<ast::Block<'a>>)> = self
            .overrides
            .iter()
            .filter(|(_, o)| o.name == b.name && !std::ptr::eq::<ast::Block<'a>>(&***o, &**b))
            .copied()
            .collect();
        for (file, o) in overrides {
            let ptr: *const ast::Block<'a> = &**o;
            if self.active.contains(&ptr) {
                continue;
            }
            self.active.push(ptr);
            self.stack.push(file);
            self.scopes.push(BTreeMap::new());
            self.body(&o.body);
            self.scopes.pop();
            self.stack.pop();
            self.active.pop();
        }
    }

    /// Checks a condition, and returns the paths it proves present.
    fn cond(&mut self, e: &'a Expr<'a>) -> Vec<String> {
        self.expr(e, Mode::Guard);
        narrows(e)
    }

    fn is_narrowed(&self, e: &Expr<'_>) -> bool {
        path_of(e).is_some_and(|p| self.narrowed.contains(&p))
    }

    /// An expression's type, and whether it may still be absent, having
    /// reported what `mode` doesn't allow of an absent value.
    fn expr(&mut self, e: &'a Expr<'a>, mode: Mode) -> (Ty, bool) {
        let (ty, optional) = self.infer(e);
        if optional && matches!(mode, Mode::Use | Mode::Compare) {
            let at = range(e);
            let what = path_of(e).unwrap_or_else(|| self.text(at.clone()).to_string());
            let d = match mode {
                Mode::Compare => self
                    .error(
                        &codes::TEMPLATE_COMPARE_OPTIONAL,
                        format!("`{what}` is optional, and comparing an absent value is an error"),
                        at,
                    )
                    .help(format!("guard it first, `{{% if {what} and … %}}`, or compare `{what} | default(…)`")),
                _ => self
                    .error(
                        &codes::TEMPLATE_UNGUARDED_OPTIONAL,
                        format!("`{what}` is optional, so it may be absent here"),
                        at,
                    )
                    .help(format!("guard it with `{{% if {what} %}}`, or give it a value with `| default(…)`")),
            };
            self.push(d);
        }
        (ty, optional && matches!(mode, Mode::Guard | Mode::Pass))
    }

    fn infer(&mut self, e: &'a Expr<'a>) -> (Ty, bool) {
        match e {
            Expr::Const(c) => (const_ty(&c.value), false),
            Expr::Var(v) => self.var(v),
            Expr::GetAttr(g) => {
                let (base, optional) = self.infer(&g.expr);
                if optional {
                    self.reach_through(&g.expr, e);
                }
                let end = g.span().end_offset as usize;
                self.field(
                    e,
                    &g.expr,
                    &base,
                    g.name,
                    end.saturating_sub(g.name.len())..end,
                )
            }
            Expr::GetItem(g) => {
                let (base, optional) = self.infer(&g.expr);
                if optional {
                    self.reach_through(&g.expr, e);
                }
                match g
                    .subscript_expr
                    .as_const()
                    .filter(|k| k.kind() == ValueKind::String)
                {
                    Some(key) => {
                        let key = key.as_str().unwrap_or_default().to_string();
                        self.field(e, &g.expr, &base, &key, range(&g.subscript_expr))
                    }
                    None => {
                        self.expr(&g.subscript_expr, Mode::Use);
                        match base {
                            Ty::List(item) => ((*item).clone(), false),
                            _ => (Ty::Any, false),
                        }
                    }
                }
            }
            Expr::Slice(s) => {
                let (ty, _) = self.expr(&s.expr, Mode::Use);
                for part in [&s.start, &s.stop, &s.step].into_iter().flatten() {
                    self.expr(part, Mode::Use);
                }
                (ty, false)
            }
            Expr::UnaryOp(u) => match u.op {
                ast::UnaryOpKind::Not => {
                    self.expr(&u.expr, Mode::Guard);
                    (Ty::Bool, false)
                }
                ast::UnaryOpKind::Neg => self.expr(&u.expr, Mode::Use),
            },
            Expr::BinOp(b) => {
                use ast::BinOpKind as Op;
                match b.op {
                    Op::ScAnd => {
                        let paths = self.cond(&b.left);
                        let mark = self.narrowed.len();
                        self.narrowed.extend(paths);
                        let (ty, _) = self.expr(&b.right, Mode::Guard);
                        self.narrowed.truncate(mark);
                        (ty, false)
                    }
                    Op::ScOr => {
                        self.expr(&b.left, Mode::Guard);
                        self.infer(&b.right)
                    }
                    Op::Concat => {
                        self.expr(&b.left, Mode::Use);
                        self.expr(&b.right, Mode::Use);
                        (Ty::Str, false)
                    }
                    Op::Eq | Op::Ne | Op::Lt | Op::Lte | Op::Gt | Op::Gte | Op::In => {
                        self.expr(&b.left, Mode::Compare);
                        self.expr(&b.right, Mode::Compare);
                        (Ty::Bool, false)
                    }
                    _ => {
                        let (l, _) = self.expr(&b.left, Mode::Use);
                        let (r, _) = self.expr(&b.right, Mode::Use);
                        let ty = match (&l, &r) {
                            (Ty::List(_), Ty::List(_))
                            | (Ty::Int, Ty::Int)
                            | (Ty::Str, Ty::Str) => merge(&l, &r),
                            _ => Ty::Any,
                        };
                        (ty, false)
                    }
                }
            }
            Expr::Compare(c) => {
                self.expr(&c.expr, Mode::Compare);
                for op in &c.ops {
                    self.expr(&op.expr, Mode::Compare);
                }
                (Ty::Bool, false)
            }
            Expr::IfExpr(i) => {
                let paths = self.cond(&i.test_expr);
                let mark = self.narrowed.len();
                self.narrowed.extend(paths);
                let (yes, yes_optional) = self.infer(&i.true_expr);
                self.narrowed.truncate(mark);
                self.narrowed.extend(narrows_false(&i.test_expr));
                // `{{ a if b }}` prints nothing when `b` is false.
                let (no, no_optional) = match &i.false_expr {
                    Some(f) => self.infer(f),
                    None => (Ty::None, false),
                };
                self.narrowed.truncate(mark);
                (merge(&yes, &no), yes_optional || no_optional)
            }
            Expr::Filter(f) => self.filter(f),
            Expr::Test(t) => {
                let at =
                    t.span().start_offset as usize..t.span().start_offset as usize + t.name.len();
                if !(self.known)(true, t.name) {
                    let d = self.error(
                        &codes::TEMPLATE_UNKNOWN_FILTER,
                        format!("there is no test `{}`", t.name),
                        at.clone(),
                    );
                    let candidates: Vec<&str> = TESTS
                        .iter()
                        .copied()
                        .filter(|n| (self.known)(true, n))
                        .collect();
                    let d = Self::near(d, self.span(at), t.name, candidates);
                    self.push(d);
                }
                let mode = match t.name {
                    "defined" | "undefined" | "none" => Mode::Pass,
                    _ => Mode::Use,
                };
                self.expr(&t.expr, mode);
                self.args(&t.args);
                (Ty::Bool, false)
            }
            Expr::Call(c) => self.call(c, false),
            Expr::List(l) => {
                let items: Vec<Ty> = l.items.iter().map(|i| self.expr(i, Mode::Use).0).collect();
                (list_of(items.into_iter()), false)
            }
            Expr::Tuple(t) => {
                for i in &t.items {
                    self.expr(i, Mode::Use);
                }
                (Ty::Any, false)
            }
            Expr::Map(m) => {
                // A value may be absent: `attrs` leaves it out.
                let mut fields = BTreeMap::new();
                for (k, v) in m.keys.iter().zip(&m.values) {
                    let (ty, optional) = self.expr(v, Mode::Pass);
                    match k.as_const().and_then(|k| k.as_str().map(String::from)) {
                        Some(k) => {
                            fields.insert(k, Field { ty, optional });
                        }
                        None => {
                            self.expr(k, Mode::Use);
                        }
                    }
                }
                let map = Obj {
                    name: "map".into(),
                    fields,
                    open: true,
                };
                (Ty::Obj(Rc::new(map)), false)
            }
        }
    }

    fn var(&mut self, v: &'a Spanned<ast::Var<'a>>) -> (Ty, bool) {
        if let Some(b) = self.lookup(v.id) {
            let narrowed = self.narrowed.iter().any(|p| p == v.id);
            return (b.ty.clone(), b.optional && !narrowed);
        }
        if v.id == "loop" || v.id == "self" {
            return (Ty::Any, false);
        }
        let at = v.span().start_offset as usize..v.span().end_offset as usize;
        let d = match &self.component {
            Some(sig) => {
                let d = self
                    .error(
                        &codes::TEMPLATE_UNDECLARED_INPUT,
                        format!("`{}` isn't a prop of the Component `{}`", v.id, sig.name),
                        at.clone(),
                    )
                    .help(format!(
                        "a Component reads only its props and {}; declare `{}` in its props comment and pass it in",
                        GLOBALS.map(|g| format!("`{g}`")).join(", "),
                        v.id
                    ));
                let props: Vec<&str> = sig.params.iter().map(|p| p.name.as_str()).collect();
                Self::near(d, self.span(at), v.id, props)
            }
            None => {
                let names: Vec<String> =
                    self.scopes.iter().flat_map(|s| s.keys().cloned()).collect();
                let d = self.error(
                    &codes::TEMPLATE_UNKNOWN_FIELD,
                    format!("there is no variable `{}` here", v.id),
                    at.clone(),
                );
                let seen: Vec<String> = self.scopes[..self.scopes.len().min(2)]
                    .iter()
                    .flat_map(|s| s.keys().map(|k| format!("`{k}`")))
                    .collect();
                match similar(v.id, names.iter().map(String::as_str)) {
                    Some(_) => Self::near(d, self.span(at), v.id, names.iter().map(String::as_str)),
                    None => d.help(format!("this Page sees {}", seen.join(", "))),
                }
            }
        };
        self.push(d);
        (Ty::Any, false)
    }

    fn reach_through(&mut self, base: &'a Expr<'a>, whole: &'a Expr<'a>) {
        let at = range(base);
        let what = path_of(base).unwrap_or_else(|| self.text(at.clone()).to_string());
        let full = path_of(whole).unwrap_or_else(|| self.text(range(whole)).to_string());
        let d = self
            .error(
                &codes::TEMPLATE_UNGUARDED_OPTIONAL,
                format!(
                    "`{what}` is optional, so `{full}` reaches through a value that may be absent"
                ),
                at,
            )
            .help(format!("guard it with `{{% if {what} %}}`"));
        self.push(d);
    }

    /// `name` read from a value of type `base`; `at` is where the name is.
    fn field(
        &mut self,
        e: &'a Expr<'a>,
        base_expr: &'a Expr<'a>,
        base: &Ty,
        name: &str,
        at: Range<usize>,
    ) -> (Ty, bool) {
        let on = || path_of(base_expr).unwrap_or_else(|| "this value".into());
        match base {
            Ty::Obj(o) => match o.fields.get(name) {
                Some(f) => (f.ty.clone(), f.optional && !self.is_narrowed(e)),
                None if o.open => (Ty::Any, false),
                None => {
                    let d = self
                        .error(
                            &codes::TEMPLATE_UNKNOWN_FIELD,
                            format!("`{}` has no field `{name}`", on()),
                            at.clone(),
                        )
                        .label(format!("not a field of `{}`", o.name));
                    let d = match similar(name, o.fields.keys().map(String::as_str)) {
                        Some(_) => {
                            Self::near(d, self.span(at), name, o.fields.keys().map(String::as_str))
                        }
                        None => d.help(format!(
                            "its fields are {}",
                            o.fields
                                .keys()
                                .map(|k| format!("`{k}`"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )),
                    };
                    self.push(d);
                    (Ty::Any, false)
                }
            },
            Ty::Any | Ty::Attrs | Ty::Macro(_) | Ty::Module(_) | Ty::None => (Ty::Any, false),
            other => {
                let d = self.error(
                    &codes::TEMPLATE_UNKNOWN_FIELD,
                    format!("`{}` has type `{other}`, which has no field `{name}`", on()),
                    at,
                );
                self.push(d);
                (Ty::Any, false)
            }
        }
    }

    fn args(&mut self, args: &'a [CallArg<'a>]) -> Vec<(Ty, bool)> {
        args.iter()
            .map(|a| match a {
                CallArg::Pos(e)
                | CallArg::Kwarg(_, e)
                | CallArg::PosSplat(e)
                | CallArg::KwargSplat(e) => self.expr(e, Mode::Pass),
            })
            .collect()
    }

    /// Where a kwarg's name is, which minijinja gives no span: just before
    /// its value.
    fn kwarg_at(&self, name: &str, value: &Expr<'_>) -> Range<usize> {
        let before = &self.source()[..range(value).start.min(self.source().len())];
        match before.rfind(name) {
            Some(i) => i..i + name.len(),
            None => range(value),
        }
    }

    fn filter(&mut self, f: &'a Spanned<ast::Filter<'a>>) -> (Ty, bool) {
        let start = f.span().start_offset as usize;
        let at = start..start + f.name.len();
        if !(self.known)(false, f.name) {
            let d = self.error(
                &codes::TEMPLATE_UNKNOWN_FILTER,
                format!("there is no filter `{}`", f.name),
                at.clone(),
            );
            let candidates: Vec<&str> = FILTERS
                .iter()
                .copied()
                .filter(|n| (self.known)(false, n))
                .collect();
            let d = Self::near(d, self.span(at.clone()), f.name, candidates);
            self.push(d);
        }
        let defaulting = matches!(f.name, "default" | "d");
        let (input, _) = match &f.expr {
            Some(inner) => self.expr(inner, if defaulting { Mode::Pass } else { Mode::Use }),
            None => (Ty::Any, false),
        };
        let args = self.args(&f.args);
        self.filter_args(f, &input, at);
        let item = || match &input {
            Ty::List(item) => (**item).clone(),
            _ => Ty::Any,
        };
        let ty = match f.name {
            "default" | "d" => {
                return match (f.args.first(), args.first()) {
                    (Some(CallArg::Pos(e)), _) if e.as_const().is_some_and(|v| v.is_none()) => {
                        (input, true)
                    }
                    (_, Some((ty, optional))) => (
                        if matches!(input, Ty::Any) {
                            ty.clone()
                        } else {
                            input
                        },
                        *optional,
                    ),
                    _ => (input, false),
                };
            }
            "sort" | "sort_by" | "reverse" | "unique" | "list" | "select" | "reject"
            | "selectattr" | "rejectattr" => input,
            "first" | "last" => item(),
            "map" => match self.attribute(f, "attribute", false) {
                Some((key, _)) => match item() {
                    Ty::Obj(o) => Ty::List(Rc::new(
                        o.fields.get(&key).map_or(Ty::Any, |f| f.ty.clone()),
                    )),
                    _ => Ty::List(Rc::new(Ty::Any)),
                },
                None => Ty::List(Rc::new(Ty::Any)),
            },
            "length" | "count" | "int" => Ty::Int,
            "float" => Ty::Float,
            "bool" => Ty::Bool,
            "lower" | "upper" | "trim" | "string" | "title" | "capitalize" | "replace" | "join"
            | "format" | "indent" | "dateformat" | "pluralize" | "tojson" | "urlencode"
            | "pprint" => Ty::Str,
            "safe" | "escape" | "e" | "markdown" | "org" | "attrs" => Ty::Slot,
            "split" | "lines" => Ty::List(Rc::new(Ty::Str)),
            "abs" | "round" => input,
            _ => Ty::Any,
        };
        (ty, false)
    }

    /// A constant string argument by kwarg name, or by position when
    /// `positional`.
    fn attribute(
        &self,
        f: &'a ast::Filter<'a>,
        kwarg: &str,
        positional: bool,
    ) -> Option<(String, Range<usize>)> {
        let arg = f.args.iter().find_map(|a| match a {
            CallArg::Kwarg(k, e) if *k == kwarg => Some(e),
            _ => None,
        });
        let arg = arg.or_else(|| match (positional, f.args.first()) {
            (true, Some(CallArg::Pos(e))) => Some(e),
            _ => None,
        })?;
        let key = arg.as_const()?.as_str()?.to_string();
        Some((key, range(arg)))
    }

    /// The arguments of the filters whose signatures the Engine keeps.
    fn filter_args(&mut self, f: &'a Spanned<ast::Filter<'a>>, input: &Ty, at: Range<usize>) {
        let bad = |c: &Self, message: String, at: Range<usize>| {
            c.error(&codes::TEMPLATE_BAD_FILTER_ARGUMENT, message, at)
        };
        match f.name {
            "dateformat" => {
                let mut format = None;
                for a in &f.args {
                    match a {
                        CallArg::Kwarg("format", e) => format = Some(e),
                        CallArg::Kwarg(k, e) => {
                            let k_at = self.kwarg_at(k, e);
                            let d = bad(
                                self,
                                format!("`dateformat` takes `format`, not `{k}`"),
                                k_at.clone(),
                            );
                            let d = Self::near(d, self.span(k_at), k, ["format"]);
                            self.push(d);
                            return;
                        }
                        CallArg::Pos(e) => {
                            let d = bad(
                                self,
                                "`dateformat` takes its format by name".into(),
                                range(e),
                            )
                            .help("write `dateformat(format=\"%Y-%m-%d\")`");
                            self.push(d);
                            return;
                        }
                        CallArg::PosSplat(_) | CallArg::KwargSplat(_) => return,
                    }
                }
                match format {
                    None => {
                        let d = bad(self, "`dateformat` needs a `format`".into(), at)
                            .help("write `dateformat(format=\"%Y-%m-%d\")`");
                        self.push(d);
                    }
                    Some(e) => {
                        if let Some(text) = e.as_const().and_then(|v| v.as_str().map(String::from))
                            && !known_format(&text)
                        {
                            let d = bad(self, unknown_format(&text), range(e));
                            self.push(d);
                        }
                    }
                }
            }
            "sort_by" => {
                let keys: Vec<(String, Range<usize>)> = match f.args.first() {
                    Some(CallArg::Pos(Expr::List(l))) => l
                        .items
                        .iter()
                        .filter_map(|i| Some((i.as_const()?.as_str()?.to_string(), range(i))))
                        .collect(),
                    Some(CallArg::Pos(e)) => e
                        .as_const()
                        .and_then(|v| v.as_str().map(|s| (s.to_string(), range(e))))
                        .into_iter()
                        .collect(),
                    _ => {
                        let d = bad(self, "`sort_by` needs a list of keys".into(), at)
                            .help("write `sort_by([\"-date\", \"title\"])`");
                        self.push(d);
                        return;
                    }
                };
                for (key, at) in keys {
                    self.key("sort_by", input, key.trim_start_matches('-'), at);
                }
            }
            "sort" => {
                let Some((keys, at)) = self.attribute(f, "attribute", false) else {
                    return;
                };
                for key in keys.split(',').map(str::trim) {
                    if let Some(field) = key.strip_prefix('-') {
                        let d = bad(
                            self,
                            format!(
                                "Jinja's `sort` reads `{key}` as a field named `{key}`, so it doesn't sort by `{field}`"
                            ),
                            at.clone(),
                        );
                        let filter = f.span().start_offset as usize..f.span().end_offset as usize;
                        let edit = Edit {
                            span: self.span(filter),
                            replacement: format!("sort_by([\"{key}\"])"),
                        };
                        let d = d.suggest(
                            format!("to sort by `{field}`, descending, use `sort_by([\"{key}\"])`, which also puts missing values last"),
                            Applicability::MaybeIncorrect,
                            vec![edit],
                        );
                        self.push(d);
                    } else {
                        self.key("sort", input, key, at.clone());
                    }
                }
            }
            "map" | "groupby" => {
                if let Some((key, at)) = self.attribute(f, "attribute", f.name == "groupby") {
                    self.key(f.name, input, &key, at);
                }
            }
            "selectattr" | "rejectattr" => {
                if let Some((key, at)) = self.attribute(f, "", true) {
                    self.key(f.name, input, &key, at);
                }
            }
            "attrs" if !assignable(input, &Ty::Attrs) => {
                let d = self.error(
                    &codes::TEMPLATE_WRONG_TYPE,
                    format!("`attrs` takes a map of attributes, not `{input}`"),
                    at,
                );
                self.push(d);
            }
            _ => {}
        }
    }

    /// A field name a filter reads from each item of a list.
    fn key(&mut self, filter: &str, input: &Ty, key: &str, at: Range<usize>) {
        let Ty::List(item) = input else { return };
        let Ty::Obj(o) = &**item else { return };
        let field = key.split('.').next().unwrap_or(key);
        if o.open || o.fields.contains_key(field) {
            return;
        }
        let d = self.error(
            &codes::TEMPLATE_UNKNOWN_FIELD,
            format!(
                "`{filter}` reads `{field}`, which `{}` has no field named",
                o.name
            ),
            at.clone(),
        );
        let d = match similar(field, o.fields.keys().map(String::as_str)) {
            Some(near) => {
                let text = self.text(at.clone());
                let edit = Edit {
                    span: self.span(at),
                    replacement: text.replacen(field, near, 1),
                };
                d.suggest(
                    format!("did you mean `{near}`?"),
                    Applicability::MaybeIncorrect,
                    vec![edit],
                )
            }
            None => d,
        };
        self.push(d);
    }

    fn call(&mut self, c: &'a Spanned<ast::Call<'a>>, with_body: bool) -> (Ty, bool) {
        match c.identify_call() {
            ast::CallType::Function(name) => {
                let callee = range(&c.expr);
                match self.lookup(name).map(|b| b.ty.clone()) {
                    Some(Ty::Macro(sig)) => return self.macro_call(&sig, c, callee, with_body),
                    Some(_) => {
                        self.args(&c.args);
                        return (Ty::Any, false);
                    }
                    None => {}
                }
                self.args(&c.args);
                match name {
                    "super" | "caller" => (Ty::Slot, false),
                    "url" | "absolute_url" if self.functions.contains(name) => (Ty::Url, false),
                    "range" if self.functions.contains(name) => (Ty::List(Rc::new(Ty::Int)), false),
                    _ if self.functions.contains(name) => (Ty::Any, false),
                    _ => {
                        let d = self.error(
                            &codes::TEMPLATE_UNKNOWN_FILTER,
                            format!("there is no function or macro `{name}`"),
                            callee.clone(),
                        );
                        let macros: Vec<String> = self
                            .scopes
                            .iter()
                            .flat_map(|s| {
                                s.iter()
                                    .filter(|(_, b)| matches!(b.ty, Ty::Macro(_)))
                                    .map(|(k, _)| k.clone())
                            })
                            .collect();
                        let candidates: Vec<&str> = self
                            .functions
                            .iter()
                            .chain(&macros)
                            .map(String::as_str)
                            .collect();
                        let d = Self::near(d, self.span(callee), name, candidates);
                        self.push(d);
                        (Ty::Any, false)
                    }
                }
            }
            ast::CallType::Method(base, name) => {
                let start = c.span().start_offset as usize + 1;
                let at = start..start + name.len();
                if let Expr::Var(v) = base
                    && let Some(Ty::Module(file)) = self.lookup(v.id).map(|b| b.ty.clone())
                {
                    let sig = self.macros.get(&*file).and_then(|m| m.get(name)).cloned();
                    if let Some(sig) = sig {
                        return self.macro_call(&sig, c, range(&c.expr), with_body);
                    }
                    let d = self.error(
                        &codes::TEMPLATE_UNKNOWN_FILTER,
                        format!("`{file}`, imported as `{}`, has no macro `{name}`", v.id),
                        at.clone(),
                    );
                    let candidates: Vec<&str> = self
                        .macros
                        .get(&*file)
                        .into_iter()
                        .flat_map(|m| m.keys().copied())
                        .collect();
                    let d = Self::near(d, self.span(at), name, candidates);
                    self.push(d);
                    self.args(&c.args);
                    return (Ty::Any, false);
                }
                let (ty, _) = self.expr(base, Mode::Use);
                self.args(&c.args);
                if !matches!(ty, Ty::Any) {
                    let d = self.error(
                        &codes::TEMPLATE_UNKNOWN_FILTER,
                        format!("`.{name}()` is a method call on a `{ty}`, and Templates have no methods"),
                        at,
                    );
                    let d = match (self.known)(false, name) {
                        true => d.help(format!("use the filter: `| {name}`")),
                        false => d,
                    };
                    self.push(d);
                }
                (Ty::Any, false)
            }
            ast::CallType::Block(_) => (Ty::Slot, false),
            ast::CallType::Object(e) => {
                self.expr(e, Mode::Use);
                self.args(&c.args);
                (Ty::Any, false)
            }
        }
    }

    fn macro_call(
        &mut self,
        sig: &Rc<Sig>,
        c: &'a ast::Call<'a>,
        callee: Range<usize>,
        with_body: bool,
    ) -> (Ty, bool) {
        let what = match sig.props {
            Some(_) => "the Component",
            None => "the macro",
        };
        let bad = |c: &Self, message: String, at: Range<usize>| {
            c.error(&codes::TEMPLATE_BAD_PROP, message, at)
        };
        let mut given = BTreeSet::new();
        let mut splat = false;
        let mut position = 0;
        for a in &c.args {
            let (name, value) = match a {
                CallArg::Pos(e) => {
                    let name = sig.params.get(position).map(|p| p.name.clone());
                    position += 1;
                    if name.is_none() {
                        let message = format!(
                            "{what} `{}` takes {} arguments, not more",
                            sig.name,
                            sig.params.len()
                        );
                        let d = bad(self, message, range(e));
                        self.push(d);
                    }
                    (name, e)
                }
                CallArg::Kwarg(k, e) => match sig.params.iter().any(|p| p.name == *k) {
                    true => (Some(k.to_string()), e),
                    false => {
                        let at = self.kwarg_at(k, e);
                        let message = format!("{what} `{}` has no prop `{k}`", sig.name);
                        let d = bad(self, message, at.clone())
                            .note(format!("`{}` is in `{}`", sig.name, sig.file));
                        let d = Self::near(
                            d,
                            self.span(at),
                            k,
                            sig.params.iter().map(|p| p.name.as_str()),
                        );
                        self.push(d);
                        (None, e)
                    }
                },
                CallArg::PosSplat(e) | CallArg::KwargSplat(e) => {
                    splat = true;
                    (None, e)
                }
            };
            let (ty, optional) = self.expr(value, Mode::Pass);
            let Some(name) = name else { continue };
            let at = range(value);
            if !given.insert(name.clone()) {
                let d = bad(
                    self,
                    format!("the prop `{name}` of `{}` is passed twice", sig.name),
                    at,
                );
                self.push(d);
                continue;
            }
            let Some(prop) = sig.props.iter().flatten().find(|p| p.name == name) else {
                continue;
            };
            let text = path_of(value).unwrap_or_else(|| self.text(at.clone()).to_string());
            if !assignable(&ty, &prop.ty) {
                let message = format!(
                    "the prop `{name}` of `{}` has type `{}`, but `{text}` has type `{ty}`",
                    sig.name, prop.ty
                );
                let d = bad(self, message, at);
                self.push(d);
            } else if optional && !prop.optional {
                let message = format!(
                    "`{text}` is optional, but the prop `{name}` of `{}` is required",
                    sig.name
                );
                let d = bad(self, message, at).help(format!(
                    "guard the call with `{{% if {text} %}}`, or pass `{text} | default(…)`"
                ));
                self.push(d);
            } else if let (Ty::Enum(values), Some(literal)) = (&prop.ty, value.as_const())
                && let Some(literal) = literal.as_str()
                && !values.iter().any(|v| v == literal)
            {
                let message = format!(
                    "the prop `{name}` of `{}` is one of {}, not \"{literal}\"",
                    sig.name,
                    values
                        .iter()
                        .map(|v| format!("\"{v}\""))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                let d = bad(self, message, at);
                self.push(d);
            }
        }
        if !splat {
            for p in &sig.params {
                let required = match sig.props.iter().flatten().find(|x| x.name == p.name) {
                    Some(x) => !x.optional,
                    None => !p.has_default,
                };
                if required && !given.contains(&p.name) {
                    let message = format!("{what} `{}` needs the prop `{}`", sig.name, p.name);
                    let d = bad(self, message, callee.clone())
                        .note(format!("`{}` is in `{}`", sig.name, sig.file));
                    self.push(d);
                }
            }
        }
        match (with_body, sig.slot) {
            (true, Slot::No) => {
                let message = format!(
                    "`{}` takes no `{{% call %}}` body: it never calls `caller()`",
                    sig.name
                );
                let d = bad(self, message, callee.clone())
                    .help(format!("call it as `{{{{ {}(…) }}}}`", self.text(callee)));
                self.push(d);
            }
            (false, Slot::Required) => {
                let message = format!(
                    "`{}` needs a `{{% call %}}` body, because it calls `caller()`",
                    sig.name
                );
                let d = bad(self, message, callee.clone()).help(format!(
                    "write `{{% call {}(…) %}}…{{% endcall %}}`",
                    self.text(callee)
                ));
                self.push(d);
            }
            _ => {}
        }
        (Ty::Slot, false)
    }
}
