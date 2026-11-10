//! Expressions: each one's type and whether it may be absent, the paths a
//! condition proves present, and what each use requires of a value.

use std::collections::BTreeMap;
use std::ops::Range;
use std::rc::Rc;

use minijinja::machinery::ast::{self, CallArg, Expr, Spanned};
use minijinja::value::ValueKind;
use yuanme_content::config::similar;
use yuanme_diag::codes;

use super::Checker;
use super::types::{Field, Obj, Ty, const_ty, list_of, merge};

/// What every Page sees, and so what a Component may read besides its props.
const GLOBALS: [&str; 6] = [
    "site",
    "data",
    "collections",
    "taxonomies",
    "page",
    "yuanme",
];

#[rustfmt::skip]
const TESTS: &[&str] = &[
    "defined", "undefined", "none", "safe", "escaped", "boolean", "odd", "even", "divisibleby",
    "number", "integer", "float", "string", "sequence", "iterable", "mapping", "startingwith",
    "endingwith", "lower", "upper", "sameas", "eq", "equalto", "ne", "lt", "lessthan", "le", "gt",
    "greaterthan", "ge", "in", "true", "false", "filter", "test",
];

/// What a use of a value requires of it.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Mode {
    /// Printed, looped over, filtered: an absent value fails the Page.
    Use,
    /// `==`, `<`, `in`: an absent value fails the Page, even under SemiStrict.
    Compare,
    /// Tested for truth: an absent value is false.
    Guard,
    /// Bound or passed on, where it stays optional.
    Pass,
}

/// The byte range an expression covers. minijinja starts some spans late,
/// such as an attribute's at its dot and a method call's at its name, and
/// an operator's early, so they are rebuilt from their parts.
pub(super) fn range(e: &Expr<'_>) -> Range<usize> {
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
pub(super) fn path_of(e: &Expr<'_>) -> Option<String> {
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
pub(super) fn narrows_false(e: &Expr<'_>) -> Vec<String> {
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

impl<'a> Checker<'a> {
    /// Checks a condition, and returns the paths it proves present.
    pub(super) fn cond(&mut self, e: &'a Expr<'a>) -> Vec<String> {
        self.expr(e, Mode::Guard);
        narrows(e)
    }

    fn is_narrowed(&self, e: &Expr<'_>) -> bool {
        path_of(e).is_some_and(|p| self.narrowed.contains(&p))
    }

    /// An expression's type, and whether it may still be absent, having
    /// reported what `mode` doesn't allow of an absent value.
    pub(super) fn expr(&mut self, e: &'a Expr<'a>, mode: Mode) -> (Ty, bool) {
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
            Expr::GetItem(g) => self.get_item(e, g),
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
            Expr::BinOp(b) => self.bin_op(b),
            Expr::Compare(c) => {
                self.expr(&c.expr, Mode::Compare);
                for op in &c.ops {
                    self.expr(&op.expr, Mode::Compare);
                }
                (Ty::Bool, false)
            }
            Expr::IfExpr(i) => self.if_expr(i),
            Expr::Filter(f) => self.filter(f),
            Expr::Test(t) => self.test(t),
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
            Expr::Map(m) => self.map(m),
        }
    }

    /// `e` is `g` as an expression, for its path.
    fn get_item(&mut self, e: &'a Expr<'a>, g: &'a Spanned<ast::GetItem<'a>>) -> (Ty, bool) {
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

    fn bin_op(&mut self, b: &'a Spanned<ast::BinOp<'a>>) -> (Ty, bool) {
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
                    (Ty::List(_), Ty::List(_)) | (Ty::Int, Ty::Int) | (Ty::Str, Ty::Str) => {
                        merge(&l, &r)
                    }
                    _ => Ty::Any,
                };
                (ty, false)
            }
        }
    }

    fn if_expr(&mut self, i: &'a Spanned<ast::IfExpr<'a>>) -> (Ty, bool) {
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

    fn test(&mut self, t: &'a Spanned<ast::Test<'a>>) -> (Ty, bool) {
        let at = t.span().start_offset as usize..t.span().start_offset as usize + t.name.len();
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

    fn map(&mut self, m: &'a Spanned<ast::Map<'a>>) -> (Ty, bool) {
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

    pub(super) fn args(&mut self, args: &'a [CallArg<'a>]) -> Vec<(Ty, bool)> {
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
    pub(super) fn kwarg_at(&self, name: &str, value: &Expr<'_>) -> Range<usize> {
        let before = &self.source()[..range(value).start.min(self.source().len())];
        match before.rfind(name) {
            Some(i) => i..i + name.len(),
            None => range(value),
        }
    }
}
