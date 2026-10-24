//! The one seam every caller crosses: `build` turns a snapshot of the
//! project into every file of the Site, in memory (ADR 0011).

mod cache;
mod link;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use yuanme_content::config::{self, Config};
use yuanme_content::site::{self, Project, RouteKind, Site};
use yuanme_diag::{Code, Diagnostic, FileId, Files, Level, Span, codes};

pub use cache::Cache;
use link::link;

/// A snapshot of the project's files, by path relative to the project root
/// with `/` separators. It is everything a build reads.
#[derive(Clone, Default)]
pub struct Inputs {
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
    Dev,
}

/// Every file of one Site, or the Diagnostics that stopped it.
pub struct Build {
    /// By path in the output directory, such as `blog/pew/index.html`.
    pub files: BTreeMap<String, Output>,
    pub diagnostics: Vec<Diagnostic>,
    /// The files the Diagnostics point into.
    pub sources: Files,
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
    let failed = |diagnostics, sources| Build {
        files: BTreeMap::new(),
        diagnostics,
        sources,
    };
    let (config, mut diagnostics) = site_config(inputs, cache.engine_version(), &mut sources);
    let Some(config) = config else {
        return failed(diagnostics, sources);
    };
    // Entries are parsed afresh each build until #47 caches them. `mode` is
    // also for images on request (#62) and the unsubset font (#52).
    let (site, more) = resolve(inputs, &config, mode == Mode::Dev, &mut sources);
    diagnostics.extend(more);
    let Some(site) = site else {
        return failed(diagnostics, sources);
    };
    let statics = static_files(inputs);
    diagnostics.extend(static_collisions(&config, &site, &statics));
    if diagnostics.iter().any(|d| d.level == Level::Error) {
        return failed(diagnostics, sources);
    }
    // The assets stage (#56) runs alongside render and hands link its
    // bundles.
    let mut files = link(render(&site), Vec::new());
    files.extend(statics);

    let build = Build {
        files,
        diagnostics,
        sources,
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

/// Pages with asset placeholders, by output path. Templates arrive with #54.
fn render(_site: &Site) -> Vec<(String, Vec<u8>)> {
    Vec::new()
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
