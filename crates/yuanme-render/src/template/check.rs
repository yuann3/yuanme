//! `check` (#19): every Template walked against the schema of what its
//! Pages see, before any Page renders. It finds what rendering can't: a
//! typo on a path no Entry reaches or one `| default` hides, a Component
//! reading what it wasn't given, and a prop of the wrong type. minijinja's
//! AST has no stability promise, so all that reads it is here (#24).
//!
//! It reads each Template as the Author wrote it, so its spans need no
//! map; render errors in a Template that scoped styles rewrite are mapped
//! back to it (ADR 0006).

mod calls;
mod filters;
mod headers;
mod infer;
mod schema;
mod signatures;
mod types;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::rc::Rc;

use minijinja::machinery::ast::{self, Expr, Spanned, Stmt};
use minijinja::machinery::parse;
use minijinja::{Environment, context};
use yuanme_content::config::{Config, similar};
use yuanme_content::site::Site;
use yuanme_diag::{Applicability, Code, Diagnostic, Edit, FileId, Files, Span, codes};

use crate::image::Images;
use headers::Prop;
use infer::{Mode, narrows_false, path_of, range};
use schema::Schema;
pub use schema::context_schemas;
use signatures::Sig;
use types::{Ty, assignable, merge};

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
    images: &dyn Images,
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
        images,
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
    images: &'a dyn Images,
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
}
