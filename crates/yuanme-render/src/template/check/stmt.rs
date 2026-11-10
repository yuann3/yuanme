//! Statements: what each one binds, narrows and walks.

use std::collections::BTreeMap;
use std::rc::Rc;

use minijinja::machinery::ast::{self, Expr, Spanned, Stmt};
use yuanme_diag::codes;

use super::infer::{Mode, narrows_false, path_of, range};
use super::types::Ty;
use super::{Bind, Checker};

impl<'a> Checker<'a> {
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

    pub(super) fn body(&mut self, stmts: &'a [Stmt<'a>]) {
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

    pub(super) fn stmt(&mut self, s: &'a Stmt<'a>) {
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
}
