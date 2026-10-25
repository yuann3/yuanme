//! Type checking (#20): with `[typescript] check = true`, TypeScript 7's
//! `tsc`, a Managed tool, checks every TypeScript Script a build bundled,
//! and each error it finds is an `asset/type-error`. Its output is cached
//! under every file it may read, so an unchanged project runs no `tsc`.

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::json;
use yuanme_assets::tool;
use yuanme_assets::typescript::{self, TypeError};
use yuanme_content::config::Config;
use yuanme_diag::{Diagnostic, FileId, Files, Span, codes};

use crate::{Bundle, Cache, Inputs};

/// Where the types are, from the project root, which a tsconfig.json
/// includes.
const TYPES: &str = ".yuanme/types/yuanme.d.ts";

/// Writes the types of what yuanme gives Scripts to
/// `.yuanme/types/yuanme.d.ts` under `root`, unless they are there.
pub fn write_types(root: &Path) -> io::Result<()> {
    let path = root.join(TYPES);
    if std::fs::read(&path).is_ok_and(|b| b == typescript::TYPES.as_bytes()) {
        return Ok(());
    }
    std::fs::create_dir_all(path.parent().expect("in .yuanme/types"))?;
    // Written whole and renamed, so an editor or a `tsc` running at the
    // same time never reads half of it.
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&tmp, typescript::TYPES)?;
    std::fs::rename(&tmp, &path)
}

/// The type errors in the TypeScript Scripts of `scripts`, a build's
/// bundles, adding the files they are in to `sources`; none without
/// `[typescript] check = true`.
///
/// `build` checks in every mode but `Mode::Dev`, since `tsc` takes about
/// 100 ms. The dev server calls this itself after a build, with that
/// build's `scripts` and `sources`, on a thread of its own and with a
/// Cache of its own, and sends what it returns to the overlay (#67).
pub fn check_types(
    inputs: &Inputs,
    config: &Config,
    scripts: &[Bundle],
    cache: &mut Cache,
    sources: &mut Files,
) -> Vec<Diagnostic> {
    let Some(at) = &config.typescript.check else {
        return Vec::new();
    };
    let at = Span {
        file: config.file,
        range: at.clone(),
    };
    let is_ts = |p: &str| [".ts", ".mts", ".cts"].iter().any(|e| p.ends_with(e));
    let is_declarations = |p: &str| [".d.ts", ".d.mts", ".d.cts"].iter().any(|e| p.ends_with(e));
    // `tsc` follows their imports. A project's own `.d.ts` is imported by
    // nothing, and declares globals the Scripts use.
    let mut roots: BTreeSet<&str> = scripts
        .iter()
        .flat_map(|b| &b.modules)
        .map(|(path, _)| path.as_str())
        .filter(|p| is_ts(p) && !p.starts_with("node_modules/") && inputs.files.contains_key(*p))
        .collect();
    if roots.is_empty() {
        return Vec::new();
    }
    roots.extend(
        inputs
            .files
            .keys()
            .map(String::as_str)
            .filter(|p| is_declarations(p) && !p.starts_with("static/")),
    );
    let Some(root) = inputs.root.as_deref() else {
        let message = "yuanme checks types only for a project in a folder";
        return vec![Diagnostic::error(&codes::ASSET_TYPE_ERROR, message, at)];
    };
    // `tsc` names files from the real path of the folder it runs in, so
    // through a symlinked root, such as macOS's `/var`, it names them as
    // `../../var/…`. On Windows a real path is a `\\?\` one, so the root
    // stays as it is given there.
    let real = (!cfg!(windows))
        .then(|| std::fs::canonicalize(root).ok())
        .flatten();
    let root = real.as_deref().unwrap_or(root);

    let tsconfig = tsconfig(inputs, &roots);
    let mut key = cache.key("tsc");
    for part in [typescript::VERSION, &tsconfig, typescript::TYPES] {
        key.update(&(part.len() as u64).to_le_bytes());
        key.update(part.as_bytes());
    }
    // Everything of the project `tsc` may read, but `node_modules`, which
    // `package.json` stands for.
    for (path, input) in inputs.iter() {
        let read = [".ts", ".mts", ".cts", ".js", ".mjs", ".cjs", ".json"]
            .iter()
            .any(|e| path.ends_with(e));
        if read {
            key.update(path.as_bytes());
            key.update(&[0]);
            key.update(input.hash.as_bytes());
        }
    }
    let key = key.finalize();
    let output = match cache.get("tsc", &key) {
        Some(cached) => String::from_utf8_lossy(&cached).into_owned(),
        None => match run(root, &tsconfig) {
            Ok(output) => {
                cache.insert("tsc", key, output.as_bytes());
                output
            }
            Err((message, help)) => {
                let d = Diagnostic::error(&codes::ASSET_TOOL_UNAVAILABLE, message, at);
                return vec![d.help(help)];
            }
        },
    };
    let mut files = BTreeMap::new();
    typescript::errors(&output)
        .into_iter()
        .map(|e| diagnostic(e, inputs, root, &at, sources, &mut files))
        .collect()
}

/// The tsconfig the check runs on, in `.yuanme/`: the Site's own
/// `tsconfig.json`, when it has one, with only `roots` and the types as
/// its files. Without one, the options `yuanme docs scripts` shows.
fn tsconfig(inputs: &Inputs, roots: &BTreeSet<&str>) -> String {
    let mut files: Vec<String> = roots.iter().map(|p| format!("../{p}")).collect();
    files.push("types/yuanme.d.ts".into());
    let mut tsconfig = if inputs.files.contains_key("tsconfig.json") {
        json!({ "extends": "../tsconfig.json", "compilerOptions": { "noEmit": true } })
    } else {
        // TypeScript 7's defaults are strict, the newest target, and the
        // DOM. These are how the bundler reads a Script.
        json!({
            "compilerOptions": {
                "noEmit": true,
                "allowJs": true,
                "allowImportingTsExtensions": true,
                "resolveJsonModule": true,
                "skipLibCheck": true,
                "paths": { "/*": ["../*"] },
            }
        })
    };
    tsconfig["include"] = json!([]);
    tsconfig["files"] = json!(files);
    serde_json::to_string_pretty(&tsconfig).expect("JSON")
}

/// One run of `tsc` on `tsconfig`, written to a file of its own in
/// `.yuanme/` and removed, or a message and help saying why it can't run.
fn run(root: &Path, tsconfig: &str) -> Result<String, (String, String)> {
    let binary = binary()?;
    static RUNS: AtomicUsize = AtomicUsize::new(0);
    let path = root.join(format!(
        ".yuanme/tsconfig-{}-{}.json",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::Relaxed)
    ));
    let can_t_write = |e: io::Error| {
        let message = format!("can't write {}: {e}", path.display());
        (message, "make `.yuanme` writable".to_string())
    };
    write_types(root).map_err(can_t_write)?;
    std::fs::write(&path, tsconfig).map_err(can_t_write)?;
    let result = typescript::run(&binary, root, &path);
    if let Err(e) = std::fs::remove_file(&path) {
        // Left behind, it is a few bytes in `.yuanme`.
        eprintln!("warning: could not delete {}: {e}", path.display());
    }
    result.map_err(|message| {
        let help = format!("set `{}` to a `tsc` that runs here", typescript::ENV);
        (message, help)
    })
}

/// The pinned `tsc`, or a message and help saying why there is none.
fn binary() -> Result<PathBuf, (String, String)> {
    let override_help = format!(
        "set `{}` to the `tsc` of a TypeScript {} release",
        typescript::ENV,
        typescript::VERSION
    );
    let Some(tool) = typescript::tool() else {
        if let Some(path) = std::env::var_os(typescript::ENV) {
            return Ok(PathBuf::from(path));
        }
        let platform = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let message = format!("TypeScript publishes no `tsc` for {platform}");
        return Err((message, override_help));
    };
    let Some(cache) = tool::cache_dir() else {
        let message = "there is no folder to keep `tsc` in: `HOME` isn't set".into();
        return Err((message, override_help));
    };
    tool.binary(&cache).map_err(|e| (e.message, e.help))
}

/// `e` at the place `tsc` puts it, in a file of the project or of
/// `node_modules`; one about no file, or about one yuanme can't read, is
/// at `[typescript] check`.
fn diagnostic(
    e: TypeError,
    inputs: &Inputs,
    root: &Path,
    check: &Span,
    sources: &mut Files,
    files: &mut BTreeMap<String, Option<FileId>>,
) -> Diagnostic {
    let mut message = format!("{}: {}", e.code, e.message);
    let span = e.at.and_then(|(path, line, column)| {
        let file = *files.entry(path.clone()).or_insert_with(|| {
            let text = match inputs.files.get(&path).and_then(|i| i.bytes.as_deref()) {
                Some(bytes) => String::from_utf8_lossy(bytes).into_owned(),
                None => std::fs::read_to_string(root.join(&path)).ok()?,
            };
            Some(sources.add(path.as_str(), text))
        });
        let Some(file) = file else {
            message = format!("{path}({line},{column}): {message}");
            return None;
        };
        let from = offset(sources.text(file), line, column);
        let range = from..token_end(sources.text(file), from);
        Some(Span { file, range })
    });
    let mut d = Diagnostic::error(
        &codes::ASSET_TYPE_ERROR,
        message,
        span.unwrap_or(check.clone()),
    );
    for detail in e.details {
        d = d.note(detail);
    }
    d
}

/// The byte offset of `line` from 1 and `column` from 1 in UTF-16 units.
fn offset(text: &str, line: u32, column: u32) -> usize {
    let start: usize = text
        .split_inclusive('\n')
        .take(line.saturating_sub(1) as usize)
        .map(str::len)
        .sum();
    let line_text = text[start..].split('\n').next().unwrap_or("");
    let mut units = 1;
    for (i, c) in line_text.char_indices() {
        if units >= column as usize {
            return start + i;
        }
        units += c.len_utf16();
    }
    start + line_text.len()
}

/// Where the token at `at` ends. `tsc --pretty false` says only where an
/// error starts, and it puts most on a name or a literal; its pretty
/// output underlines the whole node, but in a layout of its own that it
/// elides past a few lines.
fn token_end(text: &str, at: usize) -> usize {
    let rest = &text[at..];
    let Some(first) = rest.chars().next() else {
        return at;
    };
    let word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    let len = if word(first) {
        rest.find(|c: char| !word(c)).unwrap_or(rest.len())
    } else if matches!(first, '"' | '\'' | '`') {
        let mut escaped = false;
        let mut end = rest.len();
        for (i, c) in rest.char_indices().skip(1) {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '\n' if first != '`' => {
                    end = i;
                    break;
                }
                _ if c == first => {
                    end = i + 1;
                    break;
                }
                _ => {}
            }
        }
        end
    } else {
        first.len_utf8()
    };
    at + len
}
