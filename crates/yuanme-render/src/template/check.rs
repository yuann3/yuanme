//! `check` (#19): every Template walked against the schema of what its
//! Pages see, before any Page renders. It finds what rendering can't: a
//! typo on a path no Entry reaches or one `| default` hides, a Component
//! reading what it wasn't given, and a prop of the wrong type. minijinja's
//! AST has no stability promise, so all that reads it is here (#24).
//!
//! It reads each Template as the Author wrote it, so its spans need no
//! map; render errors in a Template that scoped styles rewrite are mapped
//! back to it (ADR 0006).
//!
//! Pass 1, in `signatures`, reads every macro's signature and the props
//! and vars comments of `headers`. Passes 2 to 4, in `walk`, check each
//! Component's body, each Route's Template and then each Template no
//! Route reaches, statement by statement in `stmt` and expression by
//! expression in `infer`, `filters` and `calls`, against the `types` of
//! what `schema` says a Page sees.

mod calls;
mod filters;
mod headers;
mod infer;
mod schema;
mod signatures;
mod stmt;
mod types;
mod walk;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::rc::Rc;

use minijinja::machinery::ast::{self, Spanned, Stmt};
use minijinja::machinery::parse;
use minijinja::{Environment, context};
use yuanme_content::{Config, Site, similar};
use yuanme_diag::{Applicability, Code, Diagnostic, Edit, FileId, Files, Span};

use crate::image::Images;
use headers::Prop;
use schema::Schema;
pub use schema::context_schemas;
use signatures::Sig;
use types::{Ty, merge};

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
        .expect("`n is filter` is fixed, valid Jinja");
    let is_test = env
        .compile_expression("n is test")
        .expect("`n is test` is fixed, valid Jinja");
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
        walked: BTreeSet::new(),
        open: false,
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
    // Pass 2 checks Components on their own.
    let unused: Vec<&str> = asts
        .keys()
        .copied()
        .filter(|t| !t.starts_with("components/") && !c.walked.contains(t))
        .collect();
    for template in unused {
        // One walked as a Partial of an earlier one is checked.
        if !c.walked.contains(template) {
            c.unused(template);
        }
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
    /// Every Template a walk has reached.
    walked: BTreeSet<&'a str>,
    /// Whether a name nothing binds is `Any`, in a Template no Route uses.
    open: bool,
}

impl<'a> Checker<'a> {
    fn file(&self) -> FileId {
        self.templates[*self
            .stack
            .last()
            .expect("each pass pushes the Template it reads before reading it")]
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

    fn bind(&mut self, name: &str, b: Bind) {
        self.narrowed
            .retain(|p| p != name && !p.starts_with(&format!("{name}.")));
        self.scopes
            .last_mut()
            .expect("`reset` starts every walk with the globals' scope")
            .insert(name.to_string(), b);
    }

    fn lookup(&self, name: &str) -> Option<&Bind> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }
}
