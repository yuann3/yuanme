//! The project as the assets stage sees it: what a URL in a Page or a
//! stylesheet names, and the files lightningcss reads.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use yuanme_assets::{CssProject, Import, Packages, is_tailwind};
use yuanme_content::config;
use yuanme_diag::{Code, Diagnostic, Span, codes};

use super::{SIDECARS_ENTRY, file_name};
use crate::hashing::Asset;
use crate::url::{has_scheme, normalize, percent_decode, suffix};
use crate::{Body, Inputs};

/// What a URL names.
pub(super) enum Target {
    /// Another origin, a `data:` URL or a fragment alone: left as written.
    External,
    /// A Static file or a Page, by its URL path from the root.
    Site(String),
    /// A project file, by its path from the project root.
    Source(String),
}

/// lightningcss's and the Pages' view of the project.
pub(super) struct Project<'a> {
    pub(super) inputs: &'a Inputs,
    /// The URL of every Page, `/blog/pew/` and `/blog/pew/index.html` both.
    pub(super) site: BTreeSet<String>,
    /// `None` for Inputs held in memory, which have no `node_modules`.
    pub(super) packages: Option<Packages>,
    /// The text of [`SIDECARS_ENTRY`].
    pub(super) sidecars: String,
    /// The Tailwind CLI's output for each stylesheet that imports Tailwind,
    /// which lightningcss reads in place of its text.
    pub(super) tailwind: BTreeMap<String, String>,
}

impl Project<'_> {
    /// What `url` names, written in the file at the URL `base`. A path
    /// from `/` names a file from the project's root, or a Static file
    /// without its `static/`; any other path is relative to `base` (#20).
    pub(super) fn resolve(&self, url: &str, base: &str) -> Result<Target, Problem> {
        let path = &url[..url.len() - suffix(url).len()];
        if path.is_empty() || url.starts_with("//") || has_scheme(url) {
            return Ok(Target::External);
        }
        let absolute = if path.starts_with('/') {
            normalize(path)
        } else {
            let dir = &base[..base
                .rfind('/')
                .expect("`base` is a URL path from the root, so it has a `/`")
                + 1];
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
    pub(super) fn bytes(&self, path: &str) -> Result<Arc<[u8]>, Problem> {
        if let Some(bytes) = self.inputs.files.get(path).and_then(|i| i.bytes.clone()) {
            return Ok(bytes);
        }
        let root = self
            .inputs
            .root
            .as_deref()
            .expect("a file that isn't in Inputs is a node_modules file, which is_source finds only under a root");
        std::fs::read(root.join(path))
            .map(Arc::from)
            .map_err(|e| Problem::new(&codes::ASSET_NOT_FOUND, format!("can't read `{path}`: {e}")))
    }

    /// A project file's text, if it is UTF-8.
    pub(super) fn text(&self, path: &str) -> Option<String> {
        let bytes = self.bytes(path).ok()?;
        String::from_utf8(bytes.to_vec()).ok()
    }

    /// Whether the project names packages it hasn't installed.
    pub(super) fn dependencies_missing(&self) -> bool {
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

impl CssProject for Project<'_> {
    type Error = Problem;

    fn read(&self, path: &str) -> Result<String, Problem> {
        if path == SIDECARS_ENTRY {
            return Ok(self.sidecars.clone());
        }
        if let Some(output) = self.tailwind.get(path) {
            return Ok(output.clone());
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
        // A stylesheet that imports Tailwind is the Tailwind CLI's to build,
        // and only one a Page links gets to it.
        if is_tailwind(specifier) {
            let message =
                format!("`@import \"{specifier}\"` is in a stylesheet that no Template links");
            return Err(Problem::new(&codes::ASSET_TAILWIND_ERROR, message)
                .help("import Tailwind from the stylesheet a `<link rel=\"stylesheet\">` names")
                .url(specifier));
        }
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
pub(super) struct Problem {
    pub(super) code: &'static Code,
    message: String,
    help: Option<String>,
    /// The URL as written, which a Diagnostic in a stylesheet covers.
    pub(super) url: String,
    /// The file, and the byte in it, that the Problem is in, when that is
    /// not where lightningcss reports it: the importing stylesheet.
    pub(super) file: Option<(String, usize)>,
}

impl Problem {
    pub(super) fn new(code: &'static Code, message: String) -> Problem {
        Problem {
            code,
            message,
            help: None,
            url: String::new(),
            file: None,
        }
    }

    pub(super) fn help(mut self, help: impl Into<String>) -> Problem {
        self.help = Some(help.into());
        self
    }

    pub(super) fn url(mut self, url: &str) -> Problem {
        self.url = url.to_string();
        self
    }

    pub(super) fn file(mut self, path: &str, at: usize) -> Problem {
        self.file = Some((path.to_string(), at));
        self
    }

    pub(super) fn diagnostic(self, span: Span) -> Diagnostic {
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

/// A file a URL names, copied as is under its hash.
pub(super) fn copy(project: &Project<'_>, path: &str) -> Result<Asset, Problem> {
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
