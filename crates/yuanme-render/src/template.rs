//! Templates (#19): every Route rendered through its Template with
//! minijinja, under SemiStrict undefined rules, and the sitemap.

mod check;
mod context;
mod filters;
pub(crate) mod scope;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use jiff::tz::TimeZone;
use minijinja::machinery::{Token, tokenize};
use minijinja::syntax::SyntaxConfig;
use minijinja::value::ValueKind;
use minijinja::{AutoEscape, Environment, Error, ErrorKind, UndefinedBehavior};
use rayon::prelude::*;
use yuanme_content::{Collection, Config, RouteKind, Site, collections};
use yuanme_diag::{Code, Diagnostic, FileId, Files, Level, Span, codes};

use crate::image::Images;

pub use check::context_schemas;
use context::Context;
pub use filters::escape;
use filters::{Coded, Preloads};

/// The built-in feed Template. `templates/rss.xml` replaces it.
const RSS: (&str, &str) = ("rss.xml", include_str!("template/rss.xml"));

/// Each render thread's stack, which is reserved, not used. minijinja fails
/// a render 500 Templates and macros deep; a debug build needed more than
/// rayon's 2 MiB to get there, so a Template that recursed forever aborted
/// the process instead. 4 MiB was enough for the Template in tests/.
const STACK: usize = 64 << 20;

/// The sitemap protocol's limit of URLs in one file.
const SITEMAP_URLS: usize = 50_000;

pub struct Page {
    /// From the output directory, such as `blog/pew/index.html`.
    pub output: String,
    pub text: String,
    /// The Templates its render used: the Route's own, then every one it
    /// extends, includes or imports, at any depth. The Page gets their
    /// Sidecars (#56).
    pub templates: Vec<String>,
}

/// Renders every Route, then the sitemap. Any error fails the whole
/// render, after every Page has been tried.
pub struct Render<'a> {
    pub config: &'a Config,
    pub site: &'a Site,
    pub files: &'a Files,
    /// Every file of `templates/` in `files`, by its path there.
    pub templates: &'a BTreeMap<String, FileId>,
    /// The Templates with a Sidecar stylesheet, whose tags get their scope
    /// attribute (ADR 0006).
    pub scoped: &'a BTreeSet<String>,
    /// The URL of every Static file, which `url()` accepts as well as
    /// Routes.
    pub statics: &'a BTreeSet<String>,
    /// Each Entry's body as HTML, which [`crate::html::body`] writes.
    pub bodies: Vec<String>,
    /// What makes what `image()` returns.
    pub images: Arc<dyn Images>,
}

/// Each Template scoped by its Sidecar, rewritten, by name.
type Rewritten<'a> = BTreeMap<&'a str, scope::Scoped>;

impl<'a> Render<'a> {
    pub fn run(mut self) -> (Vec<Page>, Vec<Diagnostic>) {
        let origin = self.config.site.url.trim_end_matches('/').to_string();
        let sitemap = sitemap(self.site, &origin);
        let mut diagnostics = Diagnostics::default();
        let rewritten = self.scope(&mut diagnostics);
        let mut env = self.environment(&origin, &sitemap);
        let sources = self.add_templates(&mut env, &rewritten, &mut diagnostics);
        let chosen = self.choose(&sources, &mut diagnostics);
        let checked = check::check(
            &env,
            self.config,
            self.site,
            self.files,
            self.templates,
            &chosen,
            &*self.images,
        );
        for d in checked {
            diagnostics.push(d);
        }
        if diagnostics.failed() {
            return (Vec::new(), diagnostics.0);
        }

        let bodies = std::mem::take(&mut self.bodies);
        let context = Context::new(self.config, self.site, bodies, &origin);
        let uses = uses(&sources);
        let rendered = self.pages(&env, &chosen, &rewritten, &uses, &context);
        let mut pages = Vec::new();
        for r in rendered {
            match r {
                Ok(page) => pages.push(page),
                Err(d) => diagnostics.push(d),
            }
        }
        // Freeing every Entry's values took 4 ms of a prose edit's rebuild in
        // `yuanme dev`, which nothing waits for.
        rayon::spawn(move || drop(context));
        if diagnostics.failed() {
            return (Vec::new(), diagnostics.0);
        }
        pages.extend(sitemap);
        (pages, diagnostics.0)
    }

    /// Each scoped Template with its scope attribute on its tags, and a
    /// warning for each tag that can't have it.
    fn scope(&self, diagnostics: &mut Diagnostics) -> Rewritten<'a> {
        let mut rewritten = BTreeMap::new();
        for name in self.scoped {
            let file = self.templates[name];
            let (text, unscoped) = scope::rewrite(self.files.text(file), &scope::attribute(name));
            for (range, why) in unscoped {
                diagnostics.push(unscoped_tag(why, Span { file, range }));
            }
            rewritten.insert(name.as_str(), text);
        }
        rewritten
    }

    /// minijinja set up as Templates run (#19), with yuanme's filters and
    /// functions. `url()` accepts every Route, Static file and file of
    /// `sitemap`.
    fn environment<'s>(&self, origin: &str, sitemap: &[Page]) -> Environment<'s> {
        let mut urls: BTreeSet<String> = self.site.routes.iter().map(|r| r.url.clone()).collect();
        urls.extend(self.statics.iter().cloned());
        urls.extend(sitemap.iter().map(|p| format!("/{}", p.output)));

        let mut env = Environment::new();
        env.set_undefined_behavior(UndefinedBehavior::SemiStrict);
        env.set_debug(true);
        env.set_syntax(
            SyntaxConfig::builder()
                .trim_blocks(true)
                .lstrip_blocks(true)
                .build()
                .expect("the default delimiters, trimmed, are a valid syntax"),
        );
        env.set_auto_escape_callback(|name| match name.rsplit_once('.').map(|(_, ext)| ext) {
            Some("html" | "xml") => AutoEscape::Html,
            _ => AutoEscape::None,
        });
        env.set_formatter(|out, state, value| {
            // A null prints as nothing (#19). SemiStrict's error for an
            // absent value comes before the formatter is called.
            if value.is_none() || value.is_undefined() {
                return Ok(());
            }
            // `true`, as HTML and JS write it, where Jinja writes `True`.
            let text = match value.kind() {
                ValueKind::Bool => value.is_true().to_string(),
                _ => value.to_string(),
            };
            let text = match state.auto_escape() {
                AutoEscape::Html if !value.is_safe() => escape(&text),
                _ => text,
            };
            out.write_str(&text).map_err(Error::from)
        });
        filters::add(
            &mut env,
            filters::Site {
                origin: origin.to_string(),
                timezone: TimeZone::get(&self.config.site.timezone)
                    .expect("config checked the zone"),
                urls,
            },
            self.images.clone(),
        );
        env
    }

    /// Adds every Template, and the built-in feed unless the project has
    /// its own, to `env`, scoped ones as `rewritten` has them. Returns
    /// their sources as written, by name.
    fn add_templates<'s>(
        &self,
        env: &mut Environment<'s>,
        rewritten: &'s Rewritten<'a>,
        diagnostics: &mut Diagnostics,
    ) -> BTreeMap<&'a str, &'a str>
    where
        'a: 's,
    {
        let mut sources: BTreeMap<&str, &str> = self
            .templates
            .iter()
            .map(|(name, &file)| (name.as_str(), self.files.text(file)))
            .collect();
        sources.entry(RSS.0).or_insert(RSS.1);
        for (&name, &source) in &sources {
            let source = rewritten.get(name).map_or(source, |r| r.text.as_str());
            if let Err(e) = env.add_template(name, source) {
                let d = diagnostic(&e, self.files, self.templates, rewritten, None);
                diagnostics.push(d);
            }
        }
        sources
    }

    /// Each Route's Template, in `site.routes`' order, with where Site
    /// config chooses it, and an error for each one that doesn't exist.
    fn choose(
        &self,
        sources: &BTreeMap<&str, &str>,
        diagnostics: &mut Diagnostics,
    ) -> Vec<(String, Span)> {
        let collections = collections(self.config);
        let chosen: Vec<(String, Span)> = self
            .site
            .routes
            .iter()
            .map(|r| template_of(self.config, &collections, self.site, &r.kind))
            .collect();
        let mut missing = BTreeSet::new();
        for (name, at) in &chosen {
            if !sources.contains_key(name.as_str()) && missing.insert(name) {
                let d = Diagnostic::error(
                    &codes::TEMPLATE_NOT_FOUND,
                    format!("the Template `{name}` doesn't exist"),
                    at.clone(),
                )
                .help(format!(
                    "create `templates/{name}`, or set `template` to one that exists"
                ));
                diagnostics.push(d);
            }
        }
        chosen
    }

    /// Each Route rendered with its Template in `chosen`, each on a thread
    /// of its own, or the Diagnostic saying why it can't be.
    fn pages(
        &self,
        env: &Environment<'_>,
        chosen: &[(String, Span)],
        rewritten: &Rewritten<'_>,
        uses: &BTreeMap<&str, Vec<String>>,
        context: &Context,
    ) -> Vec<Result<Page, Diagnostic>> {
        let threads = rayon::ThreadPoolBuilder::new()
            .stack_size(STACK)
            .build()
            .expect("only spawning a thread fails, and a render can't go on without its threads");
        threads.install(|| {
            self.site
                .routes
                .par_iter()
                .zip(chosen)
                .map(|(route, (name, at))| {
                    let template = env
                        .get_template(name)
                        .expect("each chosen Template was added, or a Diagnostic returned above");
                    match template.render_captured(context.page(route)) {
                        Ok(captured) => {
                            let preloads = captured
                                .state()
                                .get_extension::<Preloads>()
                                .map(|p| p.0.concat())
                                .unwrap_or_default();
                            let mut text = captured.into_output();
                            if !preloads.is_empty() {
                                text.insert_str(head_end(&text), &preloads);
                            }
                            Ok(Page {
                                output: route.output.clone(),
                                text,
                                templates: closure(name, uses),
                            })
                        }
                        Err(e) => {
                            Err(
                                diagnostic(&e, self.files, self.templates, rewritten, Some(at))
                                    .note(format!("while rendering `{}`", route.url)),
                            )
                        }
                    }
                })
                .collect()
        })
    }
}

fn unscoped_tag(why: scope::Unscoped, span: Span) -> Diagnostic {
    let (message, label, help) = match why {
        scope::Unscoped::DynamicName => (
            "this tag's name is built with `{{ }}`, so `check` can't match the Sidecar's type selectors against it",
            "the tag gets the scope attribute after the expression",
            "write the tag once for each name, or a space after the name if `{{ }}` prints attributes",
        ),
        scope::Unscoped::BranchedName => (
            "this tag's name is chosen by `{% %}`, so it gets no scope attribute",
            "not scoped",
            "write the whole tag in each branch",
        ),
        scope::Unscoped::Unterminated => (
            "yuanme can't find where this tag ends, so it and the tags after it get no scope attribute",
            "no `>` outside quotes after this",
            "close each quote in the branch of `{% if %}` that opens it",
        ),
    };
    Diagnostic::warning(&codes::TEMPLATE_UNSCOPED_TAG, message, span)
        .label(label)
        .help(help)
}

/// Where a preload goes: before `</head>`, or else after the doctype, since
/// a `<link>` before `<body>` is in the head.
fn head_end(html: &str) -> usize {
    let lower = html.to_ascii_lowercase();
    if let Some(at) = lower.find("</head>") {
        return at;
    }
    let start = lower.len() - lower.trim_start().len();
    match lower[start..].starts_with("<!doctype") {
        true => lower[start..].find('>').map_or(start, |e| start + e + 1),
        false => start,
    }
}

/// Each Route's Template, in `site.routes`' order, as Site config chooses it.
pub fn route_templates(config: &Config, site: &Site) -> Vec<String> {
    let collections = collections(config);
    site.routes
        .iter()
        .map(|r| template_of(config, &collections, site, &r.kind).0)
        .collect()
}

/// One Diagnostic for each mistake, though many Pages hit it.
#[derive(Default)]
struct Diagnostics(Vec<Diagnostic>);

impl Diagnostics {
    fn push(&mut self, d: Diagnostic) {
        let key = |d: &Diagnostic| (d.code.name, d.message.clone(), d.primary().clone());
        if !self.0.iter().any(|seen| key(seen) == key(&d)) {
            self.0.push(d);
        }
    }

    fn failed(&self) -> bool {
        self.0.iter().any(|d| d.level == Level::Error)
    }
}

/// A Route's Template, and where Site config chooses it.
fn template_of(
    config: &Config,
    collections: &BTreeMap<String, Collection>,
    site: &Site,
    kind: &RouteKind,
) -> (String, Span) {
    let at = |range: &Range<usize>| Span {
        file: config.file,
        range: range.clone(),
    };
    let collection = |name: &str| {
        let c = &collections[name];
        (c.template.clone(), at(&c.template_at))
    };
    match kind {
        RouteKind::Entry(i) => {
            let name = &site.entries[*i].collection;
            let (template, at) = collection(name);
            (template.unwrap_or_else(|| format!("{name}/entry.html")), at)
        }
        RouteKind::List {
            collection: name, ..
        } => (format!("{name}/list.html"), collection(name).1),
        RouteKind::Term { taxonomy, .. } => {
            let t = &config.taxonomies[taxonomy];
            let name = t
                .template
                .clone()
                .unwrap_or_else(|| format!("{taxonomy}/term.html"));
            (name, at(&t.template_at))
        }
        RouteKind::Terms { taxonomy } => (
            format!("{taxonomy}/index.html"),
            at(&config.taxonomies[taxonomy].template_at),
        ),
        RouteKind::Template(name) => {
            let r = &config.routes[name];
            (r.template.clone(), at(&r.template_at))
        }
        RouteKind::Feed { name, .. } => {
            let f = &config.feeds[name];
            (f.template.clone(), at(&f.template_at))
        }
    }
}

/// A minijinja error as a Diagnostic in the Template it happened in, as
/// the Author wrote it: `rewritten` maps a scoped Template's places back.
/// `route` is where Site config chose the Page's Template, for an error in
/// the built-in one.
fn diagnostic(
    e: &Error,
    files: &Files,
    templates: &BTreeMap<String, FileId>,
    rewritten: &BTreeMap<&str, scope::Scoped>,
    route: Option<&Span>,
) -> Diagnostic {
    let code: &'static Code = std::iter::successors(std::error::Error::source(e), |s| s.source())
        .find_map(|s| s.downcast_ref::<Coded>().map(|c| c.0))
        .unwrap_or(match e.kind() {
            ErrorKind::TemplateNotFound => &codes::TEMPLATE_NOT_FOUND,
            ErrorKind::SyntaxError => &codes::TEMPLATE_SYNTAX,
            ErrorKind::UnknownFilter
            | ErrorKind::UnknownTest
            | ErrorKind::UnknownFunction
            | ErrorKind::UnknownMethod => &codes::TEMPLATE_UNKNOWN_FILTER,
            ErrorKind::MissingArgument | ErrorKind::TooManyArguments => {
                &codes::TEMPLATE_BAD_FILTER_ARGUMENT
            }
            _ => &codes::TEMPLATE_RENDER,
        });
    let message = match e.detail() {
        Some(detail) if e.kind() == ErrorKind::InvalidOperation => detail.to_string(),
        Some(detail) => format!("{}: {detail}", e.kind()),
        None => e.kind().to_string(),
    };
    let file = e.name().and_then(|n| templates.get(n));
    let span = match (file, route) {
        (Some(&file), _) => {
            let text = files.text(file);
            let scoped = e.name().and_then(|n| rewritten.get(n));
            let range = e.range().map(|r| match scoped {
                Some(s) => s.original(r.start)..s.original(r.end),
                None => r,
            });
            let range = range.or_else(|| {
                // A line, when minijinja has no finer place.
                let line = e.line()?;
                let start: usize = text
                    .split_inclusive('\n')
                    .take(line - 1)
                    .map(str::len)
                    .sum();
                let len = text[start..].find('\n').unwrap_or(text.len() - start);
                Some(start..start + len)
            });
            Span {
                file,
                range: range.unwrap_or(0..0),
            }
        }
        (None, Some(route)) => route.clone(),
        (None, None) => unreachable!("only the built-in Template has no file"),
    };
    let mut d = Diagnostic::error(code, message, span);
    if file.is_none() {
        d = d.note(format!(
            "in the built-in Template `{}`",
            e.name().unwrap_or(RSS.0)
        ));
    }
    d
}

/// The Templates each one names in `extends`, `include`, `import` and
/// `from`. `check` makes every such name a plain string (#19), so this
/// finds them all without rendering.
fn uses<'a>(sources: &BTreeMap<&'a str, &'a str>) -> BTreeMap<&'a str, Vec<String>> {
    sources
        .iter()
        .map(|(&name, &source)| {
            let tokens: Vec<Token<'_>> = tokenize(source, false, SyntaxConfig::default())
                .map_while(Result::ok)
                .map(|(t, _)| t)
                .collect();
            let mut named = Vec::new();
            for i in 1..tokens.len() {
                let (Token::BlockStart, Token::Ident(tag)) = (&tokens[i - 1], &tokens[i]) else {
                    continue;
                };
                if !["extends", "include", "import", "from"].contains(tag) {
                    continue;
                }
                // A name, or a list of names, up to `import`, `as` or the
                // tag's end.
                for t in tokens[i + 1..]
                    .iter()
                    .take_while(|t| !matches!(t, Token::BlockEnd | Token::Ident(_)))
                {
                    match t {
                        Token::Str(s) => named.push(s.to_string()),
                        Token::String(s) => named.push(s.to_string()),
                        _ => {}
                    }
                }
            }
            (name, named)
        })
        .collect()
}

/// `name` and every Template it uses, at any depth, each once.
fn closure(name: &str, uses: &BTreeMap<&str, Vec<String>>) -> Vec<String> {
    let mut out = vec![name.to_string()];
    let mut i = 0;
    while i < out.len() {
        for used in uses.get(out[i].as_str()).into_iter().flatten() {
            if !out.contains(used) {
                out.push(used.clone());
            }
        }
        i += 1;
    }
    out
}

/// `sitemap-index.xml` and the files it lists, of at most 50,000 URLs
/// each: every Page whose URL ends in `/`. That leaves out `/404.html`,
/// feeds and `.txt` Routes; Unlisted Entries are left out too (#18). A
/// Site with no such Page has no sitemap.
fn sitemap(site: &Site, origin: &str) -> Vec<Page> {
    let mut urls: Vec<&str> = site
        .routes
        .iter()
        .filter(|r| r.url.ends_with('/'))
        .filter(|r| !matches!(r.kind, RouteKind::Entry(i) if site.entries[i].unlisted))
        .map(|r| r.url.as_str())
        .collect();
    urls.sort_unstable();
    if urls.is_empty() {
        return Vec::new();
    }
    let head = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";
    let mut pages = Vec::new();
    let mut index =
        format!("{head}<sitemapindex xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
    for (n, chunk) in urls.chunks(SITEMAP_URLS).enumerate() {
        let output = format!("sitemap-{n}.xml");
        index.push_str(&format!(
            "<sitemap><loc>{}</loc></sitemap>\n",
            escape(&format!("{origin}/{output}"))
        ));
        let mut text =
            format!("{head}<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n");
        for url in chunk {
            text.push_str(&format!(
                "<url><loc>{}</loc></url>\n",
                escape(&format!("{origin}{url}"))
            ));
        }
        text.push_str("</urlset>\n");
        pages.push(Page {
            output,
            text,
            templates: Vec::new(),
        });
    }
    index.push_str("</sitemapindex>\n");
    pages.push(Page {
        output: "sitemap-index.xml".into(),
        text: index,
        templates: Vec::new(),
    });
    pages
}
