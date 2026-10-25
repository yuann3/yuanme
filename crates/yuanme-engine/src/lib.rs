//! The one seam every caller crosses: `build` turns a snapshot of the
//! project into every file of the Site, in memory (ADR 0011).

mod assets;
mod budgets;
mod cache;
mod highlight;
mod host;
pub mod image;
mod images;
mod link;
mod links;
mod math;
#[cfg(test)]
mod tests;
mod typescript;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use yuanme_content::config::{self, Budget, Config};
use yuanme_content::site::{self, Project, RouteKind, Site};
use yuanme_diag::{Code, Diagnostic, FileId, Files, Level, Span, codes};
use yuanme_render::template::scope;

pub use cache::Cache;
pub use host::{file_of, served, wrangler};
use link::link;
pub use typescript::{check_types, write_types};

/// A snapshot of the project's files, by path relative to the project root
/// with `/` separators. It is everything a build reads.
#[derive(Clone, Default)]
pub struct Inputs {
    /// Where streamed files are read from. `None` for Inputs built in
    /// memory, which hold every file's bytes.
    root: Option<PathBuf>,
    files: BTreeMap<String, Input>,
    /// Folders the walk found, so an empty Collection folder still exists.
    dirs: BTreeSet<String>,
}

#[derive(Clone)]
pub struct Input {
    pub hash: blake3::Hash,
    /// `None` for a file whose bytes were hashed as a stream and not kept,
    /// such as an image: it is read from the project when it is needed.
    pub bytes: Option<Arc<[u8]>>,
}

impl Inputs {
    /// The Inputs of the project at `root`.
    pub fn at(root: PathBuf) -> Inputs {
        Inputs {
            root: Some(root),
            ..Inputs::default()
        }
    }

    pub fn insert(&mut self, path: impl Into<String>, bytes: impl Into<Arc<[u8]>>) {
        let bytes = bytes.into();
        let input = Input {
            hash: blake3::hash(&bytes),
            bytes: Some(bytes),
        };
        self.files.insert(path.into(), input);
    }

    pub fn insert_streamed(&mut self, path: impl Into<String>, hash: blake3::Hash) {
        self.files.insert(path.into(), Input { hash, bytes: None });
    }

    pub fn insert_dir(&mut self, path: impl Into<String>) {
        self.dirs.insert(path.into());
    }

    fn is_dir(&self, dir: &str) -> bool {
        let under = format!("{dir}/");
        self.dirs.contains(dir)
            || self
                .files
                .range(under.clone()..)
                .next()
                .is_some_and(|(p, _)| p.starts_with(&under))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Input)> {
        self.files
            .iter()
            .map(|(path, input)| (path.as_str(), input))
    }
}

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
        images: BTreeMap::new(),
        budgets: Vec::new(),
    };
    let (config, mut diagnostics) = site_config(inputs, cache.engine_version(), &mut sources);
    let Some(config) = config else {
        return failed(diagnostics, sources, None, None);
    };
    // Entries are parsed afresh each build until #47 caches them. `mode` is
    // also for the unsubset font (#52).
    let (site, more) = resolve(inputs, &config, mode.drafts(), &mut sources);
    diagnostics.extend(more);
    let Some(site) = site else {
        return failed(diagnostics, sources, Some(config), None);
    };
    let statics = static_files(inputs);
    diagnostics.extend(static_collisions(&config, &site, &statics));
    let templates = templates(inputs, &mut sources, &mut diagnostics);
    if diagnostics.iter().any(|d| d.level == Level::Error) {
        return failed(diagnostics, sources, Some(config), Some(site));
    }
    let (code, more) = highlight::code(&site, &sources, cache);
    diagnostics.extend(more);
    let images = Arc::new(images::Images::new(inputs, cache, &config, &templates));
    let (pictures, more) = images.bodies(&site);
    diagnostics.extend(more);
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
        &code,
        &pictures,
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
    let mut pages = pages;
    let bodies: BTreeMap<String, String> = pages
        .iter_mut()
        .filter_map(|p| Some((p.output.clone(), p.body.take()?)))
        .collect();
    let stage = assets::assets(
        inputs,
        &config,
        &site.data,
        pages,
        &templates,
        &mut sources,
        cache,
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
    highlight::stylesheet(&mut pages, &mut assets);
    let math_font = math::stylesheet(&site, mode, cache, &mut pages, &mut assets);
    let mut files = link(pages, assets);
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
    );
    diagnostics.extend(more);
    let more = images::in_css(inputs, &files, &heavy, &mut sources, &fallback);
    diagnostics.extend(more);
    let budgets = match &config.budgets {
        Some(budgets) if mode.encodes() => {
            let math_font = math_font.as_deref();
            let (measured, more) =
                budgets::measure(&config, budgets, &site, &files, &bodies, math_font, cache);
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
        images: used.variants,
        budgets,
    };
    if !build.failed() {
        cache.sweep();
    }
    build
}

fn site_config(
    inputs: &Inputs,
    engine: &str,
    sources: &mut Files,
) -> (Option<Config>, Vec<Diagnostic>) {
    const PATH: &str = "yuanme.toml";
    let input = inputs.files.get(PATH).expect("a project has yuanme.toml");
    let bytes = input
        .bytes
        .as_deref()
        .expect("yuanme.toml is read, not streamed");
    match utf8(PATH, bytes, &codes::CONFIG_SYNTAX, sources) {
        Ok(file) => config::parse(sources, file, engine),
        Err(d) => (None, vec![d]),
    }
}

/// Adds a text file to `sources`. Not UTF-8 is an error rather than a lossy
/// copy, which could parse as something the Author never wrote.
fn utf8(
    path: &str,
    bytes: &[u8],
    code: &'static Code,
    sources: &mut Files,
) -> Result<FileId, Diagnostic> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(sources.add(path, text)),
        Err(e) => {
            // The lossy copy matches the bytes up to the first bad one.
            let file = sources.add(path, String::from_utf8_lossy(bytes));
            let at = e.valid_up_to();
            let span = Span {
                file,
                range: at..at,
            };
            Err(Diagnostic::error(
                code,
                format!("{path} is not UTF-8"),
                span,
            ))
        }
    }
}

/// Reads every Entry and Data file and resolves the Site.
fn resolve(
    inputs: &Inputs,
    config: &Config,
    drafts: bool,
    sources: &mut Files,
) -> (Option<Site>, Vec<Diagnostic>) {
    let (entries, data) = site::sources(config, inputs.files.keys().map(String::as_str));
    let mut diagnostics = Vec::new();
    let mut add = |path: &str| {
        let bytes = inputs.files[path]
            .bytes
            .as_deref()
            .expect("text is read, not streamed");
        utf8(path, bytes, &codes::ENTRY_SYNTAX, sources)
            .map_err(|d| diagnostics.push(d))
            .ok()
    };
    let entries: Vec<_> = entries
        .into_iter()
        .filter_map(|(c, p)| Some((c, add(p)?)))
        .collect();
    let data: Vec<_> = data.into_iter().filter_map(&mut add).collect();
    if !diagnostics.is_empty() {
        return (None, diagnostics);
    }
    let is_file = |p: &str| inputs.files.contains_key(p);
    let is_dir = |d: &str| inputs.is_dir(d);
    let project = Project {
        files: sources,
        entries,
        data,
        is_file: &is_file,
        is_dir: &is_dir,
    };
    site::resolve(config, &project, drafts)
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

/// Every Template, by its path in `templates/`: every text file there but
/// the Sidecars beside them.
fn templates(
    inputs: &Inputs,
    sources: &mut Files,
    diagnostics: &mut Vec<Diagnostic>,
) -> BTreeMap<String, FileId> {
    let mut out = BTreeMap::new();
    for (path, input) in inputs.iter() {
        let Some(name) = path.strip_prefix("templates/") else {
            continue;
        };
        if [".css", ".js", ".mjs", ".ts", ".mts"]
            .iter()
            .any(|e| name.ends_with(e))
        {
            continue;
        }
        // An image, which is streamed, is no Template.
        let Some(bytes) = input.bytes.as_deref() else {
            continue;
        };
        match utf8(path, bytes, &codes::TEMPLATE_SYNTAX, sources) {
            Ok(file) => {
                out.insert(name.to_string(), file);
            }
            Err(d) => diagnostics.push(d),
        }
    }
    out
}

/// `static/` is copied to the same path, unhashed.
fn static_files(inputs: &Inputs) -> Vec<(String, Output)> {
    inputs
        .iter()
        .filter_map(|(path, input)| {
            let out = path.strip_prefix("static/")?;
            let body = match &input.bytes {
                Some(bytes) => Body::Bytes(bytes.clone()),
                None => Body::Input(path.to_string()),
            };
            Some((
                out.to_string(),
                Output {
                    body,
                    hash: input.hash,
                },
            ))
        })
        .collect()
}
