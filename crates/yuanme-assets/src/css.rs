//! Stylesheets (#20): each one a Template links is one bundle, with its
//! `@import`s inlined and its `url()`s left for the Engine to name.

mod scope;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use elsa::sync::FrozenMap;
use lightningcss::bundler::{Bundler, ResolveResult, SourceProvider};
use lightningcss::dependencies::{Dependency, DependencyOptions};
use lightningcss::error::ErrorLocation;
use lightningcss::printer::PrinterOptions;
use lightningcss::stylesheet::{MinifyOptions, ParserOptions};
use lightningcss::targets::{Browsers, Targets};
use oxc_resolver::{ResolveOptions, Resolver};

pub use scope::Compound;

/// The project a bundle reads, with files named by their path from the
/// project root, such as `styles/site.css`.
pub trait Project: Sync {
    type Error: std::error::Error + Send + Sync;

    fn read(&self, path: &str) -> Result<String, Self::Error>;

    /// What `@import "<specifier>"` in the stylesheet at `from` names.
    fn import(&self, specifier: &str, from: &str) -> Result<Import, Self::Error>;
}

pub enum Import {
    /// A stylesheet to inline, by its path from the project root.
    File(String),
    /// A URL the bundle keeps its `@import` of.
    External(String),
}

pub struct Bundle {
    /// Minified. Each `url()` holds `"<placeholder>"`, quotes included, for
    /// the Engine to replace with a quoted URL.
    pub code: String,
    /// Each `url()` and each `@import` of an External URL, in the order of
    /// the code.
    pub urls: Vec<Url>,
    /// Each compound selector of a scoped stylesheet's rules.
    pub compounds: Vec<Compound>,
}

pub struct Url {
    /// As written, unescaped.
    pub url: String,
    pub placeholder: String,
    /// The stylesheet it is written in, by its path from the project root.
    /// A relative URL is relative to it.
    pub file: String,
    /// Where it starts: lines from 1, columns from 1 in UTF-16 units.
    pub line: u32,
    pub column: u32,
}

#[derive(Debug)]
pub struct Error<E> {
    pub kind: ErrorKind<E>,
    pub at: Option<Location>,
}

#[derive(Debug)]
pub enum ErrorKind<E> {
    /// From [`Project`], such as an `@import` that names nothing.
    Project(E),
    /// What lightningcss reports: invalid CSS, or `@import`s it can't merge.
    Css(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Location {
    pub file: String,
    /// Lines from 1, columns from 1 in UTF-16 units.
    pub line: u32,
    pub column: u32,
}

/// Bundles the stylesheet at `entry` for `targets`: the oldest version of
/// each browser, as `[major, minor, patch]` by Site config's key for it.
/// `scopes` gives the attribute that scopes each Sidecar it imports (ADR
/// 0006); with any, `:global()` parses.
pub fn bundle<P: Project>(
    project: &P,
    entry: &str,
    targets: &BTreeMap<String, [u32; 3]>,
    scopes: &BTreeMap<String, String>,
) -> Result<Bundle, Box<Error<P::Error>>> {
    let provider = Provider {
        project,
        texts: FrozenMap::new(),
    };
    let options = ParserOptions {
        error_recovery: false,
        css_modules: (!scopes.is_empty()).then(scope::modules),
        ..ParserOptions::default()
    };
    let mut bundler = Bundler::new(&provider, None, options);
    let mut sheet = bundler.bundle(Path::new(entry)).map_err(|e| {
        let at = e.loc.map(location);
        let kind = match e.kind {
            lightningcss::bundler::BundleErrorKind::ResolverError(e) => ErrorKind::Project(e),
            kind => ErrorKind::Css(kind.to_string()),
        };
        Error { kind, at }
    })?;
    let mut compounds = Vec::new();
    if !scopes.is_empty() {
        scope::scope(&mut sheet.rules, &sheet.sources, scopes, &mut compounds).map_err(
            |(message, at)| {
                Box::new(Error {
                    kind: ErrorKind::Css(message),
                    at: Some(at),
                })
            },
        )?;
    }
    let targets = self::targets(targets);
    sheet
        .minify(MinifyOptions {
            targets,
            ..MinifyOptions::default()
        })
        .map_err(|e| css_error(e.kind.to_string(), e.loc))?;
    let printed = sheet
        .to_css(PrinterOptions {
            minify: true,
            targets,
            analyze_dependencies: Some(DependencyOptions {
                remove_imports: false,
            }),
            ..PrinterOptions::default()
        })
        .map_err(|e| css_error(e.kind.to_string(), e.loc))?;
    let urls = printed
        .dependencies
        .expect("asked for")
        .into_iter()
        .map(|d| {
            let (url, placeholder, loc) = match d {
                Dependency::Url(d) => (d.url, d.placeholder, d.loc),
                Dependency::Import(d) => (d.url, d.placeholder, d.loc),
            };
            Url {
                url,
                placeholder,
                file: loc.file_path,
                line: loc.start.line,
                column: loc.start.column,
            }
        })
        .collect();
    Ok(Bundle {
        code: printed.code,
        urls,
        compounds,
    })
}

fn css_error<E>(message: String, at: Option<ErrorLocation>) -> Box<Error<E>> {
    Box::new(Error {
        kind: ErrorKind::Css(message),
        at: at.map(location),
    })
}

fn location(loc: ErrorLocation) -> Location {
    Location {
        file: loc.filename,
        line: loc.line + 1,
        column: loc.column,
    }
}

/// Site config's keys for browsers, in lightningcss's terms. `safari` is
/// both Safaris, as Tailwind's own targets have it.
fn targets(versions: &BTreeMap<String, [u32; 3]>) -> Targets {
    let mut browsers = Browsers::default();
    for (browser, [major, minor, patch]) in versions {
        let version = Some(major << 16 | minor << 8 | patch);
        match browser.as_str() {
            "chrome" => browsers.chrome = version,
            "edge" => browsers.edge = version,
            "firefox" => browsers.firefox = version,
            "safari" => {
                browsers.safari = version;
                browsers.ios_saf = version;
            }
            "opera" => browsers.opera = version,
            "samsung" => browsers.samsung = version,
            _ => unreachable!("Site config allows no other browser"),
        }
    }
    Targets::from(browsers)
}

/// lightningcss's view of the project. It hands out `&str`s that live as
/// long as it does, so each file's text is kept once read.
struct Provider<'p, P> {
    project: &'p P,
    texts: FrozenMap<PathBuf, String>,
}

impl<P: Project> SourceProvider for Provider<'_, P> {
    type Error = P::Error;

    fn read<'a>(&'a self, file: &Path) -> Result<&'a str, P::Error> {
        if let Some(text) = self.texts.get(file) {
            return Ok(text);
        }
        let text = self.project.read(path(file))?;
        Ok(self.texts.insert(file.to_path_buf(), text))
    }

    fn resolve(&self, specifier: &str, from: &Path) -> Result<ResolveResult, P::Error> {
        Ok(match self.project.import(specifier, path(from))? {
            Import::File(path) => ResolveResult::File(path.into()),
            Import::External(url) => ResolveResult::External(url),
        })
    }
}

/// Every path lightningcss has came from [`Project`] as a `&str`.
fn path(file: &Path) -> &str {
    file.to_str().expect("a path from Project")
}

/// npm packages in `node_modules`, found as Tailwind finds a stylesheet's
/// `@import` of one (#20): through the `style` condition of `exports`, or
/// else the `style` field.
pub struct Packages {
    root: PathBuf,
    resolver: Resolver,
}

impl Packages {
    /// `root` is the project's folder.
    pub fn new(root: &Path) -> Packages {
        let resolver = Resolver::new(ResolveOptions {
            condition_names: vec!["style".into()],
            main_fields: vec!["style".into()],
            extensions: vec![".css".into()],
            ..ResolveOptions::default()
        });
        // Resolved paths are real paths, so a symlinked project root must
        // be too for them to start with it.
        let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        Packages { root, resolver }
    }

    /// The file `specifier` names, imported from the project file `from`,
    /// by its path from the project root. Bun's isolated layout links each
    /// package from `node_modules/.bun`, so a package's files are at their
    /// real paths there.
    pub fn resolve(&self, specifier: &str, from: &str) -> Result<String, String> {
        let dir = self.root.join(from);
        let dir = dir.parent().expect("a file is in a folder");
        let resolved = self
            .resolver
            .resolve(dir, specifier)
            .map_err(|e| e.to_string())?;
        let relative = resolved
            .path()
            .strip_prefix(&self.root)
            .map_err(|_| format!("`{specifier}` resolves outside the project"))?;
        relative
            .to_str()
            .map(|p| p.replace(std::path::MAIN_SEPARATOR, "/"))
            .ok_or_else(|| format!("`{specifier}` resolves to a path that is not UTF-8"))
    }
}
