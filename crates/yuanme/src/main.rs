mod load;
mod sync;
#[cfg(test)]
mod tests;

use std::io;
use std::process::ExitCode;

use yuanme_engine::{Cache, Mode};

/// Every cache key starts with it (ADR 0011): the release version, plus the
/// git commit and a dirty flag when built from a checkout.
const ENGINE_VERSION: &str = env!("YUANME_ENGINE_VERSION");

fn main() -> ExitCode {
    run().unwrap_or_else(|e| {
        eprintln!("error: {e}");
        ExitCode::FAILURE
    })
}

/// Builds the project in the current directory into `dist/`. The CLI
/// replaces this with `yuanme build` and its flags (#65).
fn run() -> io::Result<ExitCode> {
    let root = std::env::current_dir()?;
    if !root.join("yuanme.toml").is_file() {
        eprintln!("error: no yuanme.toml in {}", root.display());
        return Ok(ExitCode::FAILURE);
    }
    let out = root.join("dist");
    let cache_dir = root.join(".yuanme/cache");

    write_schema(&root.join(".yuanme/schemas/yuanme.toml.json"))?;
    let inputs = load::load(&root, &out)?;
    let mut cache = Cache::open(cache_dir.clone(), ENGINE_VERSION)?;
    let build = yuanme_engine::build(&inputs, &mut cache, Mode::Build);
    cache.save()?;
    for diagnostic in &build.diagnostics {
        eprintln!("{}", diagnostic.render(&build.sources, false));
    }
    if build.failed() {
        return Ok(ExitCode::FAILURE);
    }

    let report = sync::sync(&build, &root, &out, &cache_dir.join("output.manifest"))?;
    if !report.unknown.is_empty() {
        eprintln!("warning: dist/ holds files yuanme didn't write, left as they are:");
        for path in &report.unknown {
            eprintln!("  {path}");
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Editors and agents read Site config's schema from here (#21). It is
/// rewritten only when it changed, so editors watching it aren't woken.
fn write_schema(path: &std::path::Path) -> io::Result<()> {
    let schema = serde_json::to_string_pretty(&yuanme_content::config::json_schema())? + "\n";
    if std::fs::read_to_string(path).is_ok_and(|old| old == schema) {
        return Ok(());
    }
    std::fs::create_dir_all(path.parent().expect("in .yuanme/schemas"))?;
    std::fs::write(path, schema)
}
