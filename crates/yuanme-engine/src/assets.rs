//! The assets stage (#20): the stylesheets, Scripts and files that Pages
//! name, found in their rendered HTML, the Site-wide Sidecar stylesheet and
//! each Page's Sidecar scripts. Each becomes an [`Asset`] that the link pass
//! names by its hash.

mod diagnostics;
mod order;
mod project;
mod scripts;
pub(crate) mod styles;
pub(crate) mod tailwind;
pub(crate) mod typescript;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;

use rayon::prelude::*;
use yuanme_assets::{
    Packages, ReferenceKind, Scan, ScriptEntry, bundle_css, imports_tailwind, js_strings,
    scan_html, tailwind_tokens,
};
use yuanme_content::config::{Config, RouterMode};
use yuanme_content::fields::Value;
use yuanme_diag::{Applicability, Diagnostic, FileId, Files, Span, codes};
use yuanme_render::template::{Page, scope};

use diagnostics::{Diagnostics, SourceFiles, module_type, reference_span, unmatched};
use order::{Order, Use};
use project::{Project, Target, copy};
use scripts::Scripts;

use crate::hashing::{Asset, placeholder};
use crate::url::suffix;
use crate::{Body, Bundle, Cache, Inputs};

/// The stylesheet holding every used Template's Sidecar CSS.
const SIDECARS: &str = "sidecars.css";
/// Where that stylesheet's `@import`s are, as lightningcss sees it. NUL
/// can't be in a project file's name.
const SIDECARS_ENTRY: &str = "\0sidecars.css";

/// What the assets stage leaves for the link pass.
pub(crate) struct Staged {
    /// Each Page's bytes, with placeholders for the assets it names.
    pub pages: Vec<(String, Vec<u8>)>,
    /// What each HTML Page shows, in the order of `pages`.
    pub seen: Vec<Option<Arc<Seen>>>,
    pub assets: Vec<Asset>,
    /// What went into each Script bundle.
    pub scripts: Vec<Bundle>,
    /// The dev client's chunk, by its path in the output directory.
    pub client: Option<String>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Staged {
    fn failed(diagnostics: Diagnostics) -> Staged {
        Staged {
            pages: Vec::new(),
            seen: Vec::new(),
            assets: Vec::new(),
            scripts: Vec::new(),
            client: None,
            diagnostics: diagnostics.0,
        }
    }
}

/// What later passes ask of a Page's text: what it names, its highlighted
/// code's classes, and whether it has math. It reads nothing but the text,
/// so `yuanme dev` keeps it with the text for its next build.
pub(crate) struct Seen {
    pub scan: Scan,
    pub code: Option<BTreeSet<String>>,
    pub math: bool,
}

impl Seen {
    fn of(text: &str) -> Seen {
        let code = yuanme_render::highlight::classes(text);
        let mut scan = scan_html(text);
        // Only Tailwind's tokens read them, as a set; a Page of
        // highlighted code holds thousands of the same few.
        scan.classes.sort_unstable();
        scan.classes.dedup();
        Seen {
            scan,
            code: code.map(|c| c.into_iter().map(str::to_string).collect()),
            math: styles::has_math(text.as_bytes()),
        }
    }
}

/// The assets stage of one build, run by [`Stage::run`] one step at a
/// time. `dev` is `yuanme dev`'s: it keeps a Tailwind CLI running for each
/// stylesheet, and bundles the dev client.
pub(crate) struct Stage<'a> {
    project: Project<'a>,
    config: &'a Config,
    templates: &'a BTreeMap<String, FileId>,
    sources: &'a mut Files,
    cache: &'a mut Cache,
    dev: bool,
    /// Each used Sidecar stylesheet, and the attribute that scopes it.
    scopes: BTreeMap<String, String>,
    diagnostics: Diagnostics,
    files: SourceFiles,
}

/// What the Pages name, each listed once, in the order first named.
struct Named {
    /// Each stylesheet a Page links, which is one bundle.
    stylesheets: Vec<String>,
    /// Each Script a Page loads, by the project file it starts from.
    scripts: Vec<String>,
    /// Each file copied as is. The stylesheets' `url()`s add theirs.
    copies: BTreeSet<String>,
    /// Per Page: (where, what goes there, keeping the URL's `?` and `#`).
    edits: Vec<Vec<(Range<usize>, Use)>>,
}

/// One stylesheet bundle.
struct Stylesheet {
    /// Its name before hashing, such as `site.css`.
    name: String,
    code: String,
    /// lightningcss's placeholder for each `url()`, and where it goes.
    urls: Vec<(String, Use)>,
}

impl<'a> Stage<'a> {
    pub(crate) fn new(
        inputs: &'a Inputs,
        config: &'a Config,
        templates: &'a BTreeMap<String, FileId>,
        pages: &[Page],
        sources: &'a mut Files,
        cache: &'a mut Cache,
        dev: bool,
    ) -> Stage<'a> {
        let mut site = BTreeSet::new();
        for page in pages {
            site.insert(format!("/{}", page.output));
            if let Some(dir) = page.output.strip_suffix("index.html") {
                site.insert(format!("/{dir}"));
            }
        }
        let scopes: BTreeMap<String, String> = pages
            .iter()
            .flat_map(|p| &p.templates)
            .map(|t| (scope::sidecar(t), scope::attribute(t)))
            .filter(|(path, _)| inputs.files.contains_key(path))
            .collect();
        let project = Project {
            inputs,
            site,
            packages: inputs.root.as_deref().map(Packages::new),
            sidecars: scopes
                .keys()
                .map(|p| format!("@import {};\n", css_string(&format!("/{p}"))))
                .collect(),
            tailwind: BTreeMap::new(),
        };
        Stage {
            project,
            config,
            templates,
            sources,
            cache,
            dev,
            scopes,
            diagnostics: Diagnostics::default(),
            files: SourceFiles::default(),
        }
    }

    /// `pages` with placeholders for the assets they name, and those
    /// assets, unless a step has an error.
    pub(crate) fn run(mut self, data: &BTreeMap<String, Value>, pages: Vec<Page>) -> Staged {
        self.check_scopes();
        let seen = self.scan(&pages);
        let mut named = self.references(&pages, &seen);
        let (page_scripts, page_script) = self.page_scripts(&pages, &seen);
        let scripts = self.bundle_scripts(data, &named.scripts, page_scripts);
        let tailwind = self.tailwind(&named.stylesheets, &seen, scripts.as_ref());
        let stylesheets = self.bundle_stylesheets(&named.stylesheets, &tailwind, &mut named.copies);
        let Some(scripts) = scripts.filter(|_| !self.diagnostics.failed()) else {
            return Staged::failed(self.diagnostics);
        };

        let copies: Vec<String> = named.copies.into_iter().collect();
        // Every bundle succeeded, or the build failed above.
        let chunks_at = copies.len() + stylesheets.len();
        let order = Order {
            copies,
            stylesheets: &named.stylesheets,
            scripts: &named.scripts,
            entries: &scripts.entries,
            preloads: &scripts.preloads,
            chunks_at,
        };
        let Some(mut assets) = self.copies(&order.copies) else {
            return Staged::failed(self.diagnostics);
        };
        assets.extend(stylesheets.into_iter().map(|s| s.linked(&order)));
        assets.extend(scripts.assets);
        let spliced = self.splice(&pages, &seen, named.edits, page_script, &order);
        self.keep(pages, &seen);
        let client = self.dev.then(|| {
            let chunk = *scripts
                .entries
                .last()
                .expect("bundle_scripts adds the dev client's entry last");
            scripts.report[chunk].file.clone()
        });
        Staged {
            pages: spliced,
            seen,
            assets,
            client,
            scripts: scripts.report,
            diagnostics: self.diagnostics.0,
        }
    }

    /// An error for each two used Sidecars that would share a scope
    /// attribute.
    fn check_scopes(&mut self) {
        let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
        for (path, attribute) in &self.scopes {
            if let Some(other) = seen.insert(attribute, path) {
                let message =
                    format!("`{other}` and `{path}` have the same scope attribute, `{attribute}`");
                let span = self.files.span(&self.project, self.sources, path, 1, 1, "");
                self.diagnostics.push(
                    Diagnostic::error(&codes::ASSET_SCOPE_COLLISION, message, span)
                        .help("rename one of their Templates, and its Sidecar"),
                );
            }
        }
    }

    /// What each HTML Page shows, in the order of `pages`: the last dev
    /// build's when the Page's text is the same.
    fn scan(&mut self, pages: &[Page]) -> Vec<Option<Arc<Seen>>> {
        let kept = std::mem::take(&mut self.cache.seen);
        pages
            .par_iter()
            .map(|p| {
                if !p.output.ends_with(".html") {
                    return None;
                }
                Some(match kept.get(&p.output) {
                    Some((text, seen)) if *text == p.text => seen.clone(),
                    _ => Arc::new(Seen::of(&p.text)),
                })
            })
            .collect()
    }

    /// What each Page's references name, with an error for each that
    /// names nothing yuanme can serve.
    fn references(&mut self, pages: &[Page], seen: &[Option<Arc<Seen>>]) -> Named {
        let (templates, sources) = (self.templates, &*self.sources);
        let mut named = Named {
            stylesheets: Vec::new(),
            scripts: Vec::new(),
            copies: BTreeSet::new(),
            edits: vec![Vec::new(); pages.len()],
        };
        for (i, (page, scan)) in pages.iter().zip(seen).enumerate() {
            let Some(Seen { scan, .. }) = scan.as_deref() else {
                continue;
            };
            let base = format!("/{}", page.output);
            for r in &scan.references {
                let at_template =
                    || reference_span(page, &page.text[r.at.clone()], templates, sources);
                let path = match self.project.resolve(r.url.trim(), &base) {
                    Ok(Target::Source(path)) => path,
                    Ok(_) => continue,
                    Err(problem) => {
                        let (span, _) = at_template();
                        self.diagnostics
                            .push(problem.diagnostic(span).note(note(page)));
                        continue;
                    }
                };
                let use_ = match r.kind {
                    ReferenceKind::Classic => {
                        let (span, found) = at_template();
                        let mut d = Diagnostic::error(
                            &codes::ASSET_CLASSIC_SCRIPT,
                            format!("`{}` is loaded as a classic script", r.url.trim()),
                            span.clone(),
                        )
                        .help("yuanme bundles Scripts as modules");
                        if found && let Some(edit) = module_type(&span, sources) {
                            d = d.suggest(
                                "add `type=\"module\"`",
                                Applicability::MaybeIncorrect,
                                vec![edit],
                            );
                        }
                        self.diagnostics.push(d.note(note(page)));
                        continue;
                    }
                    ReferenceKind::Module => {
                        let problem = if path.ends_with(".jsx") || path.ends_with(".tsx") {
                            Some((&codes::ASSET_JSX, "is JSX, which yuanme doesn't compile"))
                        } else if !scripts::EXTENSIONS.iter().any(|e| path.ends_with(e)) {
                            Some((
                                &codes::ASSET_INVALID_SCRIPT,
                                "is no Script: a Script is a `.ts`, `.js`, `.mts` or `.mjs` file",
                            ))
                        } else {
                            None
                        };
                        if let Some((code, why)) = problem {
                            let (span, _) = at_template();
                            let message = format!("`{}` {why}", r.url.trim());
                            self.diagnostics
                                .push(Diagnostic::error(code, message, span).note(note(page)));
                            continue;
                        }
                        Use::Script(path)
                    }
                    ReferenceKind::Stylesheet => Use::Bundle(path),
                    // A preload of a stylesheet must fetch the bundle that
                    // `<link rel=stylesheet>` gets.
                    ReferenceKind::Link if path.ends_with(".css") => Use::Bundle(path),
                    ReferenceKind::Link => {
                        named.copies.insert(path.clone());
                        Use::Copy(path)
                    }
                };
                if let Use::Bundle(path) = &use_
                    && !named.stylesheets.contains(path)
                {
                    named.stylesheets.push(path.clone());
                }
                if let Use::Script(path) = &use_
                    && !named.scripts.contains(path)
                {
                    named.scripts.push(path.clone());
                }
                named.edits[i].push((r.at.clone(), use_));
            }
        }
        named
    }

    /// Each Page's Script (ADR 0013): the Sidecar scripts of its Templates,
    /// in their order, and with the Router on, the Router. Pages with the
    /// same Sidecar scripts share one, so this is each distinct Page
    /// Script's Sidecar scripts, and which of them each Page has.
    fn page_scripts(
        &self,
        pages: &[Page],
        seen: &[Option<Arc<Seen>>],
    ) -> (Vec<Vec<String>>, Vec<Option<usize>>) {
        let router = self.config.router == RouterMode::Swap;
        let inputs = self.project.inputs;
        let mut page_scripts: Vec<Vec<String>> = Vec::new();
        // By Template, since every Page renders a few of the same ones.
        let mut sidecar_scripts: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        let page_script = pages
            .iter()
            .zip(seen)
            .map(|(page, scan)| {
                scan.as_ref()?.scan.stylesheet_at?;
                let mut sidecars = Vec::new();
                for t in &page.templates {
                    let found = sidecar_scripts.entry(t).or_insert_with(|| {
                        let stem = t.rsplit_once('.').map_or(&**t, |(s, _)| s);
                        scripts::EXTENSIONS
                            .iter()
                            .map(|e| format!("templates/{stem}{e}"))
                            .filter(|path| inputs.files.contains_key(path))
                            .collect()
                    });
                    sidecars.extend(found.iter().cloned());
                }
                if sidecars.is_empty() && !router {
                    return None;
                }
                Some(match page_scripts.iter().position(|s| *s == sidecars) {
                    Some(i) => i,
                    None => {
                        page_scripts.push(sidecars);
                        page_scripts.len() - 1
                    }
                })
            })
            .collect();
        (page_scripts, page_script)
    }

    /// Every Script, Page Script and, in dev, the dev client, in one
    /// rolldown build; `None` when it fails.
    fn bundle_scripts(
        &mut self,
        data: &BTreeMap<String, Value>,
        script_entries: &[String],
        page_scripts: Vec<Vec<String>>,
    ) -> Option<Scripts> {
        let entries: Vec<ScriptEntry> = script_entries
            .iter()
            .map(|path| ScriptEntry::Script(path.clone()))
            .chain(page_scripts.into_iter().map(ScriptEntry::Page))
            .chain(self.dev.then_some(ScriptEntry::Client))
            .collect();
        scripts::bundle(
            &self.project,
            self.config,
            data,
            &entries,
            self.sources,
            &mut self.files,
            &mut self.diagnostics,
        )
    }

    /// Each of `stylesheets` that imports Tailwind, built by it from the
    /// tokens of every Page, Script and Sidecar for lightningcss to read in
    /// place of its text. With a Script that failed, the tokens aren't all
    /// known, and the build fails anyway.
    fn tailwind(
        &mut self,
        stylesheets: &[String],
        seen: &[Option<Arc<Seen>>],
        scripts: Option<&Scripts>,
    ) -> Vec<String> {
        let tailwind: Vec<String> = stylesheets
            .iter()
            .filter(|e| {
                self.project
                    .text(e)
                    .is_some_and(|css| imports_tailwind(&css))
            })
            .cloned()
            .collect();
        if let (Some(scripts), false) = (scripts, tailwind.is_empty()) {
            let classes = seen.iter().flatten().flat_map(|s| &s.scan.classes);
            let mut strings = Vec::new();
            for asset in &scripts.assets {
                if let Asset::Copy {
                    body: Body::Bytes(code),
                    ..
                } = asset
                {
                    strings.extend(js_strings(&String::from_utf8_lossy(code)));
                }
            }
            let sidecar_texts: Vec<String> = self
                .scopes
                .keys()
                .filter_map(|p| self.project.text(p))
                .collect();
            let tokens = tailwind_tokens(
                classes
                    .map(String::as_str)
                    .chain(strings.iter().map(String::as_str))
                    .chain(sidecar_texts.iter().map(String::as_str)),
            );
            self.project.tailwind = tailwind::outputs(
                &self.project,
                self.config,
                self.cache,
                &tailwind,
                &tokens,
                self.dev,
                self.sources,
                &mut self.files,
                &mut self.diagnostics,
            );
        }
        tailwind
    }

    /// Each of `stylesheets` bundled, and the Site-wide Sidecar stylesheet
    /// when a Page uses a Sidecar, with each `url()` resolved; the files
    /// they name join `copies`. A bundle that fails has an error instead.
    fn bundle_stylesheets(
        &mut self,
        stylesheets: &[String],
        tailwind: &[String],
        copies: &mut BTreeSet<String>,
    ) -> Vec<Stylesheet> {
        let mut bundles: Vec<(String, &str)> = stylesheets
            .iter()
            .map(|e| (file_name(e).to_string(), e.as_str()))
            .collect();
        if !self.scopes.is_empty() {
            bundles.push((SIDECARS.to_string(), SIDECARS_ENTRY));
        }
        let (project, scopes) = (&self.project, &self.scopes);
        let targets = &self.config.build.targets;
        let unscoped = BTreeMap::new();
        let bundled: Vec<_> = bundles
            .par_iter()
            .map(|(_, entry)| {
                let scopes = if *entry == SIDECARS_ENTRY {
                    scopes
                } else {
                    &unscoped
                };
                let is_tailwind = tailwind.iter().any(|t| t == entry);
                // Tailwind failed, and said so.
                if is_tailwind && !project.tailwind.contains_key(*entry) {
                    return None;
                }
                Some(bundle_css(project, entry, targets, scopes, is_tailwind))
            })
            .collect();
        let mut out = Vec::new();
        for ((name, entry), result) in bundles.into_iter().zip(bundled) {
            let bundle = match result {
                None => continue,
                Some(Ok(bundle)) => bundle,
                Some(Err(e)) => {
                    let d = self.files.error(*e, entry, &self.project, self.sources);
                    self.diagnostics.push(d);
                    continue;
                }
            };
            for compound in &bundle.compounds {
                let unmatched = unmatched(
                    compound,
                    self.templates,
                    &self.project,
                    self.sources,
                    &mut self.files,
                );
                if let Some(d) = unmatched {
                    self.diagnostics.push(d);
                }
            }
            let mut urls = Vec::new();
            for url in &bundle.urls {
                let use_ = match self.project.resolve(&url.url, &format!("/{}", url.file)) {
                    Ok(Target::External) => Use::Url(url.url.clone()),
                    // The bundle is served from `/assets/`, so a relative URL
                    // must become one from the root.
                    Ok(Target::Site(path)) => Use::Url(path + suffix(&url.url)),
                    Ok(Target::Source(path)) => {
                        copies.insert(path.clone());
                        Use::Copy(path)
                    }
                    Err(problem) => {
                        let span = self.files.span(
                            &self.project,
                            self.sources,
                            &url.file,
                            url.line,
                            url.column,
                            &url.url,
                        );
                        self.diagnostics.push(problem.diagnostic(span));
                        continue;
                    }
                };
                urls.push((url.placeholder.clone(), use_));
            }
            out.push(Stylesheet {
                name,
                code: bundle.code,
                urls,
            });
        }
        out
    }

    /// Each of `copies`, as is, unless one can't be read.
    fn copies(&mut self, copies: &[String]) -> Option<Vec<Asset>> {
        let mut assets = Vec::new();
        for path in copies {
            match copy(&self.project, path) {
                Ok(asset) => assets.push(asset),
                Err(problem) => self.diagnostics.push(problem.diagnostic(Span {
                    file: self.config.file,
                    range: 0..0,
                })),
            }
        }
        (!self.diagnostics.failed()).then_some(assets)
    }

    /// Each Page's bytes with a placeholder for each asset it names. Its
    /// Sidecar stylesheet and
    /// Page Script go after its own stylesheets, and then a preload of
    /// every chunk its Scripts import, so the browser fetches them all at
    /// once.
    fn splice(
        &self,
        pages: &[Page],
        seen: &[Option<Arc<Seen>>],
        edits: Vec<Vec<(Range<usize>, Use)>>,
        page_script: Vec<Option<usize>>,
        order: &Order<'_>,
    ) -> Vec<(String, Vec<u8>)> {
        let swap = self.config.router == RouterMode::Swap;
        let native = self.config.router == RouterMode::Native;
        let sidecars_link = (!self.scopes.is_empty()).then(|| {
            format!(
                "<link rel=\"stylesheet\" href=\"{}\">",
                placeholder(order.sidecars())
            )
        });
        pages
            .par_iter()
            .zip(seen)
            .zip(edits)
            .zip(page_script)
            .map(|(((page, scan), edits), page_script)| {
                let loaded: Vec<usize> = edits
                    .iter()
                    .filter_map(|(_, use_)| match use_ {
                        Use::Script(path) => Some(order.script(path)),
                        _ => None,
                    })
                    .chain(page_script.map(|i| order.page_script(i)))
                    .collect();
                let mut preloads: Vec<usize> = Vec::new();
                for &chunk in &loaded {
                    for &c in &order.preloads[chunk] {
                        if !loaded.contains(&c) && !preloads.contains(&c) {
                            preloads.push(c);
                        }
                    }
                }
                let mut tags = sidecars_link.clone().unwrap_or_default();
                if native {
                    tags += "<style>@view-transition{navigation:auto}</style>";
                }
                if let Some(i) = page_script {
                    let src = placeholder(order.chunk(order.page_script(i)));
                    // The Router swaps in only a Page that carries it.
                    let marker = if swap { " data-yuanme-router" } else { "" };
                    tags += &format!("<script type=\"module\" src=\"{src}\"{marker}></script>");
                }
                for chunk in preloads {
                    let href = placeholder(order.chunk(chunk));
                    tags += &format!("<link rel=\"modulepreload\" href=\"{href}\">");
                }
                let mut edits: Vec<(Range<usize>, String)> = edits
                    .into_iter()
                    .map(|(at, use_)| {
                        let with =
                            placeholder(order.index(&use_)) + suffix(page.text[at.clone()].trim());
                        (at, with)
                    })
                    .collect();
                let at = scan.as_ref().and_then(|s| s.scan.stylesheet_at);
                if let (false, Some(at)) = (tags.is_empty(), at) {
                    edits.push((at..at, tags));
                    edits.sort_by_key(|(at, _)| at.start);
                }
                let bytes = splice(&page.text, &edits);
                (page.output.clone(), bytes)
            })
            .collect()
    }

    /// In dev, what each HTML Page shows, kept with the Page's text for the
    /// next build's [`Stage::scan`].
    fn keep(&mut self, pages: Vec<Page>, seen: &[Option<Arc<Seen>>]) {
        if !self.dev {
            return;
        }
        for (page, seen) in pages.into_iter().zip(seen) {
            if let Some(seen) = seen {
                self.cache
                    .seen
                    .insert(page.output, (page.text, seen.clone()));
            }
        }
    }
}

impl Stylesheet {
    /// The bundle as an asset, each `url()` pointing at its asset's
    /// placeholder or at its URL.
    fn linked(self, order: &Order<'_>) -> Asset {
        let mut code = self.code;
        // lightningcss's placeholder for each `url()`, not the link pass's.
        for (written, use_) in self.urls {
            let url = match &use_ {
                Use::Url(url) => css_string(url),
                _ => format!("\"{}\"", placeholder(order.index(&use_))),
            };
            code = code.replacen(&format!("\"{written}\""), &url, 1);
        }
        Asset::Linked {
            name: self.name,
            bytes: code.into_bytes(),
        }
    }
}

/// `text` with each range replaced, in order.
fn splice(text: &str, edits: &[(Range<usize>, String)]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut done = 0;
    for (at, with) in edits {
        out.extend_from_slice(&text.as_bytes()[done..at.start]);
        out.extend_from_slice(with.as_bytes());
        done = at.end;
    }
    out.extend_from_slice(&text.as_bytes()[done..]);
    out
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn note(page: &Page) -> String {
    format!("in the Page written to `{}`", page.output)
}

/// A CSS string, quoted.
fn css_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\a "),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
