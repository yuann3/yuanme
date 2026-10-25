//! Scripts (#20): every Script of a Site in one rolldown build, as ESM with
//! shared chunks. The Engine hands over the project files a Script may
//! import; rolldown reads packages from `node_modules` itself.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rolldown::plugin::{
    HookLoadArgs, HookLoadOutput, HookLoadReturn, HookResolveIdArgs, HookResolveIdOutput,
    HookResolveIdReturn, HookUsage, Plugin, PluginContext, PluginContextResolveOptions,
    SharedLoadPluginContext,
};
use rolldown::{
    Bundler, BundlerOptions, BundlerTransformOptions, CommentsOptions, Either, InputItem,
    ModuleType, OutputFormat, Platform, PreserveEntrySignatures, RawMinifyOptions, TsConfig,
};
use rolldown_common::{Output, ResolvedExternal};
use rolldown_error::{BuildDiagnostic, DiagnosticOptions, EventKind};

/// The project, as a build of its Scripts reads it.
pub struct Project {
    /// The project's folder, whose `node_modules` packages come from. `None`
    /// for a project held in memory, which has no packages.
    pub root: Option<PathBuf>,
    /// Every file a Script may import, by its path from the project root:
    /// Scripts, and JSON modules. A Data file is one too, already turned
    /// into JSON, under its own name.
    pub files: BTreeMap<String, Arc<[u8]>>,
}

/// The virtual module of the Router's API (#20). With no Router on the
/// Page, `navigate` is a full load; #61 swaps in the Router's own.
const ROUTER: &str = "yuanme:router";
const ROUTER_ID: &str = "\0yuanme:router";
const ROUTER_CODE: &str = "export function navigate(url, { history = \"auto\" } = {}) {\n  if (history === \"replace\") location.replace(url);\n  else location.assign(url);\n}\n";

pub struct Chunk {
    /// rolldown's file name for it, such as `shared-Dig5JG4F.js`, which the
    /// code of the chunks that import it holds as `"./<file>"`.
    pub file: String,
    /// Its name before hashing, such as `shared`.
    pub name: String,
    /// The entry it is, by its index in the entries, or `None` for a shared
    /// chunk or one only imported with `import()`.
    pub entry: Option<usize>,
    /// Minified.
    pub code: String,
    /// The chunks it imports, by index, and those it imports with
    /// `import()`.
    pub imports: Vec<usize>,
    pub dynamic_imports: Vec<usize>,
    /// What went into it: each module by its path from the project root,
    /// with its size in the chunk's code.
    pub modules: Vec<(String, usize)>,
}

pub struct Bundles {
    pub chunks: Vec<Chunk>,
    pub warnings: Vec<Problem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub kind: ProblemKind,
    pub message: String,
    /// The file it is in, by its path from the project root, which may be
    /// in `node_modules`. `None` when rolldown names none.
    pub file: Option<String>,
    pub at: At,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemKind {
    /// An import that names nothing. `bare` for a package name, such as
    /// `nanoid`, rather than a path.
    NotFound {
        bare: bool,
    },
    /// An import of the stylesheet at this path from the project root.
    Css {
        stylesheet: String,
    },
    Jsx,
    Invalid,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum At {
    /// The specifier as written, found in the file's text.
    Specifier(String),
    /// Lines from 1, columns from 0 in UTF-16 units, as rolldown has them.
    Position {
        line: u32,
        column: u32,
    },
    Byte(usize),
    Start,
}

/// Bundles `entries`, project files by their path from the project root,
/// for `targets`: the oldest version of each browser, as
/// `[major, minor, patch]` by Site config's key for it.
pub fn bundle(
    project: Project,
    entries: &[String],
    targets: &BTreeMap<String, [u32; 3]>,
) -> Result<Bundles, Vec<Problem>> {
    // Resolved paths are real paths, so a symlinked project root must be
    // too for them to start with it.
    let root = project
        .root
        .as_ref()
        .map(|r| r.canonicalize().unwrap_or_else(|_| r.clone()));
    let cwd = root.clone().unwrap_or_else(|| PathBuf::from("/"));
    let prefix = match &root {
        Some(root) => format!("{}/", root.to_string_lossy().trim_end_matches('/')),
        None => "/".to_string(),
    };
    let problems = Arc::new(Mutex::new(Vec::new()));
    let plugin = Files {
        has_packages: root.is_some(),
        files: project.files,
        prefix: prefix.clone(),
        problems: problems.clone(),
    };
    let ids: Vec<String> = entries.iter().map(|e| format!("{prefix}{e}")).collect();
    let options = BundlerOptions {
        input: Some(
            ids.iter()
                .map(|id| InputItem {
                    name: None,
                    import: id.clone(),
                })
                .collect(),
        ),
        cwd: Some(cwd.clone()),
        platform: Some(Platform::Browser),
        format: Some(OutputFormat::Esm),
        minify: Some(RawMinifyOptions::Bool(true)),
        // Licences stay; `@__PURE__` and the like only help a later
        // minifier, and there is none.
        comments: Some(CommentsOptions {
            legal: true,
            annotation: false,
            jsdoc: false,
        }),
        entry_filenames: Some("[name]-[hash].js".to_string().into()),
        chunk_filenames: Some("[name]-[hash].js".to_string().into()),
        // ADR 0013: `false` would drop a Sidecar's `mount`.
        preserve_entry_signatures: Some(PreserveEntrySignatures::ExportsOnly),
        // A tsconfig.json is for the type checker; read here, it would
        // change the output from outside the Inputs.
        tsconfig: Some(TsConfig::Auto(false)),
        transform: Some(BundlerTransformOptions {
            // JSX is a parse error, which `asset/jsx` reports.
            jsx: Some(Either::Left("disable".into())),
            target: Some(Either::Right(target_list(targets))),
            ..BundlerTransformOptions::default()
        }),
        ..BundlerOptions::default()
    };
    let diagnostic_options = DiagnosticOptions { cwd };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a tokio runtime");
    let result = runtime.block_on(async {
        let mut bundler = Bundler::with_plugins(options, vec![Plugin::new_shared(plugin)])?;
        let output = bundler.generate().await;
        bundler.close().await?;
        output
    });
    let mut problems = std::mem::take(&mut *problems.lock().expect("not poisoned"));
    let problem = |d: &BuildDiagnostic, kind| {
        let message = d.to_message_with(&diagnostic_options);
        let location = d
            .to_diagnostic_with(&diagnostic_options)
            .get_primary_location();
        let kind = match kind {
            ProblemKind::Invalid
                if matches!(d.kind(), EventKind::ParseError) && message.contains("JSX") =>
            {
                ProblemKind::Jsx
            }
            kind => kind,
        };
        let (file, at) = match location {
            Some((file, line, column, _)) => (
                Some(file),
                At::Position {
                    line: line as u32,
                    column: column as u32,
                },
            ),
            None => (d.id().map(|id| relative(&id, &prefix)), At::Start),
        };
        Problem {
            kind,
            message,
            file,
            at,
        }
    };
    let output = match result {
        Ok(output) => output,
        Err(errors) => {
            problems.extend(
                errors
                    .into_vec()
                    .iter()
                    .map(|d| problem(d, ProblemKind::Invalid)),
            );
            return Err(problems);
        }
    };
    let warnings: Vec<Problem> = output
        .warnings
        .iter()
        .map(|d| problem(d, ProblemKind::Warning))
        .collect();
    if !problems.is_empty() {
        problems.extend(warnings);
        return Err(problems);
    }
    let chunks: Vec<_> = output
        .assets
        .into_iter()
        .filter_map(|o| match o {
            Output::Chunk(chunk) => Some(chunk),
            Output::Asset(_) => None,
        })
        .collect();
    let index = |file: &str| chunks.iter().position(|c| c.filename.as_str() == file);
    let chunks = chunks
        .iter()
        .map(|c| Chunk {
            file: c.filename.to_string(),
            name: c.name.to_string(),
            entry: c
                .facade_module_id
                .as_ref()
                .filter(|_| c.is_entry)
                .and_then(|id| ids.iter().position(|e| e.as_str() == id.as_ref())),
            code: c.code.clone(),
            // An external import, such as `https://`, is no chunk.
            imports: c.imports.iter().filter_map(|f| index(f)).collect(),
            dynamic_imports: c.dynamic_imports.iter().filter_map(|f| index(f)).collect(),
            modules: c
                .modules
                .keys
                .iter()
                .zip(&c.modules.values)
                .map(|(id, m)| (relative(id, &prefix), m.rendered_length()))
                .collect(),
        })
        .collect();
    Ok(Bundles { chunks, warnings })
}

/// Site config's keys for browsers, in oxc's terms. `safari` is both
/// Safaris, as for lightningcss.
fn target_list(targets: &BTreeMap<String, [u32; 3]>) -> Vec<String> {
    let mut out = Vec::new();
    for (browser, [major, minor, patch]) in targets {
        let version = format!("{major}.{minor}.{patch}");
        out.push(format!("{browser}{version}"));
        if browser == "safari" {
            out.push(format!("ios{version}"));
        }
    }
    out
}

fn relative(id: &str, prefix: &str) -> String {
    if id == ROUTER_ID {
        return ROUTER.to_string();
    }
    id.strip_prefix(prefix).unwrap_or(id).to_string()
}

/// Project files for rolldown: it resolves and loads them from the Engine's
/// Inputs, and packages from `node_modules` as usual. Every import that
/// names nothing, or names a stylesheet, becomes a [`Problem`] and an
/// external, so one build finds all of them.
#[derive(Debug)]
struct Files {
    files: BTreeMap<String, Arc<[u8]>>,
    has_packages: bool,
    /// What a project file's module id starts with: the project root.
    prefix: String,
    problems: Arc<Mutex<Vec<Problem>>>,
}

impl Files {
    /// The project file a module id names, if it is one.
    fn project_file<'a>(&self, id: &'a str) -> Option<&'a str> {
        id.strip_prefix(&self.prefix)
            .filter(|path| !path.starts_with("node_modules/") && self.files.contains_key(*path))
    }

    /// The project file `specifier`, a path from `/` or relative to
    /// `importer`, names: as written, or as TypeScript's resolution finds
    /// it, without its extension or with `.js` for `.ts`.
    fn find(&self, specifier: &str, importer: &str) -> Option<String> {
        let path = if let Some(path) = specifier.strip_prefix('/') {
            normalize(path)?
        } else {
            let dir = importer.rsplit_once('/').map_or("", |(dir, _)| dir);
            normalize(&format!("{dir}/{specifier}"))?
        };
        let mut candidates = vec![path.clone()];
        for (js, ts) in [(".js", ".ts"), (".mjs", ".mts")] {
            if let Some(stem) = path.strip_suffix(js) {
                candidates.push(format!("{stem}{ts}"));
            }
        }
        for suffix in [".ts", ".js", "/index.ts", "/index.js"] {
            candidates.push(format!("{path}{suffix}"));
        }
        candidates.into_iter().find(|c| self.files.contains_key(c))
    }

    /// Records `problem` and keeps the import as it is written, so the build
    /// goes on to find the next.
    fn problem(
        &self,
        kind: ProblemKind,
        message: String,
        importer: &str,
        specifier: &str,
    ) -> HookResolveIdReturn {
        self.problems.lock().expect("not poisoned").push(Problem {
            kind,
            message,
            file: Some(
                importer
                    .strip_prefix(&self.prefix)
                    .unwrap_or(importer)
                    .to_string(),
            ),
            at: At::Specifier(specifier.to_string()),
        });
        Ok(Some(HookResolveIdOutput {
            id: specifier.into(),
            external: Some(ResolvedExternal::Bool(true)),
            ..HookResolveIdOutput::default()
        }))
    }
}

impl Plugin for Files {
    fn name(&self) -> Cow<'static, str> {
        Cow::Borrowed("yuanme:files")
    }

    fn register_hook_usage(&self) -> HookUsage {
        HookUsage::ResolveId | HookUsage::Load
    }

    async fn resolve_id(
        &self,
        ctx: &PluginContext,
        args: &HookResolveIdArgs<'_>,
    ) -> HookResolveIdReturn {
        let specifier = args.specifier;
        if specifier == ROUTER {
            return Ok(Some(HookResolveIdOutput::from_id(ROUTER_ID)));
        }
        let Some(importer) = args.importer else {
            // An entry, which the Engine has found already.
            return Ok(Some(HookResolveIdOutput::from_id(specifier)));
        };
        let is_path = specifier.starts_with('/')
            || specifier.starts_with("./")
            || specifier.starts_with("../");
        if let Some(from) = self.project_file(importer)
            && is_path
        {
            let Some(path) = self.find(specifier, from) else {
                let message = format!("`{specifier}` names no Script or Data file of the project");
                return self.problem(
                    ProblemKind::NotFound { bare: false },
                    message,
                    importer,
                    specifier,
                );
            };
            if path.ends_with(".css") {
                let message = format!("a Script imports the stylesheet `{path}`");
                return self.problem(
                    ProblemKind::Css { stylesheet: path },
                    message,
                    importer,
                    specifier,
                );
            }
            if path.ends_with(".jsx") || path.ends_with(".tsx") {
                let message = format!("`{path}` is JSX, which yuanme doesn't compile");
                return self.problem(ProblemKind::Jsx, message, importer, specifier);
            }
            return Ok(Some(HookResolveIdOutput::from_id(format!(
                "{}{path}",
                self.prefix
            ))));
        }
        // A URL, such as `https://` or `data:`, is rolldown's.
        if has_scheme(specifier) {
            return Ok(None);
        }
        let not_found = |why: String| {
            let bare = !is_path;
            let message = if bare {
                format!("`{specifier}` is no package in `node_modules`: {why}")
            } else {
                format!("`{specifier}` names nothing: {why}")
            };
            self.problem(ProblemKind::NotFound { bare }, message, importer, specifier)
        };
        if !self.has_packages {
            return not_found("the project has no folder on disk".into());
        }
        // A package, or a file a package imports. rolldown resolves it
        // again once this returns, as it would without this plugin.
        let options = PluginContextResolveOptions {
            import_kind: args.kind,
            ..PluginContextResolveOptions::default()
        };
        let resolved = match ctx
            .resolve(specifier, Some(importer), Some(options))
            .await?
        {
            Ok(resolved) => resolved,
            Err(e) => return not_found(e.to_string()),
        };
        if resolved.external.is_external() {
            return Ok(None);
        }
        let id: &str = resolved.id.as_ref();
        // The resolver looks in every `node_modules` up to the root of the
        // disk, but the build reads only the project's.
        let Some(path) = id.strip_prefix(&self.prefix) else {
            return not_found("it resolves outside the project".into());
        };
        if path.ends_with(".css") {
            let message = format!("a Script imports the stylesheet `{specifier}`");
            let stylesheet = path.to_string();
            return self.problem(
                ProblemKind::Css { stylesheet },
                message,
                importer,
                specifier,
            );
        }
        Ok(None)
    }

    async fn load(&self, _ctx: SharedLoadPluginContext, args: &HookLoadArgs<'_>) -> HookLoadReturn {
        if args.id == ROUTER_ID {
            return Ok(Some(HookLoadOutput {
                code: ROUTER_CODE.into(),
                module_type: Some(ModuleType::Js),
                ..HookLoadOutput::default()
            }));
        }
        let Some(path) = self.project_file(args.id) else {
            return Ok(None);
        };
        let bytes = &self.files[path];
        let code = match std::str::from_utf8(bytes) {
            Ok(code) => code,
            Err(e) => {
                self.problems.lock().expect("not poisoned").push(Problem {
                    kind: ProblemKind::Invalid,
                    message: format!("`{path}` is not UTF-8"),
                    file: Some(path.to_string()),
                    at: At::Byte(e.valid_up_to()),
                });
                ""
            }
        };
        let json = [".json", ".toml", ".yaml", ".yml"]
            .iter()
            .any(|e| path.ends_with(e));
        Ok(Some(HookLoadOutput {
            code: code.into(),
            module_type: json.then_some(ModuleType::Json),
            ..HookLoadOutput::default()
        }))
    }
}

/// `a/./b/../c` → `a/c`, or `None` above the root.
fn normalize(path: &str) -> Option<String> {
    let mut out: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                out.pop()?;
            }
            s => out.push(s),
        }
    }
    Some(out.join("/"))
}

fn has_scheme(url: &str) -> bool {
    url.split_once(':').is_some_and(|(scheme, _)| {
        scheme.len() > 1
            && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    })
}
