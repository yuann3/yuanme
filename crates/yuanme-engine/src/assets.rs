//! The assets stage (#20): the stylesheets and files that Pages name,
//! found in their rendered HTML, and the Site-wide Sidecar stylesheet.
//! Each becomes an [`Asset`] that the link pass names by its hash.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use rayon::prelude::*;
use yuanme_assets::css::{self, Import, Packages};
use yuanme_assets::html::{self, Kind};
use yuanme_content::config::{self, Config};
use yuanme_diag::{Applicability, Code, Diagnostic, Edit, FileId, Files, Span, codes};
use yuanme_render::template::Page;

use crate::link::Asset;
use crate::{Body, Inputs};

/// The stylesheet holding every used Template's Sidecar CSS.
const SIDECARS: &str = "sidecars.css";
/// Where that stylesheet's `@import`s are, as lightningcss sees it. NUL
/// can't be in a project file's name.
const SIDECARS_ENTRY: &str = "\0sidecars.css";

pub(crate) struct Stage {
    /// Each Page's bytes, with placeholders for the assets it names.
    pub pages: Vec<(String, Vec<u8>)>,
    pub assets: Vec<Asset>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Stage {
    fn failed(diagnostics: Diagnostics) -> Stage {
        Stage {
            pages: Vec::new(),
            assets: Vec::new(),
            diagnostics: diagnostics.0,
        }
    }
}

pub(crate) fn assets(
    inputs: &Inputs,
    config: &Config,
    pages: Vec<Page>,
    templates: &BTreeMap<String, FileId>,
    sources: &mut Files,
) -> Stage {
    let mut site = BTreeSet::new();
    for page in &pages {
        site.insert(format!("/{}", page.output));
        if let Some(dir) = page.output.strip_suffix("index.html") {
            site.insert(format!("/{dir}"));
        }
    }
    let sidecars: BTreeSet<String> = pages
        .iter()
        .flat_map(|p| &p.templates)
        .map(|t| {
            format!(
                "templates/{}.css",
                t.rsplit_once('.').map_or(&**t, |(s, _)| s)
            )
        })
        .filter(|path| inputs.files.contains_key(path))
        .collect();
    let project = Project {
        inputs,
        site: &site,
        packages: inputs.root.as_deref().map(Packages::new),
        sidecars: sidecars
            .iter()
            .map(|p| format!("@import {};\n", css_string(&format!("/{p}"))))
            .collect(),
    };
    let mut diagnostics = Diagnostics::default();

    // What each Page names.
    let scans: Vec<Option<html::Scan>> = pages
        .par_iter()
        .map(|p| p.output.ends_with(".html").then(|| html::scan(&p.text)))
        .collect();
    let mut entries: Vec<String> = Vec::new();
    let mut copies: BTreeSet<String> = BTreeSet::new();
    // Per Page: (where, what goes there, keeping the URL's `?` and `#`).
    let mut edits: Vec<Vec<(Range<usize>, Use)>> = vec![Vec::new(); pages.len()];
    for (i, (page, scan)) in pages.iter().zip(&scans).enumerate() {
        let Some(scan) = scan else { continue };
        let base = format!("/{}", page.output);
        for r in &scan.references {
            let at_template = || reference_span(page, &page.text[r.at.clone()], templates, sources);
            let path = match project.resolve(r.url.trim(), &base) {
                Ok(Target::Source(path)) => path,
                Ok(_) => continue,
                Err(problem) => {
                    let (span, _) = at_template();
                    diagnostics.push(problem.diagnostic(span).note(note(page)));
                    continue;
                }
            };
            let use_ = match r.kind {
                Kind::Classic => {
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
                    diagnostics.push(d.note(note(page)));
                    continue;
                }
                // #58 bundles it.
                Kind::Module => continue,
                Kind::Stylesheet => Use::Bundle(path),
                // A preload of a stylesheet must fetch the bundle that
                // `<link rel=stylesheet>` gets.
                Kind::Link if path.ends_with(".css") => Use::Bundle(path),
                Kind::Link => {
                    copies.insert(path.clone());
                    Use::Copy(path)
                }
            };
            if let Use::Bundle(path) = &use_
                && !entries.contains(path)
            {
                entries.push(path.clone());
            }
            edits[i].push((r.at.clone(), use_));
        }
    }

    // Every stylesheet, bundled.
    let mut bundles: Vec<(String, &str)> = entries
        .iter()
        .map(|e| (file_name(e).to_string(), e.as_str()))
        .collect();
    if !sidecars.is_empty() {
        bundles.push((SIDECARS.to_string(), SIDECARS_ENTRY));
    }
    let bundled: Vec<_> = bundles
        .par_iter()
        .map(|(_, entry)| css::bundle(&project, entry, &config.build.targets))
        .collect();
    let mut css_files = CssFiles::default();
    // Each bundle's code, and where its `url()`s go.
    let mut codes: Vec<(String, Vec<(String, Use)>)> = Vec::new();
    for ((_, entry), result) in bundles.iter().zip(bundled) {
        let bundle = match result {
            Ok(bundle) => bundle,
            Err(e) => {
                diagnostics.push(css_files.error(*e, entry, &project, sources));
                continue;
            }
        };
        let mut urls = Vec::new();
        for url in &bundle.urls {
            let use_ = match project.resolve(&url.url, &format!("/{}", url.file)) {
                Ok(Target::External) => Use::Url(url.url.clone()),
                // The bundle is served from `/assets/`, so a relative URL
                // must become one from the root.
                Ok(Target::Site(path)) => Use::Url(path + suffix(&url.url)),
                Ok(Target::Source(path)) => {
                    copies.insert(path.clone());
                    Use::Copy(path)
                }
                Err(problem) => {
                    let span = css_files
                        .span(&project, sources, &url.file, url.line, url.column, &url.url);
                    diagnostics.push(problem.diagnostic(span));
                    continue;
                }
            };
            urls.push((url.placeholder.clone(), use_));
        }
        codes.push((bundle.code, urls));
    }
    if diagnostics.failed() {
        return Stage::failed(diagnostics);
    }

    // The link pass needs the copies first: bundles name them.
    let copies: Vec<String> = copies.into_iter().collect();
    let index = |use_: &Use| -> usize {
        match use_ {
            Use::Copy(path) => copies.binary_search(path).expect("listed"),
            Use::Bundle(path) => {
                copies.len() + entries.iter().position(|e| e == path).expect("listed")
            }
            Use::Url(_) => unreachable!("a URL is no asset"),
        }
    };
    let mut assets = Vec::new();
    for path in &copies {
        match copy(&project, path) {
            Ok(asset) => assets.push(asset),
            Err(problem) => diagnostics.push(problem.diagnostic(Span {
                file: config.file,
                range: 0..0,
            })),
        }
    }
    if diagnostics.failed() {
        return Stage::failed(diagnostics);
    }
    for ((name, _), (code, urls)) in bundles.iter().zip(codes) {
        let mut code = code;
        for (placeholder, use_) in urls {
            let url = match &use_ {
                Use::Url(url) => css_string(url),
                _ => format!("\"{}\"", placeholder_of(index(&use_))),
            };
            code = code.replacen(&format!("\"{placeholder}\""), &url, 1);
        }
        assets.push(Asset::Linked {
            name: name.clone(),
            bytes: code.into_bytes(),
        });
    }
    let sidecars_link = (!sidecars.is_empty()).then(|| {
        format!(
            "<link rel=\"stylesheet\" href=\"{}\">",
            placeholder_of(copies.len() + entries.len())
        )
    });

    let pages = pages
        .into_iter()
        .zip(scans)
        .zip(edits)
        .map(|((page, scan), edits)| {
            let mut edits: Vec<(Range<usize>, String)> = edits
                .into_iter()
                .map(|(at, use_)| {
                    let with = placeholder_of(index(&use_)) + suffix(page.text[at.clone()].trim());
                    (at, with)
                })
                .collect();
            if let (Some(link), Some(at)) = (&sidecars_link, scan.and_then(|s| s.stylesheet_at)) {
                edits.push((at..at, link.clone()));
                edits.sort_by_key(|(at, _)| at.start);
            }
            (page.output, splice(&page.text, &edits))
        })
        .collect();
    Stage {
        pages,
        assets,
        diagnostics: diagnostics.0,
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

#[derive(Clone)]
enum Use {
    Bundle(String),
    Copy(String),
    /// Written as is.
    Url(String),
}

fn placeholder_of(index: usize) -> String {
    format!("\0asset:{index}\0")
}

/// The `?query#fragment` of a URL, kept after its path is replaced.
fn suffix(url: &str) -> &str {
    url.find(['?', '#']).map_or("", |i| &url[i..])
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn note(page: &Page) -> String {
    format!("in the Page written to `{}`", page.output)
}

/// A file a URL names, copied as is under its hash.
fn copy(project: &Project<'_>, path: &str) -> Result<Asset, Problem> {
    let name = file_name(path).to_string();
    if let Some(input) = project.inputs.files.get(path) {
        // A streamed image is copied from the project, not held in memory.
        let body = match &input.bytes {
            Some(bytes) => Body::Bytes(bytes.clone()),
            None => Body::Input(path.to_string()),
        };
        return Ok(Asset::Copy {
            name,
            body,
            hash: input.hash,
        });
    }
    let bytes = project.bytes(path)?;
    Ok(Asset::Copy {
        name,
        hash: blake3::hash(&bytes),
        body: Body::Bytes(bytes),
    })
}

/// What a URL names.
enum Target {
    /// Another origin, a `data:` URL or a fragment alone: left as written.
    External,
    /// A Static file or a Page, by its URL path from the root.
    Site(String),
    /// A project file, by its path from the project root.
    Source(String),
}

/// lightningcss's and the Pages' view of the project.
struct Project<'a> {
    inputs: &'a Inputs,
    /// The URL of every Page, `/blog/pew/` and `/blog/pew/index.html` both.
    site: &'a BTreeSet<String>,
    /// `None` for Inputs held in memory, which have no `node_modules`.
    packages: Option<Packages>,
    /// The text of [`SIDECARS_ENTRY`].
    sidecars: String,
}

impl Project<'_> {
    /// What `url` names, written in the file at the URL `base`. A path
    /// from `/` names a file from the project's root, or a Static file
    /// without its `static/`; any other path is relative to `base` (#20).
    fn resolve(&self, url: &str, base: &str) -> Result<Target, Problem> {
        let path = &url[..url.len() - suffix(url).len()];
        if path.is_empty() || url.starts_with("//") || has_scheme(url) {
            return Ok(Target::External);
        }
        let absolute = if path.starts_with('/') {
            normalize(path)
        } else {
            let dir = &base[..base.rfind('/').expect("from the root") + 1];
            normalize(&format!("{dir}{path}"))
        };
        let problem = |code, message| Problem::new(code, message).url(url);
        let Some(file) = percent_decode(&absolute[1..]) else {
            let message = format!("`{url}` is not a valid URL");
            return Err(problem(&codes::ASSET_NOT_FOUND, message));
        };
        let is_source = !file.starts_with("static/") && self.is_source(&file);
        let is_static = self.inputs.files.contains_key(&format!("static/{file}"));
        let is_page = self.site.contains(&absolute);
        match (is_source, is_static, is_page) {
            (true, false, false) => Ok(Target::Source(file)),
            (false, true, _) | (false, false, true) => Ok(Target::Site(absolute)),
            (true, true, _) => {
                let message =
                    format!("`{url}` names both `{file}` and the Static file `static/{file}`");
                Err(problem(&codes::ASSET_AMBIGUOUS_URL, message)
                    .help("delete or rename one of them"))
            }
            (true, false, true) => {
                let message = format!("`{url}` names both `{file}` and a Page of the Site");
                Err(problem(&codes::ASSET_AMBIGUOUS_URL, message).help(format!("rename `{file}`")))
            }
            (false, false, false) => {
                let message = format!("`{url}` names no file of the project");
                let problem = problem(&codes::ASSET_NOT_FOUND, message);
                Err(match file.strip_prefix("static/") {
                    Some(rest) => problem.help(format!(
                        "a Static file's URL leaves out `static/`: `/{rest}`"
                    )),
                    None => match self.similar(&file) {
                        Some(near) => problem.help(format!("did you mean `/{near}`?")),
                        None => problem,
                    },
                })
            }
        }
    }

    /// A project file's bytes, or else a package file's, read from
    /// `node_modules`.
    fn bytes(&self, path: &str) -> Result<Arc<[u8]>, Problem> {
        if let Some(bytes) = self.inputs.files.get(path).and_then(|i| i.bytes.clone()) {
            return Ok(bytes);
        }
        let root = self
            .inputs
            .root
            .as_deref()
            .expect("only Inputs::at reads node_modules");
        std::fs::read(root.join(path))
            .map(Arc::from)
            .map_err(|e| Problem::new(&codes::ASSET_NOT_FOUND, format!("can't read `{path}`: {e}")))
    }

    /// Whether the project names packages it hasn't installed.
    fn dependencies_missing(&self) -> bool {
        self.inputs.files.contains_key("package.json")
            && self
                .inputs
                .root
                .as_ref()
                .is_none_or(|root| !root.join("node_modules").is_dir())
    }

    fn is_source(&self, path: &str) -> bool {
        self.inputs.files.contains_key(path)
            || (path.starts_with("node_modules/")
                && self
                    .inputs
                    .root
                    .as_ref()
                    .is_some_and(|root| root.join(path).is_file()))
    }

    /// The project file or Static file a mistyped path most likely meant.
    fn similar(&self, path: &str) -> Option<String> {
        let candidates: Vec<&str> = self
            .inputs
            .files
            .keys()
            .map(|p| p.strip_prefix("static/").unwrap_or(p))
            .collect();
        config::similar(path, candidates).map(str::to_string)
    }
}

impl css::Project for Project<'_> {
    type Error = Problem;

    fn read(&self, path: &str) -> Result<String, Problem> {
        if path == SIDECARS_ENTRY {
            return Ok(self.sidecars.clone());
        }
        let bytes = self.bytes(path)?;
        match std::str::from_utf8(&bytes) {
            Ok(text) => Ok(text.to_string()),
            Err(e) => Err(Problem::new(
                &codes::ASSET_INVALID_CSS,
                format!("`{path}` is not UTF-8"),
            )
            .file(path, e.valid_up_to())),
        }
    }

    fn import(&self, specifier: &str, from: &str) -> Result<Import, Problem> {
        match self.resolve(specifier, &format!("/{from}")) {
            Ok(Target::Source(path)) => Ok(Import::File(path)),
            Ok(Target::Site(url)) => Ok(Import::External(url)),
            Ok(Target::External) => Ok(Import::External(specifier.to_string())),
            // `pkg/x.css` is a relative path first, as in postcss-import
            // and Tailwind, and then a package.
            Err(e) if e.code == &codes::ASSET_NOT_FOUND && !specifier.starts_with(['.', '/']) => {
                if self.dependencies_missing() {
                    let message = format!(
                        "`{specifier}` can't be found: the project has a `package.json` but no `node_modules`"
                    );
                    return Err(
                        Problem::new(&codes::ASSET_DEPENDENCIES_NOT_INSTALLED, message)
                            .help("run `bun install`")
                            .url(specifier),
                    );
                }
                let Some(packages) = &self.packages else {
                    return Err(e);
                };
                packages
                    .resolve(specifier, from)
                    .map(Import::File)
                    .map_err(|why| {
                        let message =
                            format!("`{specifier}` is no file of the project or package: {why}");
                        Problem::new(&codes::ASSET_NOT_FOUND, message)
                            .help("install the package with `bun install`")
                            .url(specifier)
                    })
            }
            Err(e) => Err(e),
        }
    }
}

#[derive(Debug)]
struct Problem {
    code: &'static Code,
    message: String,
    help: Option<String>,
    /// The URL as written, which a Diagnostic in a stylesheet covers.
    url: String,
    /// The file, and the byte in it, that the Problem is in, when that is
    /// not where lightningcss reports it: the importing stylesheet.
    file: Option<(String, usize)>,
}

impl Problem {
    fn new(code: &'static Code, message: String) -> Problem {
        Problem {
            code,
            message,
            help: None,
            url: String::new(),
            file: None,
        }
    }

    fn help(mut self, help: impl Into<String>) -> Problem {
        self.help = Some(help.into());
        self
    }

    fn url(mut self, url: &str) -> Problem {
        self.url = url.to_string();
        self
    }

    fn file(mut self, path: &str, at: usize) -> Problem {
        self.file = Some((path.to_string(), at));
        self
    }

    fn diagnostic(self, span: Span) -> Diagnostic {
        let d = Diagnostic::error(self.code, self.message, span);
        match self.help {
            Some(help) => d.help(help),
            None => d,
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Problem {}

/// One Diagnostic for each mistake, though many Pages make it.
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
        !self.0.is_empty()
    }
}

/// Where a URL in a rendered Page was written: the first of the Page's
/// Templates whose text holds it, or else the start of the Page's own, and
/// whether it was found.
fn reference_span(
    page: &Page,
    raw: &str,
    templates: &BTreeMap<String, FileId>,
    sources: &Files,
) -> (Span, bool) {
    let files = page.templates.iter().filter_map(|t| templates.get(t));
    for &file in files.clone() {
        if let Some(start) = sources.text(file).find(raw) {
            let span = Span {
                file,
                range: start..start + raw.len(),
            };
            return (span, true);
        }
    }
    let file = *files
        .clone()
        .next()
        .expect("an HTML Page has a Template file");
    (Span { file, range: 0..0 }, false)
}

/// The edit that adds `type="module"` to the `<script>` whose `src` is at
/// `span`.
fn module_type(span: &Span, sources: &Files) -> Option<Edit> {
    let text = sources.text(span.file);
    let tag = text[..span.range.start].rfind("<script")? + "<script".len();
    Some(Edit {
        span: Span {
            file: span.file,
            range: tag..tag,
        },
        replacement: " type=\"module\"".into(),
    })
}

/// Stylesheets added to the Files, for Diagnostics in them.
#[derive(Default)]
struct CssFiles(BTreeMap<String, FileId>);

impl CssFiles {
    /// A bundle's error, in the stylesheet it is in.
    fn error(
        &mut self,
        e: css::Error<Problem>,
        entry: &str,
        project: &Project<'_>,
        sources: &mut Files,
    ) -> Diagnostic {
        let (needle, file) = match &e.kind {
            css::ErrorKind::Project(problem) => (problem.url.as_str(), problem.file.clone()),
            css::ErrorKind::Css(_) => ("", None),
        };
        let span = match (file, &e.at) {
            (Some((path, at)), _) => Span {
                file: self.file(project, sources, &path),
                range: at..at,
            },
            (None, Some(at)) => self.span(project, sources, &at.file, at.line, at.column, needle),
            (None, None) => self.span(project, sources, entry, 1, 1, needle),
        };
        match e.kind {
            css::ErrorKind::Project(problem) => problem.diagnostic(span),
            css::ErrorKind::Css(message) => {
                Diagnostic::error(&codes::ASSET_INVALID_CSS, message, span)
            }
        }
    }

    /// A lossy copy of a file that isn't UTF-8 matches its bytes up to the
    /// first bad one.
    fn file(&mut self, project: &Project<'_>, sources: &mut Files, path: &str) -> FileId {
        *self.0.entry(path.to_string()).or_insert_with(|| {
            let bytes = project.bytes(path).unwrap_or_else(|_| Arc::from([]));
            sources.add(path, String::from_utf8_lossy(&bytes))
        })
    }

    /// `line` from 1 and `column` from 1 in UTF-16 units, as lightningcss
    /// reports them; `needle`, when found on the line from there, is what
    /// the span covers.
    fn span(
        &mut self,
        project: &Project<'_>,
        sources: &mut Files,
        path: &str,
        line: u32,
        column: u32,
        needle: &str,
    ) -> Span {
        let file = self.file(project, sources, path);
        let text = sources.text(file);
        let start: usize = text
            .split_inclusive('\n')
            .take(line.saturating_sub(1) as usize)
            .map(str::len)
            .sum();
        let line_text = text[start..].split('\n').next().unwrap_or("");
        let mut units = 1;
        let mut at = line_text.len();
        for (i, c) in line_text.char_indices() {
            if units >= column as usize {
                at = i;
                break;
            }
            units += c.len_utf16();
        }
        let from = start + at;
        let range = match line_text[at..].find(needle).filter(|_| !needle.is_empty()) {
            Some(i) => from + i..from + i + needle.len(),
            None => from..from,
        };
        Span { file, range }
    }
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

fn has_scheme(url: &str) -> bool {
    let Some(colon) = url.find(':') else {
        return false;
    };
    let scheme = &url[..colon];
    scheme.starts_with(|c: char| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
}

/// `/a/./b/../c` → `/a/c`. A `..` above the root stays at the root, as
/// browsers resolve it.
fn normalize(path: &str) -> String {
    let segments: Vec<&str> = path[1..].split('/').collect();
    let mut out: Vec<&str> = Vec::new();
    for (i, segment) in segments.iter().enumerate() {
        let last = i + 1 == segments.len();
        match *segment {
            "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
        if last && matches!(*segment, "." | "..") {
            out.push("");
        }
    }
    format!("/{}", out.join("/"))
}

fn percent_decode(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = path.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}
