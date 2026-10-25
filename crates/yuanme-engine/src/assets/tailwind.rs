//! Tailwind (#20): each stylesheet a Page links that imports Tailwind is
//! built by the Tailwind CLI, a Managed tool, from the Site's class
//! tokens, before lightningcss bundles it with Tailwind's options.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use yuanme_assets::tailwind::{self, Error};
use yuanme_assets::tool;
use yuanme_content::config::Config;
use yuanme_diag::{Diagnostic, Files, Span, codes};

use super::{Diagnostics, Project, SourceFiles};
use crate::Cache;

/// The CLI's unminified output for each stylesheet in `entries`, by its
/// path from the project root. Each is cached under its tokens, the CSS
/// and plugins Tailwind may read, and the release, so a warm build runs no
/// Tailwind. A stylesheet that fails has a Diagnostic instead.
#[allow(clippy::too_many_arguments)] // Each is one thing the stage holds.
pub(super) fn outputs(
    project: &Project<'_>,
    config: &Config,
    cache: &mut Cache,
    entries: &[String],
    tokens: &str,
    sources: &mut Files,
    files: &mut SourceFiles,
    diagnostics: &mut Diagnostics,
) -> BTreeMap<String, String> {
    let version = config
        .tailwind
        .version
        .as_ref()
        .map_or(tailwind::VERSION, |(v, _)| v.as_str());
    // Everything of the project Tailwind may read but the token file: its
    // stylesheets, JS plugins and installed packages. A Template's Sidecars
    // are tokens already. An `@source` of the Author's has it scan files
    // too, which could be any of them.
    let scans = project.inputs.iter().any(|(path, input)| {
        path.ends_with(".css")
            && input
                .bytes
                .as_deref()
                .is_some_and(|b| b.windows(7).any(|w| w == b"@source"))
    });
    let mut inputs = blake3::Hasher::new();
    for (path, input) in project.inputs.iter() {
        let read = [".css", ".js", ".cjs", ".mjs", ".ts", ".mts", ".cts"]
            .iter()
            .any(|e| path.ends_with(e))
            && !path.starts_with("templates/");
        if scans || read || ["package.json", "bun.lock", "bun.lockb"].contains(&path) {
            inputs.update(path.as_bytes());
            inputs.update(&[0]);
            inputs.update(input.hash.as_bytes());
        }
    }
    let inputs = inputs.finalize();
    let mut binary: Option<Result<PathBuf, (String, String)>> = None;
    let mut out = BTreeMap::new();
    for entry in entries {
        let css = project.text(entry).expect("read to find its import");
        let mut key = cache.key("tailwind");
        for part in [version, entry, tokens] {
            key.update(&(part.len() as u64).to_le_bytes());
            key.update(part.as_bytes());
        }
        key.update(inputs.as_bytes());
        let key = key.finalize();
        if let Some(cached) = cache.get("tailwind", &key) {
            out.insert(entry.clone(), String::from_utf8_lossy(&cached).into_owned());
            continue;
        }
        let at = |needle: Option<&str>, sources: &mut Files, files: &mut SourceFiles| {
            let file = files.file(project, sources, entry);
            let text = sources.text(file);
            let range = needle
                .and_then(|n| text.find(n).map(|i| i..i + n.len()))
                .or_else(|| import_at(text))
                .unwrap_or(0..0);
            Span { file, range }
        };
        let Some(root) = project.inputs.root.as_deref() else {
            let message = "yuanme runs Tailwind only for a project in a folder".to_string();
            diagnostics.push(Diagnostic::error(
                &codes::ASSET_TAILWIND_ERROR,
                message,
                at(None, sources, files),
            ));
            continue;
        };
        let binary = binary.get_or_insert_with(|| self::binary(version));
        let binary = match binary {
            Ok(binary) => binary.clone(),
            Err((message, help)) => {
                let span = match &config.tailwind.version {
                    Some((_, range)) => Span {
                        file: config.file,
                        range: range.clone(),
                    },
                    None => at(None, sources, files),
                };
                let d = Diagnostic::error(&codes::ASSET_TOOL_UNAVAILABLE, message.clone(), span);
                diagnostics.push(d.help(help.clone()));
                continue;
            }
        };
        let path = root.join(entry);
        let dir = path.parent().expect("a file is in a folder");
        match run(&binary, dir, &css, tokens) {
            Ok(output) => {
                cache.insert("tailwind", key, output.as_bytes());
                out.insert(entry.clone(), output);
            }
            Err(Error::Tailwind(message)) => {
                // Tailwind quotes what it is about in backticks, such as
                // the utility an `@apply` names.
                let quoted = message.split('`').nth(1).filter(|q| !q.is_empty());
                let span = at(quoted, sources, files);
                diagnostics.push(Diagnostic::error(
                    &codes::ASSET_TAILWIND_ERROR,
                    message,
                    span,
                ));
            }
            Err(Error::Run(message)) => {
                let span = at(None, sources, files);
                let d = Diagnostic::error(&codes::ASSET_TOOL_UNAVAILABLE, message, span);
                diagnostics.push(d.help(format!(
                    "set `{}` to a Tailwind CLI that runs here",
                    tailwind::ENV
                )));
            }
        }
    }
    out
}

/// The CLI for `version`, or a message and help saying why there is none.
fn binary(version: &str) -> Result<PathBuf, (String, String)> {
    let override_help = format!("set `{}` to a Tailwind CLI binary", tailwind::ENV);
    let Some(tool) = tailwind::tool(version) else {
        if let Some(path) = std::env::var_os(tailwind::ENV) {
            return Ok(PathBuf::from(path));
        }
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let message = format!("Tailwind publishes no CLI for {platform}");
        return Err((message, override_help));
    };
    let Some(cache) = tool::cache_dir() else {
        let message = "there is no folder to keep the Tailwind CLI in: `HOME` isn't set".into();
        return Err((message, override_help));
    };
    tool.binary(&cache).map_err(|e| (e.message, e.help))
}

/// One run of the CLI, from `dir`, on the stylesheet `css` with the token
/// file holding `tokens`, written to a folder of its own and removed.
fn run(binary: &Path, dir: &Path, css: &str, tokens: &str) -> Result<String, Error> {
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let scratch = std::env::temp_dir().join(format!(
        "yuanme-tailwind-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    ));
    let file = scratch.join("tokens.txt");
    std::fs::create_dir_all(&scratch)
        .and_then(|()| std::fs::write(&file, tokens))
        .map_err(|e| Error::Run(format!("can't write {}: {e}", file.display())))?;
    let input = tailwind::input(css, &file).expect("it imports Tailwind");
    let result = tailwind::run(binary, dir, &input, &scratch);
    if let Err(e) = std::fs::remove_dir_all(&scratch) {
        // Left behind, it costs a few bytes in the temporary folder.
        eprintln!("warning: could not delete {}: {e}", scratch.display());
    }
    result
}

/// Where `css` imports Tailwind.
fn import_at(css: &str) -> Option<std::ops::Range<usize>> {
    ["\"tailwindcss", "'tailwindcss"]
        .iter()
        .filter_map(|n| css.find(n))
        .min()
        .map(|i| {
            let end = css[i + 1..]
                .find(['"', '\''])
                .map_or(css.len(), |e| i + 1 + e + 1);
            i..end
        })
}
