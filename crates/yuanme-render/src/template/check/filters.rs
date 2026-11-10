//! Filters: what each one returns, and the arguments of the ones whose
//! signatures the Engine keeps.

use std::ops::Range;
use std::rc::Rc;

use minijinja::machinery::ast::{self, CallArg, Expr, Spanned};
use yuanme_content::config::similar;
use yuanme_diag::{Applicability, Edit, codes};

use super::calls::IMAGE;
use super::types::{Ty, assignable};
use super::{Checker, Mode, range};
use crate::template::filters::{known_format, unknown_format};

/// For "did you mean": minijinja can't list its filters and tests, so these
/// are only suggestions. Whether a name exists is asked of the Environment.
#[rustfmt::skip]
const FILTERS: &[&str] = &[
    "abs", "attr", "batch", "bool", "capitalize", "chain", "count", "d", "default", "dictsort", "e",
    "escape", "first", "float", "format", "groupby", "indent", "int", "items", "join", "last",
    "length", "lines", "list", "lower", "map", "max", "min", "pprint", "reject", "rejectattr",
    "replace", "reverse", "round", "safe", "select", "selectattr", "slice", "sort", "split",
    "string", "sum", "title", "tojson", "trim", "unique", "upper", "urlencode", "zip",
    "sort_by", "dateformat", "pluralize", "attrs", "markdown", "org", "picture",
];

impl<'a> Checker<'a> {
    pub(super) fn filter(&mut self, f: &'a Spanned<ast::Filter<'a>>) -> (Ty, bool) {
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
            "safe" | "escape" | "e" | "markdown" | "org" | "attrs" | "picture" => Ty::Slot,
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
            "picture" => self.picture(f, input, at),
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

    /// `image | picture(…)`: an Image, with an `alt`.
    fn picture(&mut self, f: &'a Spanned<ast::Filter<'a>>, input: &Ty, at: Range<usize>) {
        const KWARGS: [&str; 5] = ["alt", "priority", "class", "loading", "attrs"];
        let bad = |c: &Self, message: String, at: Range<usize>| {
            c.error(&codes::TEMPLATE_BAD_FILTER_ARGUMENT, message, at)
        };
        if !matches!(input, Ty::Any) && !matches!(input, Ty::Obj(o) if o.name == IMAGE) {
            let d = self
                .error(
                    &codes::TEMPLATE_WRONG_TYPE,
                    format!("`picture` takes an Image from `image()`, not `{input}`"),
                    at.clone(),
                )
                .help("write `image(entry.cover) | picture(alt=\"…\")`");
            self.push(d);
        }
        let mut alt = false;
        for a in &f.args {
            match a {
                CallArg::Kwarg(k, e) if KWARGS.contains(k) => {
                    alt |= *k == "alt";
                    let loading = e.as_const().and_then(|v| v.as_str().map(String::from));
                    if *k == "loading"
                        && let Some(loading) = loading.filter(|l| l != "lazy" && l != "eager")
                    {
                        let d = bad(
                            self,
                            format!("`loading` is \"lazy\" or \"eager\", not \"{loading}\""),
                            range(e),
                        );
                        self.push(d);
                    }
                }
                CallArg::Kwarg(k, e) => {
                    let k_at = self.kwarg_at(k, e);
                    let d = bad(self, format!("`picture` has no option `{k}`"), k_at.clone())
                        .help("its options are `alt`, `priority`, `class`, `loading` and `attrs`; `attrs` takes any other attribute");
                    let d = Self::near(d, self.span(k_at), k, KWARGS);
                    self.push(d);
                }
                CallArg::Pos(e) => {
                    let d = bad(self, "`picture` takes its options by name".into(), range(e))
                        .help("write `picture(alt=\"…\")`");
                    self.push(d);
                }
                CallArg::PosSplat(_) | CallArg::KwargSplat(_) => return,
            }
        }
        if !alt {
            let d = bad(
                self,
                "`picture` needs `alt`, which says what the image shows".into(),
                at,
            )
            .help("write `alt=\"\"` for an image that is only decoration");
            self.push(d);
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
}
