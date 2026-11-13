//! Pass 2: each Component's body, which sees only its props and the
//! globals.
//!
//! Pass 3: each Route's Template, with what its Pages see.
//!
//! Pass 4: each Template that pass 3 didn't reach, as any Route's.

use std::collections::BTreeMap;
use std::ops::Range;

use minijinja::machinery::ast::{self, Expr, Spanned, Stmt};
use yuanme_content::similar;
use yuanme_diag::{Applicability, Edit, Span, codes};

use super::headers::Prop;
use super::infer::{Mode, range};
use super::signatures::Sig;
use super::types::{Ty, assignable};
use super::{Bind, Checker, Group};

impl<'a> Checker<'a> {
    pub(super) fn components(&mut self) {
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

    pub(super) fn macro_body(&mut self, m: &'a Spanned<ast::Macro<'a>>, sig: &Sig) {
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

    fn reset(&mut self) {
        self.stack.clear();
        self.narrowed.clear();
        self.overrides.clear();
        self.active.clear();
        self.seen_blocks.clear();
        self.chain_broken = false;
        self.component = None;
        self.route.clear();
        self.open = false;
        let globals = self
            .globals
            .iter()
            .map(|(n, ty)| (n.to_string(), Bind::new(ty.clone(), false)))
            .collect();
        self.scopes = vec![globals];
    }

    pub(super) fn route(&mut self, g: &Group) {
        self.reset();
        self.route = g.labels.join(" or ");
        let vars = g
            .vars
            .iter()
            .map(|(n, ty)| (n.to_string(), Bind::new(ty.clone(), false)))
            .collect();
        let template = self
            .asts
            .get_key_value(g.template.as_str())
            .expect("`check` groups only Templates that parsed")
            .0;
        self.page(template, vars);
    }

    /// A Template no Route uses, which any could come to, and which a
    /// Template could extend or include: a Layout's vars are set as it
    /// declares them, and a name nothing else binds is `Any`.
    pub(super) fn unused(&mut self, template: &'a str) {
        self.reset();
        self.route = "any Route, as none uses it".to_string();
        self.open = true;
        let vars = self
            .vars
            .get(template)
            .into_iter()
            .flatten()
            .map(|v| (v.name.clone(), Bind::new(v.ty.clone(), v.optional)))
            .collect();
        self.page(template, vars);
    }

    /// Walks `template` as a Page's, which sees `vars`, and then the
    /// blocks it overrides that no Layout it extends has.
    fn page(&mut self, template: &'a str, vars: BTreeMap<String, Bind>) {
        self.scopes.push(vars);
        self.scopes.push(BTreeMap::new());
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
    pub(super) fn walk(&mut self, name: &'a str, extended: Option<Span>) {
        let Some(Stmt::Template(t)) = self.asts.get(name) else {
            return;
        };
        // An include cycle renders forever; twice is enough to check it.
        if self.stack.iter().filter(|f| **f == name).count() >= 2 {
            return;
        }
        self.walked.insert(name);
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
    pub(super) fn template_names(&mut self, e: &'a Expr<'a>, tag: &str) -> Vec<&'a str> {
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
}
