//! The one seam every caller crosses: `build` turns a snapshot of the
//! project into every file of the Site, in memory (ADR 0011).

mod assets;
mod budgets;
mod cache;
mod content;
mod hashing;
mod host;
pub mod image;
mod images;
mod inputs;
mod links;
mod math;
#[cfg(test)]
mod tests;
mod typescript;
mod url;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use yuanme_content::config::{Budget, Config};
use yuanme_content::site::{RouteKind, Site};
use yuanme_diag::{Diagnostic, Files, Level, Span, codes};
use yuanme_render::template::scope;

use content::{bodies, highlight};

pub use cache::Cache;
pub use host::{file_of, served, wrangler};
pub use inputs::{Input, Inputs};
pub use typescript::{check_types, write_types};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Build,
    /// A build with Drafts, as `yuanme build --drafts` makes: only `Dev`
    /// adds what a browser asks the dev server for.
    Drafts,
    /// A build that encodes no image, for `yuanme check`: its Pages name
    /// every variant, which [`Build::images`] lists.
    Check {
        drafts: bool,
    },
    /// With Drafts, encoding no image until a browser asks for it.
    Dev,
}

impl Mode {
    fn drafts(self) -> bool {
        !matches!(self, Mode::Build | Mode::Check { drafts: false })
    }

    fn encodes(self) -> bool {
        matches!(self, Mode::Build | Mode::Drafts)
    }
}

/// Every file of one Site, or the Diagnostics that stopped it.
pub struct Build {
    /// By path in the output directory, such as `blog/pew/index.html`.
    pub files: BTreeMap<String, Output>,
    pub diagnostics: Vec<Diagnostic>,
    /// The files the Diagnostics point into.
    pub sources: Files,
    /// Site config, unless it has an error.
    pub config: Option<Config>,
    /// The resolved Site, unless an Entry or Data file has an error. A
    /// build that failed later still has it, for `yuanme routes` and the
    /// context schemas.
    pub site: Option<Site>,
    /// What went into each Script bundle, for the build report (#20).
    pub scripts: Vec<Bundle>,
    /// The dev client (#68), by its path in `files`: only a `Mode::Dev`
    /// build has it, and no Page names it, since `yuanme dev` adds it to
    /// each HTML file as it serves it.
    pub client: Option<String>,
    /// Every image variant the Pages name, by its path in the output
    /// directory, with the settings that encode it. A build that encodes
    /// no image has none of them in `files`, and `yuanme dev` encodes one
    /// with [`image::encode`] when a browser asks for it.
    pub images: BTreeMap<String, (image::Variant, image::Settings)>,
    /// Each HTML Page measured against the Site budgets, in the order of
    /// the Site's Routes: only by a build that encodes images, which is
    /// `yuanme build`'s, and never with `budgets = false` (#15).
    pub budgets: Vec<Measured>,
}

/// One HTML Page's number for each Site budget.
pub struct Measured {
    pub url: String,
    /// Bytes of Brotli at quality 11, or for `requests` a count.
    pub values: BTreeMap<Budget, u64>,
    /// An Entry's Page's body, which its first-flight HTML leaves out, in
    /// bytes of Brotli.
    pub body: Option<u64>,
    /// The Budgets whose numbers are under half their limits, and so
    /// estimated at Brotli quality 5. The body is estimated with the HTML.
    pub estimated: BTreeSet<Budget>,
}

/// One Script bundle of the Site.
pub struct Bundle {
    /// Its path in the output directory, such as
    /// `assets/site-0123456789abcdef.js`.
    pub file: String,
    /// In bytes.
    pub size: usize,
    /// Each module in it, by its path from the project root, such as
    /// `scripts/site.ts` or `node_modules/nanoid/index.browser.js`, with its
    /// size in the bundle, in the order they run.
    pub modules: Vec<(String, usize)>,
}

#[derive(Clone)]
pub struct Output {
    pub body: Body,
    /// Changes whenever the bytes do, so the writer can skip unchanged files.
    pub hash: blake3::Hash,
}

/// Where an output file's bytes are. Only small files are held in memory;
/// the writer copies the others with `std::fs::copy`, which clones on APFS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    Bytes(Arc<[u8]>),
    /// A project file, by its path in [`Inputs`].
    Input(String),
    /// A large entry in the on-disk [`Cache`].
    Cached(PathBuf),
}

impl Build {
    /// A failed build must not touch the output directory.
    pub fn failed(&self) -> bool {
        self.diagnostics.iter().any(|d| d.level == Level::Error)
    }
}

/// `inputs` must hold `yuanme.toml`: finding it is what makes a folder a project.
pub fn build(inputs: &Inputs, cache: &mut Cache, mode: Mode) -> Build {
    let mut sources = Files::default();
    let failed = |diagnostics, sources, config, site| Build {
        files: BTreeMap::new(),
        diagnostics,
        sources,
        config,
        site,
        scripts: Vec::new(),
        client: None,
        images: BTreeMap::new(),
        budgets: Vec::new(),
    };
    let (config, mut diagnostics) =
        content::site_config(inputs, cache.engine_version(), &mut sources);
    let Some(config) = config else {
        return failed(diagnostics, sources, None, None);
    };
    let (site, more) = content::resolve(inputs, &config, mode.drafts(), &mut sources, cache);
    diagnostics.extend(more);
    let Some(site) = site else {
        return failed(diagnostics, sources, Some(config), None);
    };
    let statics = inputs.statics();
    diagnostics.extend(static_collisions(&config, &site, &statics));
    let templates = inputs.templates(&mut sources, &mut diagnostics);
    if diagnostics.iter().any(|d| d.level == Level::Error) {
        return failed(diagnostics, sources, Some(config), Some(site));
    }
    let (code, more) = highlight::code(&site, &sources, cache);
    diagnostics.extend(more);
    let images = Arc::new(images::Images::new(inputs, cache, &config, &templates));
    let (pictures, more) = images.bodies(&site);
    diagnostics.extend(more);
    let bodies = bodies::bodies(&site, &code, pictures, cache, mode == Mode::Dev);
    // An Entry's Page's first-flight HTML leaves its body out (#15).
    let measuring = config.budgets.is_some() && mode.encodes();
    let measured = measuring.then(|| bodies.clone());
    let static_urls = statics.iter().map(|(p, _)| format!("/{p}")).collect();
    let scoped = templates
        .keys()
        .filter(|t| inputs.files.contains_key(&scope::sidecar(t)))
        .cloned()
        .collect();
    let (pages, more) = yuanme_render::template::render(
        &config,
        &site,
        &sources,
        &templates,
        &scoped,
        &static_urls,
        bodies,
        images.clone(),
    );
    diagnostics.extend(more);
    let Ok(images) = Arc::try_unwrap(images) else {
        unreachable!("render has dropped its Environment")
    };
    let used = images.into_used();
    // Routes were checked against Static files before render; this is the
    // sitemap's files.
    for page in &pages {
        if statics.iter().any(|(p, _)| *p == page.output) {
            let message = format!(
                "`static/{0}` and the sitemap would both write `{0}`",
                page.output
            );
            let span = Span {
                file: config.file,
                range: 0..0,
            };
            diagnostics.push(
                Diagnostic::error(&codes::ENTRY_OUTPUT_COLLISION, message, span)
                    .help("rename the Static file: yuanme writes the sitemap itself"),
            );
        }
    }
    if diagnostics.iter().any(|d| d.level == Level::Error) {
        return failed(diagnostics, sources, Some(config), Some(site));
    }
    let rendered: BTreeMap<String, Vec<String>> = pages
        .iter()
        .filter(|p| p.output.ends_with(".html"))
        .map(|p| (p.output.clone(), p.templates.clone()))
        .collect();
    let stage = assets::assets(
        inputs,
        &config,
        &site.data,
        pages,
        &templates,
        &mut sources,
        cache,
        mode == Mode::Dev,
    );
    diagnostics.extend(stage.diagnostics);
    if mode != Mode::Dev && !diagnostics.iter().any(|d| d.level == Level::Error) {
        let more = check_types(inputs, &config, &stage.scripts, cache, &mut sources);
        diagnostics.extend(more);
    }
    if diagnostics.iter().any(|d| d.level == Level::Error) {
        return failed(diagnostics, sources, Some(config), Some(site));
    }
    let (mut pages, mut assets) = (stage.pages, stage.assets);
    highlight::stylesheet(&mut pages, &stage.seen, &mut assets);
    let math_font = math::stylesheet(&site, mode, cache, &mut pages, &stage.seen, &mut assets);
    let mut files = hashing::link(pages, assets, cache, mode == Mode::Dev);
    files.extend(statics);
    files.extend(images::copies(inputs, &used));
    let fallback = Span {
        file: config.file,
        range: 0..0,
    };
    if mode.encodes() {
        diagnostics.extend(images::encode(inputs, cache, &used, &mut files, &fallback));
    }
    let heavy = images::heavy(inputs, &config.images, &files, &used);
    let more = links::links(
        &config,
        &site,
        &rendered,
        &files,
        &used.variants,
        &heavy,
        &templates,
        &sources,
        inputs,
        cache,
    );
    diagnostics.extend(more);
    let more = images::in_css(inputs, &files, &heavy, &mut sources, &fallback);
    diagnostics.extend(more);
    let budgets = match (&config.budgets, &measured) {
        (Some(budgets), Some(bodies)) => {
            let math_font = math_font.as_deref();
            let (measured, more) =
                budgets::measure(&config, budgets, &site, &files, bodies, math_font, cache);
            diagnostics.extend(more);
            measured
        }
        _ => Vec::new(),
    };
    let (files, more) = host::host(&config, files, mode != Mode::Dev, cache);
    diagnostics.extend(more);
    if diagnostics.iter().any(|d| d.level == Level::Error) {
        return failed(diagnostics, sources, Some(config), Some(site));
    }

    let build = Build {
        files,
        diagnostics,
        sources,
        config: Some(config),
        site: Some(site),
        scripts: stage.scripts,
        client: stage.client,
        images: used.variants,
        budgets,
    };
    // A dev build or a check reads no image variant, and a dev build none
    // of what only `yuanme build` reads, such as Brotli and the font
    // subset, so counting their builds would evict them all (#67, #93).
    if !build.failed() && mode.encodes() {
        cache.sweep();
    }
    build
}

/// A Static file and a Route that would write the same output path (#18).
fn static_collisions(
    config: &Config,
    site: &Site,
    statics: &[(String, Output)],
) -> Vec<Diagnostic> {
    let statics: BTreeSet<&str> = statics.iter().map(|(p, _)| p.as_str()).collect();
    site.routes
        .iter()
        .filter(|r| statics.contains(r.output.as_str()))
        .map(|r| {
            let (what, file) = match r.kind {
                RouteKind::Entry(i) => (site.entries[i].path.clone(), site.entries[i].file),
                _ => (format!("the Route `{}`", r.url), config.file),
            };
            let message = format!(
                "`static/{}` and {what} would both write `{}`",
                r.output, r.output
            );
            let span = Span { file, range: 0..0 };
            Diagnostic::error(&codes::ENTRY_OUTPUT_COLLISION, message, span)
                .help("move the Static file, or change the Route")
        })
        .collect()
}
