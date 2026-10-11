//! `check`: validate templates against per-route-kind schemas without rendering.
//! Walks minijinja's AST (`unstable_machinery`). Throwaway spike code.
use minijinja::machinery::{ast, parse, Span};
use minijinja::syntax::SyntaxConfig;
use minijinja::value::ValueKind;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use std::rc::Rc;

// ------------------------------------------------------------------ types

#[derive(Clone, Debug)]
pub enum Ty {
    Any,
    None,
    Str,
    Int,
    Float,
    Bool,
    Date,
    DateTime,
    Url,
    Image,
    Enum(Vec<String>),
    List(Box<Ty>),
    Obj(Rc<Obj>),
    Slot,
    Attrs,
    Macro(Rc<MacroSig>),
    Ns(String),
}
#[derive(Debug)]
pub struct Obj {
    pub name: String,
    pub fields: BTreeMap<String, Field>,
    pub open: bool,
}
#[derive(Debug, Clone)]
pub struct Field {
    pub ty: Ty,
    pub optional: bool,
}
pub fn obj(name: &str, fields: &[(&str, Ty, bool)]) -> Ty {
    Ty::Obj(Rc::new(Obj {
        name: name.to_string(),
        fields: fields.iter().map(|(n, t, o)| (n.to_string(), Field { ty: t.clone(), optional: *o })).collect(),
        open: false,
    }))
}
impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Any => write!(f, "any"),
            Ty::None => write!(f, "none"),
            Ty::Str => write!(f, "string"),
            Ty::Int => write!(f, "int"),
            Ty::Float => write!(f, "float"),
            Ty::Bool => write!(f, "bool"),
            Ty::Date => write!(f, "date"),
            Ty::DateTime => write!(f, "datetime"),
            Ty::Url => write!(f, "url"),
            Ty::Image => write!(f, "image"),
            Ty::Enum(v) => write!(f, "enum({})", v.join("|")),
            Ty::List(t) => write!(f, "list<{t}>"),
            Ty::Obj(o) => write!(f, "object {}", o.name),
            Ty::Slot => write!(f, "slot"),
            Ty::Attrs => write!(f, "attrs"),
            Ty::Macro(m) => write!(f, "macro {}", m.name),
            Ty::Ns(n) => write!(f, "import of {n}"),
        }
    }
}

fn assignable(arg: &Ty, prop: &Ty) -> bool {
    use Ty::*;
    match (arg, prop) {
        (Any, _) | (_, Any) | (None, _) => true,
        (Str | Url | Enum(_) | Image | Slot, Str) => true,
        (Str | Url | Image, Url) => true,
        (Str | Enum(_), Enum(_)) => true,
        (Int, Int) | (Int | Float, Float) | (Bool, Bool) | (Date, Date) | (Date | DateTime, DateTime) => true,
        (Str | Url | Image, Image) => true,
        (Str | Slot, Slot) => true,
        (Obj(_) | Attrs, Attrs) => true,
        (List(a), List(p)) => assignable(a, p),
        (Obj(a), Obj(p)) => Rc::ptr_eq(a, p) || a.name == p.name,
        _ => false,
    }
}

#[derive(Debug)]
pub struct MacroSig {
    pub name: String,
    pub file: String,
    pub params: Vec<(String, bool)>, // name, has default
    pub props: Option<Vec<Prop>>,
    pub slot: SlotUse,
}
#[derive(Debug, Clone)]
pub struct Prop {
    pub name: String,
    pub ty: Ty,
    pub optional: bool,
}
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SlotUse {
    No,
    Optional,
    Required,
}

pub struct RouteKind {
    pub name: String,
    pub template: String,
    pub vars: Vec<(String, Ty, bool)>,
}

#[derive(Debug, Clone)]
pub struct Diag {
    pub file: String,
    pub line: u32,
    pub col: u32,
    pub msg: String,
    pub warn: bool,
}
impl fmt::Display for Diag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}:{}: {}{}", self.file, self.line, self.col, if self.warn { "warning: " } else { "" }, self.msg)
    }
}

#[derive(Clone)]
struct Bind {
    ty: Ty,
    optional: bool,
    lit: Option<String>,
}

#[derive(PartialEq, Clone, Copy)]
enum Mode {
    Use,   // printed / iterated / chained: an optional value here is an error
    Guard, // truthiness: fine under SemiStrict (counted: Strict rejects these)
    Pass,  // handed on (set, with, macro arg, `is defined`, `| default`): optionality travels with it
}

// ------------------------------------------------------------------ registry

const FILTERS: &[&str] = &[
    "abs", "attr", "batch", "bool", "capitalize", "chain", "count", "d", "default", "dictsort", "e", "escape", "first", "float", "format",
    "groupby", "indent", "int", "items", "join", "last", "length", "lines", "list", "lower", "map", "max", "min", "pprint", "reject",
    "rejectattr", "replace", "reverse", "round", "safe", "select", "selectattr", "slice", "sort", "split", "string", "sum", "title",
    "tojson", "trim", "unique", "upper", "urlencode", "zip",
    // minijinja-contrib
    "dateformat", "datetimeformat", "timeformat", "filesizeformat", "truncate", "striptags",
    // Engine
    "sort_by", "attrs", "pluralize",
];
const TESTS: &[&str] = &[
    "defined", "undefined", "none", "true", "false", "odd", "even", "number", "integer", "float", "string", "sequence", "iterable",
    "mapping", "boolean", "safe", "escaped", "divisibleby", "startingwith", "endingwith", "eq", "equalto", "==", "ne", "!=", "lt",
    "<", "le", "<=", "gt", ">", "ge", ">=", "in", "filter", "test", "lower", "upper", "sameas",
];
const FUNCTIONS: &[&str] = &["range", "dict", "debug", "namespace", "now", "cycler", "joiner", "url", "absolute_url", "asset"];
const TYPE_NAMES: &[&str] = &["string", "int", "float", "bool", "date", "datetime", "url", "enum", "list", "object", "image", "slot", "attrs"];

fn lev(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i];
        for j in 1..=b.len() {
            cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + (a[i - 1] != b[j - 1]) as usize));
        }
        prev = cur;
    }
    prev[b.len()]
}
fn dym<'x>(name: &str, cands: impl Iterator<Item = &'x str>) -> String {
    let best = cands
        .map(|c| (lev(name, c), c.chars().next() != name.chars().next(), c))
        .filter(|(d, _, c)| *d <= (name.len().max(c.len()) / 3).max(1) + 1 && *d < name.len().max(2))
        .min();
    match best {
        Some((_, _, c)) => format!("; did you mean `{c}`?"),
        None => String::new(),
    }
}

// ------------------------------------------------------------------ props comments

struct Comment {
    start: usize,
    end: usize,
    kind: &'static str, // "props" | "vars"
    body: String,
}
fn find_comments(src: &str) -> Vec<Comment> {
    let mut out = vec![];
    let mut i = 0;
    while let Some(s) = src[i..].find("{#") {
        let start = i + s;
        let Some(e) = src[start..].find("#}") else { break };
        let end = start + e + 2;
        let body = src[start + 2..end - 2].trim().trim_matches('-').trim();
        for kind in ["props", "vars"] {
            if let Some(rest) = body.strip_prefix(kind).and_then(|r| r.trim_start().strip_prefix(':')) {
                out.push(Comment { start, end, kind, body: rest.trim().to_string() });
            }
        }
        i = end;
    }
    out
}
fn split_top(s: &str) -> Vec<String> {
    let (mut depth, mut cur, mut out) = (0i32, String::new(), vec![]);
    for c in s.chars() {
        match c {
            '<' | '(' => depth += 1,
            '>' | ')' => depth -= 1,
            ',' if depth == 0 => {
                out.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

fn line_col(src: &str, off: usize) -> (u32, u32) {
    let before = &src[..off.min(src.len())];
    let line = before.matches('\n').count() as u32 + 1;
    let col = before.rsplit('\n').next().unwrap_or("").chars().count() as u32 + 1;
    (line, col)
}

// ------------------------------------------------------------------ checker

type Stmt = ast::Stmt<'static>;
type Expr = ast::Expr<'static>;

pub struct Checker {
    src: &'static BTreeMap<String, String>,
    asts: &'static BTreeMap<String, Stmt>,
    macros: BTreeMap<String, BTreeMap<String, Rc<MacroSig>>>,
    vars: BTreeMap<String, (Vec<Prop>, usize)>,
    named: BTreeMap<String, Ty>,
    globals: Vec<(String, Ty)>,
    pub diags: Vec<Diag>,
    seen: HashSet<String>,
    // walk state
    files: Vec<String>,
    scopes: Vec<HashMap<String, Bind>>,
    narrowed: Vec<String>,
    in_macro: Option<Rc<MacroSig>>,
    in_block: usize,
    overrides: Vec<(String, HashMap<&'static str, &'static ast::Spanned<ast::Block<'static>>>)>,
    seen_blocks: HashSet<String>,
    route: String,
    /// number of truthiness tests on a possibly-undefined value (legal under SemiStrict, an error under Strict)
    pub guard_sites: HashSet<String>,
}

pub fn syntax() -> SyntaxConfig {
    SyntaxConfig::builder().trim_blocks(true).lstrip_blocks(true).build().unwrap()
}

impl Checker {
    pub fn new(sources: BTreeMap<String, String>, globals: Vec<(String, Ty)>, named: BTreeMap<String, Ty>) -> Checker {
        // Spike shortcut: leak sources and ASTs so every AST reference is 'static.
        let src: &'static BTreeMap<String, String> = Box::leak(Box::new(sources));
        let mut asts = BTreeMap::new();
        let mut diags = vec![];
        for (name, s) in src.iter() {
            match parse(s, name, syntax()) {
                Ok(a) => {
                    asts.insert(name.clone(), a);
                }
                Err(e) => {
                    let (line, col) = match e.range() {
                        Some(r) => line_col(s, r.start),
                        None => (e.line().unwrap_or(1) as u32, 1),
                    };
                    diags.push(Diag { file: name.clone(), line, col, msg: format!("syntax error: {}", e.detail().unwrap_or("invalid template")), warn: false });
                }
            }
        }
        let asts: &'static BTreeMap<String, Stmt> = Box::leak(Box::new(asts));
        let mut c = Checker {
            src,
            asts,
            macros: BTreeMap::new(),
            vars: BTreeMap::new(),
            named,
            globals,
            diags,
            seen: HashSet::new(),
            files: vec![],
            scopes: vec![],
            narrowed: vec![],
            in_macro: None,
            in_block: 0,
            overrides: vec![],
            seen_blocks: HashSet::new(),
            route: String::new(),
            guard_sites: HashSet::new(),
        };
        c.collect_signatures();
        c
    }

    fn children(&self, name: &str) -> Option<&'static [Stmt]> {
        match self.asts.get(name) {
            Some(ast::Stmt::Template(t)) => Some(&t.children),
            _ => None,
        }
    }

    fn err_at(&mut self, file: &str, line: u32, col: u32, msg: String, warn: bool) {
        let d = Diag { file: file.to_string(), line, col, msg, warn };
        if self.seen.insert(d.to_string()) {
            self.diags.push(d);
        }
    }
    fn err(&mut self, span: Span, msg: String) {
        let f = self.files.last().cloned().unwrap_or_default();
        self.err_at(&f, span.start_line as u32, span.start_col as u32 + 1, msg, false);
    }
    fn err_end(&mut self, span: Span, back: usize, msg: String) {
        // point at the last `back` characters of the span (the attribute name of `a.b.c`)
        let f = self.files.last().cloned().unwrap_or_default();
        let (line, col) = line_col(&self.src[&f], (span.end_offset as usize).saturating_sub(back));
        self.err_at(&f, line, col, msg, false);
    }
    fn warn(&mut self, span: Span, msg: String) {
        let f = self.files.last().cloned().unwrap_or_default();
        self.err_at(&f, span.start_line as u32, span.start_col as u32 + 1, msg, true);
    }
    fn text(&self, span: Span) -> String {
        let f = self.files.last().unwrap();
        self.src[f][span.start_offset as usize..span.end_offset as usize].to_string()
    }

    // ---- pass 1: macro signatures, props comments, layout `vars` headers

    fn parse_ty(&self, s: &str) -> Result<Ty, String> {
        let s = s.trim();
        Ok(match s {
            "string" => Ty::Str,
            "int" => Ty::Int,
            "float" => Ty::Float,
            "bool" => Ty::Bool,
            "date" => Ty::Date,
            "datetime" => Ty::DateTime,
            "url" => Ty::Url,
            "image" => Ty::Image,
            "slot" => Ty::Slot,
            "attrs" => Ty::Attrs,
            "object" => Ty::Any,
            "enum" => Ty::Enum(vec![]),
            _ => {
                if let Some(inner) = s.strip_prefix("list<").and_then(|r| r.strip_suffix('>')) {
                    Ty::List(Box::new(self.parse_ty(inner)?))
                } else if let Some(inner) = s.strip_prefix("enum(").and_then(|r| r.strip_suffix(')')) {
                    Ty::Enum(inner.split('|').map(|x| x.trim().to_string()).collect())
                } else if let Some(t) = self.named.get(s) {
                    t.clone()
                } else {
                    return Err(format!("unknown prop type `{s}`{}", dym(s, TYPE_NAMES.iter().copied().chain(self.named.keys().map(|k| k.as_str())))));
                }
            }
        })
    }

    fn parse_props(&mut self, file: &str, c: &Comment) -> Vec<Prop> {
        let mut out = vec![];
        let (line, col) = line_col(&self.src[file], c.start);
        for part in split_top(&c.body) {
            let Some((name, ty)) = part.split_once(':') else {
                self.err_at(file, line, col, format!("malformed {} entry `{}` (expected `name: type` or `name?: type`)", c.kind, part.trim()), false);
                continue;
            };
            let name = name.trim();
            let (name, optional) = match name.strip_suffix('?') {
                Some(n) => (n.trim(), true),
                None => (name, false),
            };
            match self.parse_ty(ty) {
                Ok(ty) => out.push(Prop { name: name.to_string(), ty, optional }),
                Err(m) => {
                    self.err_at(file, line, col, format!("{m} for `{name}`"), false);
                    out.push(Prop { name: name.to_string(), ty: Ty::Any, optional });
                }
            }
        }
        out
    }

    fn collect_signatures(&mut self) {
        let names: Vec<String> = self.asts.keys().cloned().collect();
        for file in names {
            let src = &self.src[&file];
            let comments = find_comments(src);
            let mut used = vec![false; comments.len()];
            let mut sigs = BTreeMap::new();
            for (i, c) in comments.iter().enumerate() {
                if c.kind == "vars" {
                    used[i] = true;
                    let props = self.parse_props(&file, c);
                    self.vars.insert(file.clone(), (props, c.start));
                }
            }
            for s in self.children(&file).unwrap() {
                let ast::Stmt::Macro(m) = s else { continue };
                // Comments are dropped by the lexer: pair the props comment with this macro by offset.
                // The Macro span starts at the `macro` keyword, so allow only whitespace and the tag opener between.
                let start = m.span().start_offset as usize;
                let mut props = None;
                for (i, c) in comments.iter().enumerate() {
                    if c.kind == "props" && c.end <= start && src[c.end..start].chars().all(|ch| ch.is_whitespace() || "{%-".contains(ch)) {
                        used[i] = true;
                        props = Some(self.parse_props(&file, c));
                    }
                }
                let n_def = m.defaults.len();
                let params: Vec<(String, bool)> = m
                    .args
                    .iter()
                    .enumerate()
                    .map(|(i, a)| (if let ast::Expr::Var(v) = a { v.id.to_string() } else { "?".into() }, i + n_def >= m.args.len()))
                    .collect();
                let (line, col) = (m.span().start_line as u32, m.span().start_col as u32 + 1);
                match &props {
                    None if file.starts_with("components/") => {
                        self.err_at(&file, line, col, format!("component macro `{}` has no `{{# props: … #}}` comment directly above it", m.name), false)
                    }
                    Some(ps) => {
                        for (p, _) in &params {
                            if !ps.iter().any(|x| &x.name == p) {
                                let hint = dym(p, ps.iter().map(|x| x.name.as_str()));
                                self.err_at(&file, line, col, format!("macro `{}` has parameter `{p}` that the props comment does not declare{hint}", m.name), false);
                            }
                        }
                        for p in ps {
                            if !params.iter().any(|(n, _)| n == &p.name) {
                                let hint = dym(&p.name, params.iter().map(|x| x.0.as_str()));
                                self.err_at(&file, line, col, format!("props comment declares `{}` but macro `{}` has no such parameter{hint}", p.name, m.name), false);
                            }
                        }
                        for (p, has_default) in &params {
                            if let Some(x) = ps.iter().find(|x| &x.name == p) {
                                if !x.optional && *has_default {
                                    self.err_at(&file, line, col, format!("prop `{p}` of `{}` is declared required but the macro gives it a default", m.name), true);
                                }
                            }
                        }
                    }
                    _ => {}
                }
                let slot = slot_use(&m.body);
                sigs.insert(m.name.to_string(), Rc::new(MacroSig { name: m.name.to_string(), file: file.clone(), params, props, slot }));
            }
            for (i, c) in comments.iter().enumerate() {
                if !used[i] {
                    let (line, col) = line_col(src, c.start);
                    self.err_at(&file, line, col, "`props:` comment is not directly above a `{% macro %}`".into(), false);
                }
            }
            self.macros.insert(file.clone(), sigs);
        }
    }

    // ---- pass 2: macro bodies (props + always-present globals only)

    pub fn check_macro_bodies(&mut self) {
        let files: Vec<String> = self.macros.iter().filter(|(_, m)| !m.is_empty()).map(|(f, _)| f.clone()).collect();
        for file in files {
            self.reset();
            self.files.push(file.clone());
            self.scopes.push(HashMap::new()); // file top-level: imports, sets, sibling macros
            let children = self.children(&file).unwrap();
            for s in children {
                match s {
                    ast::Stmt::Import(_) | ast::Stmt::FromImport(_) | ast::Stmt::Set(_) | ast::Stmt::SetBlock(_) => self.stmt(s),
                    ast::Stmt::Macro(m) => {
                        if let Some(sig) = self.macros[&file].get(m.name).cloned() {
                            self.bind(m.name, Ty::Macro(sig), false);
                        }
                    }
                    _ => {}
                }
            }
            for s in children {
                let ast::Stmt::Macro(m) = s else { continue };
                let sig = self.macros[&file][m.name].clone();
                for d in &m.defaults {
                    self.expr(d, Mode::Pass);
                }
                self.scopes.push(HashMap::new());
                for (i, (p, has_default)) in sig.params.iter().enumerate() {
                    let prop = sig.props.as_ref().and_then(|ps| ps.iter().find(|x| &x.name == p));
                    // a default of `none` still leaves the prop absent for printing purposes
                    let di = (i + m.defaults.len()).checked_sub(m.args.len());
                    let none_default = di.map(|j| matches!(&m.defaults[j], ast::Expr::Const(c) if c.value.is_none())).unwrap_or(false);
                    let optional = match prop {
                        Some(pr) => pr.optional && (!has_default || none_default),
                        None => false, // no props comment (a local helper macro): untyped, unchecked
                    };
                    self.bind(p, prop.map(|x| x.ty.clone()).unwrap_or(Ty::Any), optional);
                }
                self.in_macro = Some(sig);
                self.body(&m.body);
                self.in_macro = None;
                self.scopes.pop();
            }
        }
    }

    // ---- pass 3: routes

    fn reset(&mut self) {
        self.files.clear();
        self.narrowed.clear();
        self.overrides.clear();
        self.seen_blocks.clear();
        self.in_block = 0;
        self.in_macro = None;
        let g: HashMap<String, Bind> = self.globals.iter().map(|(n, t)| (n.clone(), Bind { ty: t.clone(), optional: false, lit: None })).collect();
        self.scopes = vec![g];
    }

    pub fn check_route(&mut self, kind: &RouteKind) {
        self.reset();
        self.route = kind.name.clone();
        let vars = kind.vars.iter().map(|(n, t, o)| (n.clone(), Bind { ty: t.clone(), optional: *o, lit: None })).collect();
        self.scopes.push(vars);
        self.scopes.push(HashMap::new());
        if !self.asts.contains_key(&kind.template) {
            if !self.src.contains_key(&kind.template) {
                self.err_at(&kind.template, 1, 1, format!("template for route kind {} not found", kind.name), false);
            }
            return;
        }
        self.walk_template(&kind.template);
        let ov = std::mem::take(&mut self.overrides);
        for (file, blocks) in &ov {
            for (name, b) in blocks {
                if !self.seen_blocks.contains(*name) {
                    let all: Vec<String> = self.seen_blocks.iter().cloned().collect();
                    let hint = dym(name, all.iter().map(|s| s.as_str()));
                    self.err_at(file, b.span().start_line as u32, b.span().start_col as u32 + 1, format!("block `{name}` does not exist in any layout this template extends{hint}"), false);
                }
            }
        }
    }

    fn const_name(&mut self, e: &Expr, what: &str) -> Vec<String> {
        match e {
            ast::Expr::Const(c) if c.value.as_str().is_some() => vec![c.value.as_str().unwrap().to_string()],
            ast::Expr::List(l) if l.items.iter().all(|i| matches!(i, ast::Expr::Const(c) if c.value.as_str().is_some())) => {
                l.items.iter().filter_map(|i| i.as_const()).map(|v| v.to_string()).collect()
            }
            _ => {
                let t = self.text(e.span());
                self.err(e.span(), format!("{what} name must be a constant string, found the expression `{t}` (template names are resolved at check time)"));
                self.expr(e, Mode::Use);
                vec![]
            }
        }
    }
    fn missing_template(&mut self, span: Span, name: &str) {
        let hint = dym(name, self.src.keys().map(|k| k.as_str()));
        self.err(span, format!("template `{name}` not found{hint}"));
    }

    fn walk_template(&mut self, name: &str) {
        let Some(children) = self.children(name) else { return };
        if self.files.iter().filter(|f| *f == name).count() > 2 {
            return; // recursion guard
        }
        self.files.push(name.to_string());
        let extends = children.iter().find_map(|s| if let ast::Stmt::Extends(e) = s { Some(e) } else { None });
        if let Some(ext) = extends {
            let mut blocks = HashMap::new();
            for s in children {
                match s {
                    ast::Stmt::Block(b) => {
                        blocks.insert(b.name, b);
                    }
                    ast::Stmt::Extends(_) | ast::Stmt::EmitRaw(_) => {}
                    other => self.stmt(other),
                }
            }
            let parents = self.const_name(&ext.name, "extends");
            self.overrides.push((name.to_string(), blocks));
            for p in parents {
                if self.asts.contains_key(&p) {
                    self.walk_template(&p);
                } else if !self.src.contains_key(&p) {
                    self.missing_template(ext.name.span(), &p);
                }
            }
        } else {
            if let Some((vars, off)) = self.vars.get(name).cloned() {
                let (line, col) = line_col(&self.src[name], off);
                for v in vars {
                    let found = self.scopes[2..].iter().rev().find_map(|s| s.get(&v.name)).cloned();
                    match found {
                        Some(b) => {
                            let route = self.route.clone();
                            if !assignable(&b.ty, &v.ty) {
                                self.err_at(name, line, col, format!("layout variable `{}` expects {}, but route {route} sets it to {}", v.name, v.ty, b.ty), false);
                            } else if let (Ty::Enum(vals), Some(lit)) = (&v.ty, &b.lit) {
                                if !vals.is_empty() && !vals.contains(lit) {
                                    let hint = dym(lit, vals.iter().map(|s| s.as_str()));
                                    self.err_at(name, line, col, format!("layout variable `{}` is set to \"{lit}\" by route {route}, which is not one of {}{hint}", v.name, vals.join("|")), false);
                                }
                            }
                            if b.optional && !v.optional {
                                self.err_at(name, line, col, format!("layout variable `{}` is required, but route {route} sets it from an optional value", v.name), false);
                            }
                        }
                        None if v.optional => self.bind(&v.name, v.ty.clone(), true),
                        None => {
                            self.err_at(name, line, col, format!("layout variable `{}` is required, but route {} never sets it (`{{% set {} = … %}}` before the block)", v.name, self.route, v.name), false);
                            self.bind(&v.name, v.ty.clone(), false);
                        }
                    }
                }
            }
            self.body(children);
        }
        self.files.pop();
    }

    fn bind(&mut self, name: &str, ty: Ty, optional: bool) {
        self.scopes.last_mut().unwrap().insert(name.to_string(), Bind { ty, optional, lit: None });
    }
    fn lookup(&self, name: &str) -> Option<Bind> {
        self.scopes.iter().rev().find_map(|s| s.get(name)).cloned()
    }

    fn body(&mut self, stmts: &'static [Stmt]) {
        for s in stmts {
            self.stmt(s);
        }
    }

    fn bind_target(&mut self, target: &Expr, ty: Ty, optional: bool, lit: Option<String>) {
        match target {
            ast::Expr::Var(v) => {
                self.narrowed.retain(|p| p != v.id && !p.starts_with(&format!("{}.", v.id)));
                self.scopes.last_mut().unwrap().insert(v.id.to_string(), Bind { ty, optional, lit });
            }
            ast::Expr::List(l) => {
                for i in &l.items {
                    self.bind_target(i, Ty::Any, false, None);
                }
            }
            ast::Expr::Tuple(l) => {
                for i in &l.items {
                    self.bind_target(i, Ty::Any, false, None);
                }
            }
            _ => {}
        }
    }

    fn stmt(&mut self, s: &'static Stmt) {
        match s {
            ast::Stmt::Template(t) => self.body(&t.children),
            ast::Stmt::EmitRaw(_) | ast::Stmt::Continue(_) | ast::Stmt::Break(_) | ast::Stmt::Extends(_) => {}
            ast::Stmt::EmitExpr(e) => {
                self.expr(&e.expr, Mode::Use);
            }
            ast::Stmt::ForLoop(f) => {
                let (it, _) = self.expr(&f.iter, Mode::Use);
                let elem = match it {
                    Ty::List(t) => *t,
                    Ty::Any | Ty::Obj(_) | Ty::Attrs => Ty::Any,
                    other => {
                        let t = self.text(f.iter.span());
                        self.err(f.iter.span(), format!("`{t}` is {other}, which cannot be looped over"));
                        Ty::Any
                    }
                };
                self.scopes.push(HashMap::new());
                self.bind_target(&f.target, elem, false, None);
                self.bind("loop", Ty::Any, false);
                let mark = self.narrowed.len();
                if let Some(fe) = &f.filter_expr {
                    let n = self.cond(fe);
                    self.narrowed.extend(n);
                }
                self.body(&f.body);
                self.narrowed.truncate(mark);
                self.scopes.pop();
                self.body(&f.else_body);
            }
            ast::Stmt::IfCond(i) => {
                let n = self.cond(&i.expr);
                let mark = self.narrowed.len();
                self.narrowed.extend(n);
                self.body(&i.true_body);
                self.narrowed.truncate(mark);
                self.narrowed.extend(Self::narrows_neg(&i.expr));
                self.body(&i.false_body);
                self.narrowed.truncate(mark);
            }
            ast::Stmt::WithBlock(w) => {
                self.scopes.push(HashMap::new());
                for (target, val) in &w.assignments {
                    let (t, o) = self.expr(val, Mode::Pass);
                    self.bind_target(target, t, o, None);
                }
                self.body(&w.body);
                self.scopes.pop();
            }
            ast::Stmt::Set(st) => {
                let (t, o) = self.expr(&st.expr, Mode::Pass);
                let lit = if let ast::Expr::Const(c) = &st.expr { c.value.as_str().map(|s| s.to_string()) } else { None };
                self.bind_target(&st.target, t, o, lit);
            }
            ast::Stmt::SetBlock(sb) => {
                if let Some(f) = &sb.filter {
                    self.expr(f, Mode::Use);
                }
                self.body(&sb.body);
                self.bind_target(&sb.target, Ty::Slot, false, None);
            }
            ast::Stmt::AutoEscape(a) => {
                self.expr(&a.enabled, Mode::Use);
                self.body(&a.body);
            }
            ast::Stmt::FilterBlock(f) => {
                self.expr(&f.filter, Mode::Use);
                self.body(&f.body);
            }
            ast::Stmt::Block(b) => {
                self.seen_blocks.insert(b.name.to_string());
                self.in_block += 1;
                self.scopes.push(HashMap::new());
                self.body(&b.body);
                self.scopes.pop();
                let ov: Vec<(String, &'static ast::Spanned<ast::Block<'static>>)> =
                    self.overrides.iter().filter_map(|(f, m)| m.get(b.name).map(|x| (f.clone(), *x))).collect();
                for (file, blk) in ov {
                    if std::ptr::eq(blk, b) {
                        continue;
                    }
                    self.files.push(file);
                    self.scopes.push(HashMap::new());
                    self.body(&blk.body);
                    self.scopes.pop();
                    self.files.pop();
                }
                self.in_block -= 1;
            }
            ast::Stmt::Include(inc) => {
                for name in self.const_name(&inc.name, "include") {
                    if self.asts.contains_key(&name) {
                        // an include is checked in the includer's context: same scopes, new file
                        self.scopes.push(HashMap::new());
                        self.walk_template(&name);
                        self.scopes.pop();
                    } else if !self.src.contains_key(&name) && !inc.ignore_missing {
                        self.missing_template(inc.name.span(), &name);
                    }
                }
            }
            ast::Stmt::Import(imp) => {
                for name in self.const_name(&imp.expr, "import") {
                    if self.macros.contains_key(&name) {
                        self.bind_target(&imp.name, Ty::Ns(name), false, None);
                    } else {
                        self.missing_template(imp.expr.span(), &name);
                        self.bind_target(&imp.name, Ty::Any, false, None);
                    }
                }
            }
            ast::Stmt::FromImport(imp) => {
                let names = self.const_name(&imp.expr, "import");
                let file = names.first().cloned();
                let known = file.as_ref().map(|f| self.macros.contains_key(f)).unwrap_or(false);
                if let (Some(f), false) = (&file, known) {
                    self.missing_template(imp.expr.span(), f);
                }
                for (name, alias) in &imp.names {
                    let ast::Expr::Var(v) = name else { continue };
                    let target = alias.as_ref().unwrap_or(name);
                    let sig = file.as_ref().and_then(|f| self.macros.get(f)).and_then(|m| m.get(v.id)).cloned();
                    match sig {
                        Some(sig) => self.bind_target(target, Ty::Macro(sig), false, None),
                        None => {
                            if known {
                                let f = file.clone().unwrap();
                                let hint = dym(v.id, self.macros[&f].keys().map(|k| k.as_str()));
                                self.err(v.span(), format!("`{f}` defines no macro `{}`{hint}", v.id));
                            }
                            self.bind_target(target, Ty::Any, false, None);
                        }
                    }
                }
            }
            ast::Stmt::Macro(m) => {
                let file = self.files.last().cloned().unwrap();
                match self.macros.get(&file).and_then(|x| x.get(m.name)).cloned() {
                    Some(sig) => self.bind(m.name, Ty::Macro(sig), false),
                    None => self.bind(m.name, Ty::Any, false),
                }
            }
            ast::Stmt::CallBlock(cb) => {
                self.call(&cb.call, cb.call.span(), true);
                self.scopes.push(HashMap::new());
                for a in &cb.macro_decl.args {
                    self.bind_target(a, Ty::Any, false, None);
                }
                self.body(&cb.macro_decl.body);
                self.scopes.pop();
            }
            ast::Stmt::Do(d) => {
                self.call(&d.call, d.call.span(), false);
            }
        }
    }

    // ---- expressions

    fn path_of(e: &Expr) -> Option<String> {
        match e {
            ast::Expr::Var(v) => Some(v.id.to_string()),
            ast::Expr::GetAttr(g) => Self::path_of(&g.expr).map(|p| format!("{p}.{}", g.name)),
            ast::Expr::GetItem(g) => match &g.subscript_expr {
                ast::Expr::Const(c) if c.value.as_str().is_some() => Self::path_of(&g.expr).map(|p| format!("{p}.{}", c.value.as_str().unwrap())),
                _ => None,
            },
            _ => None,
        }
    }
    fn narrows(e: &Expr) -> Vec<String> {
        match e {
            ast::Expr::Var(_) | ast::Expr::GetAttr(_) | ast::Expr::GetItem(_) => Self::path_of(e).into_iter().collect(),
            ast::Expr::BinOp(b) if matches!(b.op, ast::BinOpKind::ScAnd) => {
                let mut v = Self::narrows(&b.left);
                v.extend(Self::narrows(&b.right));
                v
            }
            ast::Expr::Test(t) if t.name == "defined" => Self::path_of(&t.expr).into_iter().collect(),
            ast::Expr::UnaryOp(u) if matches!(u.op, ast::UnaryOpKind::Not) => Self::narrows_neg(&u.expr),
            _ => vec![],
        }
    }
    /// Paths proven defined when the condition is false (the `else` branch).
    fn narrows_neg(e: &Expr) -> Vec<String> {
        match e {
            ast::Expr::UnaryOp(u) if matches!(u.op, ast::UnaryOpKind::Not) => Self::narrows(&u.expr),
            ast::Expr::Test(t) if t.name == "undefined" => Self::path_of(&t.expr).into_iter().collect(),
            ast::Expr::BinOp(b) if matches!(b.op, ast::BinOpKind::ScOr) => {
                let mut v = Self::narrows_neg(&b.left);
                v.extend(Self::narrows_neg(&b.right));
                v
            }
            _ => vec![],
        }
    }
    /// Checks a condition (truthiness) and returns the paths it proves defined.
    fn cond(&mut self, e: &Expr) -> Vec<String> {
        self.expr(e, Mode::Guard);
        Self::narrows(e)
    }
    fn is_narrowed(&self, e: &Expr) -> bool {
        Self::path_of(e).map(|p| self.narrowed.contains(&p)).unwrap_or(false)
    }

    fn expr(&mut self, e: &Expr, mode: Mode) -> (Ty, bool) {
        let (t, opt) = self.infer(e);
        if opt {
            match mode {
                Mode::Use => {
                    let txt = Self::path_of(e).unwrap_or_else(|| self.text(e.span()));
                    self.err(
                        e.span(),
                        format!("`{txt}` is optional and may be undefined here; wrap it in `{{% if {txt} %}}` or give it `| default(…)`"),
                    );
                }
                Mode::Guard => {
                    let f = self.files.last().cloned().unwrap_or_default();
                    self.guard_sites.insert(format!("{f}:{}:{}", e.span().start_line, e.span().start_col + 1));
                }
                Mode::Pass => {}
            }
        }
        (t, opt && mode != Mode::Use)
    }

    fn field(&mut self, e: &Expr, base: &Expr, bt: &Ty, name: &str) -> (Ty, bool) {
        match bt {
            Ty::Obj(o) => match o.fields.get(name) {
                Some(f) => (f.ty.clone(), f.optional && !self.is_narrowed(e)),
                None if o.open => (Ty::Any, false),
                None => {
                    let on = Self::path_of(base).unwrap_or_else(|| "value".into());
                    let mut hint = dym(name, o.fields.keys().map(|k| k.as_str()));
                    if hint.is_empty() && o.fields.len() <= 8 {
                        hint = format!("; it has {}", o.fields.keys().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", "));
                    }
                    self.err_end(e.span(), name.len(), format!("unknown field `{name}` on `{on}` ({}){hint}", o.name));
                    (Ty::Any, false)
                }
            },
            Ty::Any | Ty::Attrs | Ty::Macro(_) | Ty::Ns(_) | Ty::None => (Ty::Any, false),
            other => {
                let on = Self::path_of(base).unwrap_or_else(|| "value".into());
                self.err_end(e.span(), name.len(), format!("`{on}` is {other}; it has no field `{name}`"));
                (Ty::Any, false)
            }
        }
    }

    fn infer(&mut self, e: &Expr) -> (Ty, bool) {
        match e {
            ast::Expr::Const(c) => (
                match c.value.kind() {
                    ValueKind::String => Ty::Str,
                    ValueKind::Bool => Ty::Bool,
                    ValueKind::Number => {
                        if c.value.is_integer() {
                            Ty::Int
                        } else {
                            Ty::Float
                        }
                    }
                    ValueKind::None => Ty::None,
                    _ => Ty::Any,
                },
                false,
            ),
            ast::Expr::Var(v) => match self.lookup(v.id) {
                Some(b) => (b.ty, b.optional && !self.narrowed.iter().any(|p| p == v.id)),
                None if v.id == "caller" && self.in_macro.is_some() => (Ty::Any, true),
                None if v.id == "self" || v.id == "loop" => (Ty::Any, false),
                None => {
                    let skip = if self.in_macro.is_some() { 1 } else { 0 };
                    let names: Vec<String> = self.scopes.iter().skip(skip).flat_map(|s| s.keys().cloned()).collect();
                    let hint = dym(v.id, names.iter().map(|s| s.as_str()));
                    match self.in_macro.clone() {
                        Some(m) => self.err(
                            v.span(),
                            format!("`{}` is not a prop of macro `{}`; a macro body may only read its props and the globals site, data, collections, taxonomies, page (pass it in as a prop){hint}", v.id, m.name),
                        ),
                        None => self.err(v.span(), format!("unknown variable `{}` for route kind {}{hint}", v.id, self.route)),
                    }
                    (Ty::Any, false)
                }
            },
            ast::Expr::GetAttr(g) => {
                let (bt, bopt) = self.infer(&g.expr);
                if bopt {
                    let bt_txt = Self::path_of(&g.expr).unwrap_or_else(|| self.text(g.expr.span()));
                    let full = Self::path_of(e).unwrap_or_else(|| self.text(e.span()));
                    self.err(g.expr.span(), format!("`{bt_txt}` is optional, so `{full}` reaches through a value that may be undefined; guard with `{{% if {bt_txt} %}}`"));
                }
                self.field(e, &g.expr, &bt, g.name)
            }
            ast::Expr::GetItem(g) => {
                let (bt, bopt) = self.infer(&g.expr);
                if bopt {
                    let t = self.text(g.expr.span());
                    self.err(g.expr.span(), format!("`{t}` is optional and is subscripted unguarded"));
                }
                match &g.subscript_expr {
                    ast::Expr::Const(c) if c.value.as_str().is_some() => self.field(e, &g.expr, &bt, c.value.as_str().unwrap()),
                    sub => {
                        self.expr(sub, Mode::Use);
                        match bt {
                            Ty::List(t) => (*t, false),
                            Ty::Obj(o) if !o.open => {
                                let t = self.text(e.span());
                                self.warn(e.span(), format!("`{t}` uses a non-constant key on {}; it cannot be checked", o.name));
                                (Ty::Any, false)
                            }
                            _ => (Ty::Any, false),
                        }
                    }
                }
            }
            ast::Expr::Slice(s) => {
                let (t, _) = self.expr(&s.expr, Mode::Use);
                for p in [&s.start, &s.stop, &s.step].into_iter().flatten() {
                    self.expr(p, Mode::Use);
                }
                (t, false)
            }
            ast::Expr::UnaryOp(u) => match u.op {
                ast::UnaryOpKind::Not => {
                    self.expr(&u.expr, Mode::Guard);
                    (Ty::Bool, false)
                }
                ast::UnaryOpKind::Neg => self.expr(&u.expr, Mode::Use),
            },
            ast::Expr::BinOp(b) => match b.op {
                ast::BinOpKind::ScAnd => {
                    let n = self.cond(&b.left);
                    let mark = self.narrowed.len();
                    self.narrowed.extend(n);
                    let (rt, _) = self.expr(&b.right, Mode::Guard);
                    self.narrowed.truncate(mark);
                    (rt, false)
                }
                ast::BinOpKind::ScOr => {
                    let (lt, _) = self.expr(&b.left, Mode::Guard);
                    let (rt, ropt) = self.infer(&b.right);
                    (if matches!(rt, Ty::Any) { lt } else { rt }, ropt)
                }
                ast::BinOpKind::Concat => {
                    self.expr(&b.left, Mode::Use);
                    self.expr(&b.right, Mode::Use);
                    (Ty::Str, false)
                }
                ast::BinOpKind::Eq | ast::BinOpKind::Ne | ast::BinOpKind::Lt | ast::BinOpKind::Lte | ast::BinOpKind::Gt | ast::BinOpKind::Gte | ast::BinOpKind::In => {
                    self.expr(&b.left, Mode::Use);
                    self.expr(&b.right, Mode::Use);
                    (Ty::Bool, false)
                }
                _ => {
                    let (l, _) = self.expr(&b.left, Mode::Use);
                    let (r, _) = self.expr(&b.right, Mode::Use);
                    match (l, r) {
                        (Ty::List(a), Ty::List(b2)) => (Ty::List(if assignable(&b2, &a) { a } else { Box::new(Ty::Any) }), false),
                        (Ty::Int, Ty::Int) => (Ty::Int, false),
                        (Ty::Str, Ty::Str) => (Ty::Str, false),
                        _ => (Ty::Any, false),
                    }
                }
            },
            ast::Expr::Compare(c) => {
                // probe: `==`/`!=` on an undefined value is an error even under SemiStrict
                self.expr(&c.expr, Mode::Use);
                for op in &c.ops {
                    self.expr(&op.expr, Mode::Use);
                }
                (Ty::Bool, false)
            }
            ast::Expr::IfExpr(i) => {
                let n = self.cond(&i.test_expr);
                let mark = self.narrowed.len();
                self.narrowed.extend(n);
                let (tt, topt) = self.infer(&i.true_expr);
                self.narrowed.truncate(mark);
                let (ft, fopt) = match &i.false_expr {
                    Some(f) => self.infer(f),
                    // probe: `{{ a if b }}` prints empty under SemiStrict when b is false
                    None => (Ty::None, false),
                };
                let ty = if matches!(ft, Ty::None) { tt } else if matches!(tt, Ty::None) { ft } else if assignable(&ft, &tt) { tt } else { Ty::Any };
                (ty, topt || fopt)
            }
            ast::Expr::Filter(f) => self.filter(e, f),
            ast::Expr::Test(t) => {
                if !TESTS.contains(&t.name) {
                    let hint = dym(t.name, TESTS.iter().copied());
                    self.err(e.span(), format!("unknown test `{}`{hint}", t.name));
                }
                let mode = if matches!(t.name, "defined" | "undefined" | "none") { Mode::Pass } else { Mode::Use };
                self.expr(&t.expr, mode);
                self.args(&t.args);
                (Ty::Bool, false)
            }
            ast::Expr::Call(c) => self.call(c, e.span(), false),
            ast::Expr::List(l) => {
                let mut ty = Ty::Any;
                for (i, it) in l.items.iter().enumerate() {
                    let (t, _) = self.expr(it, Mode::Use);
                    if i == 0 {
                        ty = t;
                    }
                }
                (Ty::List(Box::new(ty)), false)
            }
            ast::Expr::Tuple(l) => {
                for it in &l.items {
                    self.expr(it, Mode::Use);
                }
                (Ty::Any, false)
            }
            ast::Expr::Map(m) => {
                // values may be undefined: the map just carries them (the `attrs` filter drops them)
                let mut fields = BTreeMap::new();
                for (k, v) in m.keys.iter().zip(m.values.iter()) {
                    let (t, o) = self.expr(v, Mode::Pass);
                    if let ast::Expr::Const(c) = k {
                        if let Some(s) = c.value.as_str() {
                            fields.insert(s.to_string(), Field { ty: t, optional: o });
                        }
                    } else {
                        self.expr(k, Mode::Use);
                    }
                }
                (Ty::Obj(Rc::new(Obj { name: "map literal".into(), fields, open: true })), false)
            }
        }
    }

    fn args(&mut self, args: &[ast::CallArg<'static>]) -> Vec<(Ty, bool)> {
        args.iter()
            .map(|a| match a {
                ast::CallArg::Pos(e) | ast::CallArg::Kwarg(_, e) | ast::CallArg::PosSplat(e) | ast::CallArg::KwargSplat(e) => self.expr(e, Mode::Pass),
            })
            .collect()
    }

    fn check_key(&mut self, span: Span, filter: &str, input: &Ty, key: &str) {
        let k = key.trim().trim_start_matches('-');
        let first = k.split('.').next().unwrap_or(k);
        if let Ty::List(inner) = input {
            if let Ty::Obj(o) = &**inner {
                if !o.open && !o.fields.contains_key(first) {
                    let hint = dym(first, o.fields.keys().map(|x| x.as_str()));
                    self.err(span, format!("`{filter}` key `{first}` is not a field of {}{hint}", o.name));
                }
            }
        }
    }

    fn filter(&mut self, e: &Expr, f: &ast::Filter<'static>) -> (Ty, bool) {
        if !FILTERS.contains(&f.name) {
            let hint = dym(f.name, FILTERS.iter().copied());
            self.err_end(e.span(), 0, format!("unknown filter `{}`{hint}", f.name));
        }
        let is_default = matches!(f.name, "default" | "d");
        let (it, iopt) = match &f.expr {
            Some(inner) => self.expr(inner, if is_default { Mode::Pass } else { Mode::Use }),
            None => (Ty::Any, false),
        };
        let argt = self.args(&f.args);
        let const_arg = |i: usize, kw: &str| -> Option<&Expr> {
            let mut pos = 0;
            for a in &f.args {
                match a {
                    ast::CallArg::Pos(x) => {
                        if pos == i {
                            return Some(x);
                        }
                        pos += 1;
                    }
                    ast::CallArg::Kwarg(k, x) if *k == kw => return Some(x),
                    _ => {}
                }
            }
            None
        };
        match f.name {
            "default" | "d" => {
                let (at, aopt) = argt.first().cloned().unwrap_or((Ty::Str, false));
                let _ = iopt;
                (if matches!(it, Ty::Any | Ty::None) { at } else { it }, aopt)
            }
            "sort" => {
                if let Some(a) = const_arg(usize::MAX, "attribute").and_then(|x| x.as_const().map(|v| (x.span(), v.to_string()))) {
                    for key in a.1.split(',') {
                        if key.trim().starts_with('-') {
                            self.err(a.0, format!("`sort` has no per-key direction: `{}` is read as a field name and silently ignored; use `sort_by([\"{}\"])`", key.trim(), key.trim()));
                        } else {
                            self.check_key(a.0, "sort", &it, key);
                        }
                    }
                }
                (it, false)
            }
            "sort_by" => {
                match const_arg(0, "keys") {
                    Some(ast::Expr::List(l)) => {
                        for item in &l.items {
                            if let Some(v) = item.as_const() {
                                self.check_key(item.span(), "sort_by", &it, v.as_str().unwrap_or(""));
                            }
                        }
                    }
                    Some(other) if other.as_const().is_some() => {
                        self.err(other.span(), "`sort_by` takes a list of keys, e.g. `sort_by([\"order\", \"-modified\"])`".into())
                    }
                    _ => {}
                }
                (it, false)
            }
            "map" | "selectattr" | "rejectattr" | "groupby" => {
                let key = if f.name == "map" { const_arg(usize::MAX, "attribute") } else { const_arg(0, "attribute") };
                if let Some((sp, v)) = key.and_then(|x| x.as_const().map(|v| (x.span(), v))) {
                    if let Some(s) = v.as_str() {
                        self.check_key(sp, f.name, &it, s);
                    }
                }
                (if f.name == "map" || f.name == "groupby" { Ty::List(Box::new(Ty::Any)) } else { it }, false)
            }
            "lower" | "upper" | "trim" | "string" | "join" | "safe" | "escape" | "e" | "title" | "capitalize" | "replace" | "dateformat"
            | "datetimeformat" | "timeformat" | "urlencode" | "tojson" | "indent" | "truncate" | "striptags" | "pluralize" | "format"
            | "filesizeformat" => (Ty::Str, false),
            "attrs" => {
                if !assignable(&it, &Ty::Attrs) {
                    self.err(e.span(), format!("`attrs` expects a map of attributes, got {it}"));
                }
                (Ty::Slot, false)
            }
            "length" | "count" | "int" => (Ty::Int, false),
            "list" | "reverse" | "unique" | "select" | "reject" | "slice" => (it, false),
            "first" | "last" => match it {
                Ty::List(t) => (*t, false),
                _ => (Ty::Any, false),
            },
            _ => (Ty::Any, false),
        }
    }

    fn call(&mut self, c: &ast::Call<'static>, span: Span, has_caller: bool) -> (Ty, bool) {
        match c.identify_call() {
            ast::CallType::Function(name) => {
                match self.lookup(name) {
                    Some(Bind { ty: Ty::Macro(sig), .. }) => return self.macro_call(&sig, c, span, has_caller),
                    Some(_) => {
                        self.args(&c.args);
                        return (Ty::Any, false);
                    }
                    None => {}
                }
                self.args(&c.args);
                match name {
                    "caller" if self.in_macro.is_some() => (Ty::Slot, false),
                    "super" if self.in_block > 0 => (Ty::Slot, false),
                    "super" => {
                        self.err(span, "`super()` is only valid inside a `{% block %}`".into());
                        (Ty::Slot, false)
                    }
                    _ if FUNCTIONS.contains(&name) => (if matches!(name, "url" | "absolute_url" | "asset") { Ty::Url } else { Ty::Any }, false),
                    _ => {
                        let names: Vec<String> = self.scopes.iter().flat_map(|s| s.iter().filter(|(_, b)| matches!(b.ty, Ty::Macro(_))).map(|(k, _)| k.clone())).collect();
                        let hint = dym(name, FUNCTIONS.iter().copied().chain(names.iter().map(|s| s.as_str())));
                        self.err(span, format!("unknown function or macro `{name}`{hint}"));
                        (Ty::Any, false)
                    }
                }
            }
            ast::CallType::Method(base, name) => {
                if let ast::Expr::Var(v) = base {
                    if let Some(Bind { ty: Ty::Ns(file), .. }) = self.lookup(v.id) {
                        return match self.macros[&file].get(name).cloned() {
                            Some(sig) => self.macro_call(&sig, c, span, has_caller),
                            None => {
                                let hint = dym(name, self.macros[&file].keys().map(|k| k.as_str()));
                                self.err(span, format!("`{file}` (imported as `{}`) defines no macro `{name}`{hint}", v.id));
                                self.args(&c.args);
                                (Ty::Any, false)
                            }
                        };
                    }
                }
                let (bt, _) = self.expr(base, Mode::Use);
                self.args(&c.args);
                if !matches!(base, ast::Expr::Var(v) if v.id == "loop") && !matches!(bt, Ty::Any) {
                    let hint = if FILTERS.contains(&name) { format!("; use the filter `| {name}`") } else { String::new() };
                    self.err_end(span, 0, format!("`.{name}()` is a method call on {bt}; templates have no Python methods{hint}"));
                }
                (Ty::Any, false)
            }
            ast::CallType::Block(name) => {
                let _ = name;
                (Ty::Slot, false)
            }
            ast::CallType::Object(e) => {
                self.expr(e, Mode::Use);
                self.args(&c.args);
                (Ty::Any, false)
            }
        }
    }

    fn macro_call(&mut self, sig: &Rc<MacroSig>, c: &ast::Call<'static>, span: Span, has_caller: bool) -> (Ty, bool) {
        let mut given: HashSet<String> = HashSet::new();
        let mut pos = 0;
        let mut splat = false;
        for a in &c.args {
            let (name, e) = match a {
                ast::CallArg::Pos(e) => {
                    let n = sig.params.get(pos).map(|p| p.0.clone());
                    pos += 1;
                    if n.is_none() {
                        self.err(e.span(), format!("too many positional arguments for `{}` (it takes {})", sig.name, sig.params.len()));
                    }
                    (n, e)
                }
                ast::CallArg::Kwarg(k, e) => {
                    if !sig.params.iter().any(|p| p.0 == *k) {
                        let hint = dym(k, sig.params.iter().map(|p| p.0.as_str()));
                        let f = self.files.last().cloned().unwrap();
                        let off = (e.span().start_offset as usize).saturating_sub(k.len() + 1);
                        let (line, col) = line_col(&self.src[&f], off);
                        self.err_at(&f, line, col, format!("unknown prop `{k}` for `{}` ({}){hint}", sig.name, sig.file), false);
                        (None, e)
                    } else {
                        (Some(k.to_string()), e)
                    }
                }
                ast::CallArg::PosSplat(e) | ast::CallArg::KwargSplat(e) => {
                    splat = true;
                    self.warn(e.span(), format!("splat arguments to `{}` cannot be checked", sig.name));
                    (None, e)
                }
            };
            let (t, opt) = self.expr(e, Mode::Pass);
            let Some(name) = name else { continue };
            if !given.insert(name.clone()) {
                self.err(e.span(), format!("prop `{name}` of `{}` is passed twice", sig.name));
            }
            let Some(prop) = sig.props.as_ref().and_then(|ps| ps.iter().find(|p| p.name == name)).cloned() else { continue };
            let txt = self.text(e.span());
            if opt && !prop.optional {
                self.err(e.span(), format!("`{txt}` is optional but prop `{name}` of `{}` is required; guard the call or give it `| default(…)`", sig.name));
            }
            if !assignable(&t, &prop.ty) {
                self.err(e.span(), format!("prop `{name}` of `{}` expects {}, got {t} (`{txt}`)", sig.name, prop.ty));
            } else if let (Ty::Enum(vals), ast::Expr::Const(k)) = (&prop.ty, e) {
                if let Some(s) = k.value.as_str() {
                    if !vals.is_empty() && !vals.iter().any(|v| v == s) {
                        self.err(e.span(), format!("prop `{name}` of `{}` must be one of {}, got \"{s}\"{}", sig.name, vals.join("|"), dym(s, vals.iter().map(|v| v.as_str()))));
                    }
                }
            }
        }
        if !splat {
            for (p, has_default) in &sig.params {
                let required = match sig.props.as_ref().and_then(|ps| ps.iter().find(|x| &x.name == p)) {
                    Some(pr) => !pr.optional,
                    None => !has_default,
                };
                if required && !given.contains(p) {
                    self.err(span, format!("missing required prop `{p}` for `{}` ({})", sig.name, sig.file));
                }
            }
        }
        match (has_caller, sig.slot) {
            (true, SlotUse::No) => self.err(span, format!("`{}` takes no default slot (its body never calls `caller()`), but is used with `{{% call %}}`", sig.name)),
            (false, SlotUse::Required) => self.err(span, format!("`{}` needs a default slot: use `{{% call {}(…) %}}…{{% endcall %}}`", sig.name, sig.name)),
            _ => {}
        }
        (Ty::Slot, false)
    }
}

// Does the macro body call caller()? Guarded by `caller is defined` => optional slot.
fn slot_use(body: &[Stmt]) -> SlotUse {
    let dbg = format!("{body:?}");
    if !dbg.contains("id: \"caller\"") {
        SlotUse::No
    } else if dbg.contains("name: \"defined\"") {
        SlotUse::Optional
    } else {
        SlotUse::Required
    }
}

pub fn run(sources: BTreeMap<String, String>, schema: &crate::site::Schema) -> Checker {
    let mut c = Checker::new(sources, schema.globals.clone(), schema.named.clone());
    c.check_macro_bodies();
    for k in &schema.kinds {
        c.check_route(k);
    }
    c.diags.sort_by(|a, b| (&a.file, a.line, a.col).cmp(&(&b.file, b.line, b.col)));
    c
}

pub fn run_one(sources: BTreeMap<String, String>, schema: &crate::site::Schema, kind: &RouteKind) -> Checker {
    let mut c = Checker::new(sources, schema.globals.clone(), schema.named.clone());
    c.check_macro_bodies();
    c.check_route(kind);
    c
}
