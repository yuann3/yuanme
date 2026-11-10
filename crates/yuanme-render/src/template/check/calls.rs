//! Calls: functions, `image()`, methods, and macros with their props.

use std::collections::BTreeSet;
use std::ops::Range;
use std::rc::Rc;

use minijinja::machinery::ast::{self, CallArg, Expr, Spanned};
use yuanme_diag::codes;

use super::Checker;
use super::infer::{Mode, path_of, range};
use super::signatures::{Sig, Slot};
use super::types::{Ty, assignable, obj};

/// The type `image()` returns, by the name `picture` knows it by.
pub(super) const IMAGE: &str = "Image";

impl<'a> Checker<'a> {
    pub(super) fn call(&mut self, c: &'a Spanned<ast::Call<'a>>, with_body: bool) -> (Ty, bool) {
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
                if name == "image" && self.functions.contains(name) {
                    return (self.image(&c.args, callee), false);
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

    /// `image(src, …)`, whose `src` a literal lets `check` judge before
    /// any Page is rendered.
    fn image(&mut self, args: &'a [CallArg<'a>], callee: Range<usize>) -> Ty {
        const KWARGS: [&str; 6] = ["width", "widths", "format", "formats", "quality", "sizes"];
        let bad = |c: &Self, message: String, at: Range<usize>| {
            c.error(&codes::TEMPLATE_BAD_FILTER_ARGUMENT, message, at)
        };
        let mut src = None;
        let mut given = BTreeSet::new();
        for a in args {
            match a {
                CallArg::Pos(e) if src.is_none() => src = Some(e),
                CallArg::Kwarg(k, e) => {
                    self.expr(e, Mode::Pass);
                    if KWARGS.contains(k) {
                        given.insert(*k);
                        continue;
                    }
                    let at = self.kwarg_at(k, e);
                    let d = bad(self, format!("`image` has no option `{k}`"), at.clone())
                        .help("its options are `width`, `widths`, `format`, `formats`, `quality` and `sizes`");
                    let d = Self::near(d, self.span(at), k, KWARGS);
                    self.push(d);
                }
                CallArg::Pos(e) => {
                    self.expr(e, Mode::Pass);
                    let d = bad(
                        self,
                        "`image` takes one image, then its options by name".into(),
                        range(e),
                    );
                    self.push(d);
                }
                CallArg::PosSplat(e) | CallArg::KwargSplat(e) => {
                    self.expr(e, Mode::Pass);
                }
            }
        }
        for (one, many) in [("width", "widths"), ("format", "formats")] {
            if given.contains(one) && given.contains(many) {
                let d = bad(
                    self,
                    format!("`image` takes `{one}` or `{many}`, not both"),
                    callee.clone(),
                );
                self.push(d);
            }
        }
        match src {
            None if !args.iter().any(|a| matches!(a, CallArg::PosSplat(_))) => {
                let d = bad(self, "`image` needs an image".into(), callee)
                    .help("write `image(entry.cover)` or `image(\"/photos/sea.jpg\")`");
                self.push(d);
            }
            None => {}
            Some(e) => {
                let (ty, _) = self.expr(e, Mode::Use);
                if !assignable(&ty, &Ty::Image) {
                    let d = self.error(
                        &codes::TEMPLATE_WRONG_TYPE,
                        format!("`image` takes a path or an `image` field, not `{ty}`"),
                        range(e),
                    );
                    self.push(d);
                }
                if let Some(path) = e.as_const().and_then(|v| v.as_str().map(String::from))
                    && let Err((code, message)) = self.images.check(&path)
                {
                    let d = self.error(code, format!("`image`: {message}"), range(e));
                    self.push(d);
                }
            }
        }
        let source = obj(
            "an Image's source",
            [("type", Ty::Str, false), ("srcset", Ty::Str, false)],
        );
        obj(
            IMAGE,
            [
                ("width", Ty::Int, false),
                ("height", Ty::Int, false),
                ("url", Ty::Url, false),
                ("srcset", Ty::Str, false),
                ("sizes", Ty::Str, false),
                ("sources", Ty::List(Rc::new(source)), false),
            ],
        )
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
