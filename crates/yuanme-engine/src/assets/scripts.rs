//! Scripts (#20): every module Script and Sidecar script that Pages load,
//! in one rolldown build with shared chunks.

use std::collections::BTreeMap;
use std::sync::Arc;

use yuanme_assets::js::{self, At, Problem, ProblemKind};
use yuanme_content::config::Config;
use yuanme_content::fields::{Image, Value};
use yuanme_content::site;
use yuanme_diag::{Applicability, Diagnostic, Edit, Files, Span, codes};

use super::{Diagnostics, Project, SourceFiles};
use crate::link::{Asset, hashed_path};
use crate::{Body, Bundle};

pub(super) struct Scripts {
    /// Every chunk, final, in the order of `entries`' chunks' build.
    pub assets: Vec<Asset>,
    /// The chunk of each entry.
    pub entries: Vec<usize>,
    /// The chunks each chunk imports, through every chunk between, in the
    /// order a browser finds them. Imports with `import()` aren't followed.
    pub preloads: Vec<Vec<usize>>,
    pub report: Vec<Bundle>,
}

/// The extensions of a Script.
pub(super) const EXTENSIONS: &[&str] = &[".ts", ".js", ".mts", ".mjs"];

/// Bundles `entries`, project files by their path from the project root.
/// `None` when it fails, with the Diagnostics in `diagnostics`.
pub(super) fn bundle(
    project: &Project<'_>,
    config: &Config,
    data: &BTreeMap<String, Value>,
    entries: &[String],
    sources: &mut Files,
    files: &mut SourceFiles,
    diagnostics: &mut Diagnostics,
) -> Option<Scripts> {
    if entries.is_empty() {
        return Some(Scripts {
            assets: Vec::new(),
            entries: Vec::new(),
            preloads: Vec::new(),
            report: Vec::new(),
        });
    }
    let inputs = project.inputs;
    // What a Script may import. A stylesheet or JSX file is here so that an
    // import of one is reported as what it is.
    let mut modules: BTreeMap<String, Arc<[u8]>> = inputs
        .files
        .iter()
        .filter(|(path, _)| {
            [".json", ".css", ".jsx", ".tsx"]
                .iter()
                .chain(EXTENSIONS)
                .any(|e| path.ends_with(e))
        })
        .filter_map(|(path, input)| Some((path.clone(), input.bytes.clone()?)))
        .collect();
    // A Data file is the value Templates read as `data.<name>`, with its
    // schema's defaults.
    let (_, data_files) = site::sources(config, inputs.files.keys().map(String::as_str));
    for path in data_files {
        let name = path["data/".len()..].rsplit_once('.').expect("has one").0;
        if let Some(value) = data.get(name) {
            let text = serde_json::to_string(&json(value)).expect("JSON");
            modules.insert(path.to_string(), Arc::from(text.into_bytes()));
        }
    }
    let result = js::bundle(
        js::Project {
            root: inputs.root.clone(),
            files: modules,
        },
        entries,
        &config.build.targets,
    );
    let bundles = match result {
        Ok(bundles) => bundles,
        Err(problems) => {
            for problem in problems {
                diagnostics.push(diagnostic(problem, project, config, sources, files));
            }
            return None;
        }
    };
    for warning in bundles.warnings {
        diagnostics.push(diagnostic(warning, project, config, sources, files));
    }
    let chunks = bundles.chunks;

    // A chunk's name is the hash of itself and every chunk it can reach.
    // Its bytes hold the names of the chunks it imports, so this changes
    // whenever they do, even where chunks import each other, which a hash
    // of its final bytes couldn't do. Its own file goes first, as chunks in
    // a cycle reach the same ones.
    let hashes: Vec<blake3::Hash> = (0..chunks.len())
        .map(|i| {
            let mut reach = vec![i];
            let mut next = 0;
            while let Some(&c) = reach.get(next) {
                next += 1;
                for &d in chunks[c].imports.iter().chain(&chunks[c].dynamic_imports) {
                    if !reach.contains(&d) {
                        reach.push(d);
                    }
                }
            }
            reach.sort_by(|a, b| chunks[*a].file.cmp(&chunks[*b].file));
            let mut hasher = blake3::Hasher::new();
            hasher.update(chunks[i].file.as_bytes());
            hasher.update(&[0]);
            for c in reach {
                hasher.update(chunks[c].file.as_bytes());
                hasher.update(&[0]);
                hasher.update(chunks[c].code.as_bytes());
                hasher.update(&[0]);
            }
            hasher.finalize()
        })
        .collect();
    let names: Vec<String> = chunks.iter().map(|c| format!("{}.js", c.name)).collect();
    let paths: Vec<String> = names
        .iter()
        .zip(&hashes)
        .map(|(name, hash)| hashed_path(name, hash))
        .collect();
    let mut assets = Vec::with_capacity(chunks.len());
    let mut report = Vec::with_capacity(chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        // Every chunk is in `/assets/`, so an import stays relative. Static
        // imports are quoted with `"` and `import()` with a backtick.
        let mut code = chunk.code.clone();
        for &d in chunk.imports.iter().chain(&chunk.dynamic_imports) {
            let to = paths[d].strip_prefix("assets/").expect("in /assets/");
            for quote in ['"', '\'', '`'] {
                code = code.replace(
                    &format!("{quote}./{}{quote}", chunks[d].file),
                    &format!("{quote}./{to}{quote}"),
                );
            }
        }
        report.push(Bundle {
            file: paths[i].clone(),
            size: code.len(),
            modules: chunk.modules.clone(),
        });
        assets.push(Asset::Copy {
            name: names[i].clone(),
            body: Body::Bytes(code.into_bytes().into()),
            hash: hashes[i],
        });
    }
    let preloads = (0..chunks.len())
        .map(|i| {
            let mut out: Vec<usize> = Vec::new();
            let mut stack: Vec<usize> = chunks[i].imports.iter().rev().copied().collect();
            while let Some(c) = stack.pop() {
                if c == i || out.contains(&c) {
                    continue;
                }
                out.push(c);
                stack.extend(chunks[c].imports.iter().rev());
            }
            out
        })
        .collect();
    let entries = (0..entries.len())
        .map(|e| {
            chunks
                .iter()
                .position(|c| c.entry == Some(e))
                .expect("every entry has a chunk")
        })
        .collect();
    Some(Scripts {
        assets,
        entries,
        preloads,
        report,
    })
}

/// A Data file's value as a JSON module: dates as ISO text, as Templates
/// see them.
fn json(v: &Value) -> serde_json::Value {
    use serde_json::Value as J;
    match v {
        Value::String(s) | Value::Url(s) => J::from(s.as_str()),
        Value::Int(i) => J::from(*i),
        Value::Float(f) => J::from(*f),
        Value::Bool(b) => J::from(*b),
        Value::Date(d) => J::from(d.to_string()),
        Value::Datetime(z) => J::from(z.strftime("%Y-%m-%dT%H:%M:%S%:z").to_string()),
        Value::Image(Image::Local(path) | Image::Remote(path)) => J::from(path.as_str()),
        Value::List(items) => J::Array(items.iter().map(json).collect()),
        Value::Object(o) => J::Object(o.iter().map(|(k, v)| (k.clone(), json(v))).collect()),
    }
}

fn diagnostic(
    problem: Problem,
    project: &Project<'_>,
    config: &Config,
    sources: &mut Files,
    files: &mut SourceFiles,
) -> Diagnostic {
    let span = match &problem.file {
        None => Span {
            file: config.file,
            range: 0..0,
        },
        Some(path) => match &problem.at {
            At::Specifier(specifier) => {
                let file = files.file(project, sources, path);
                let range = quoted(sources.text(file), specifier).unwrap_or(0..0);
                Span { file, range }
            }
            At::Position { line, column } => {
                files.span(project, sources, path, *line, column + 1, "")
            }
            At::Byte(at) => Span {
                file: files.file(project, sources, path),
                range: *at..*at,
            },
            At::Start => Span {
                file: files.file(project, sources, path),
                range: 0..0,
            },
        },
    };
    let message = problem.message;
    match problem.kind {
        ProblemKind::NotFound { bare: true } if project.dependencies_missing() => {
            let At::Specifier(specifier) = &problem.at else {
                unreachable!("an import has its specifier")
            };
            Diagnostic::error(
                &codes::ASSET_DEPENDENCIES_NOT_INSTALLED,
                format!(
                    "`{specifier}` can't be found: the project has a `package.json` but no `node_modules`"
                ),
                span,
            )
            .help("run `bun install`")
        }
        ProblemKind::NotFound { bare } => {
            let d = Diagnostic::error(&codes::ASSET_NOT_FOUND, message, span);
            if bare {
                d.help("install the package with `bun install`")
            } else {
                d
            }
        }
        ProblemKind::Css { stylesheet } => {
            let in_package = problem
                .file
                .as_deref()
                .is_some_and(|f| f.starts_with("node_modules/"));
            let d = Diagnostic::error(&codes::ASSET_CSS_IN_SCRIPT, message, span.clone());
            if in_package {
                return d.help("a package whose JS imports CSS can't be used yet");
            }
            let At::Specifier(specifier) = &problem.at else {
                unreachable!("an import has its specifier")
            };
            let import = if stylesheet.starts_with("node_modules/") {
                specifier.clone()
            } else {
                format!("/{stylesheet}")
            };
            let d = d.help(format!(
                "import it from a stylesheet that a Page links: `@import \"{import}\";`"
            ));
            match statement(sources.text(span.file), &span.range) {
                Some(range) => d.suggest(
                    "delete the import",
                    Applicability::MaybeIncorrect,
                    vec![Edit {
                        span: Span {
                            file: span.file,
                            range,
                        },
                        replacement: String::new(),
                    }],
                ),
                None => d,
            }
        }
        ProblemKind::Jsx => Diagnostic::error(&codes::ASSET_JSX, message, span)
            .help("yuanme doesn't compile JSX: build the element with the DOM, or in a Template"),
        ProblemKind::Invalid => Diagnostic::error(&codes::ASSET_INVALID_SCRIPT, message, span),
        ProblemKind::Warning => Diagnostic::warning(&codes::ASSET_SCRIPT_WARNING, message, span),
    }
}

/// Where `specifier` is first written in quotes in `text`, quotes excluded.
fn quoted(text: &str, specifier: &str) -> Option<std::ops::Range<usize>> {
    ['"', '\'', '`']
        .iter()
        .filter_map(|q| text.find(&format!("{q}{specifier}{q}")))
        .min()
        .map(|at| at + 1..at + 1 + specifier.len())
}

/// The line of the import statement that `specifier` is in, line break
/// included, when that line holds nothing else.
fn statement(text: &str, specifier: &std::ops::Range<usize>) -> Option<std::ops::Range<usize>> {
    let start = text[..specifier.start].rfind('\n').map_or(0, |i| i + 1);
    let end = text[specifier.end..]
        .find('\n')
        .map_or(text.len(), |i| specifier.end + i + 1);
    let quote = text.get(specifier.start.checked_sub(1)?..specifier.start)?;
    let quoted = format!("{quote}{}{quote}", &text[specifier.clone()]);
    let line = text[start..end].trim();
    let line = line.strip_suffix(';').unwrap_or(line);
    (line.starts_with("import") && line.ends_with(&quoted)).then_some(start..end)
}
