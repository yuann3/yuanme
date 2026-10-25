//! The build report (#21), `.yuanme/report.md` and `.json`: every Page's
//! size, how long each stage took, and what was written. #66 adds the
//! Budgets and Acknowledged overages.

use std::fs;
use std::io;
use std::path::Path;
use std::time::Duration;

use serde_json::json;
use yuanme_diag::Level;
use yuanme_engine::{Body, Build};

use crate::sync;

pub struct Stages {
    pub load: Duration,
    pub build: Duration,
    pub write: Duration,
}

/// Writes both files into `dir`. `written` is `None` for a build that
/// failed, which writes nothing.
pub fn write(
    dir: &Path,
    root: &Path,
    build: &Build,
    stages: &Stages,
    written: Option<&sync::Report>,
) -> io::Result<()> {
    let size = |path: &str| -> io::Result<u64> {
        Ok(match &build.files[path].body {
            Body::Bytes(bytes) => bytes.len() as u64,
            Body::Input(input) => fs::metadata(root.join(input))?.len(),
            Body::Cached(file) => fs::metadata(file)?.len(),
        })
    };
    let mut pages = Vec::new();
    if let Some(site) = build.site.as_ref().filter(|_| !build.failed()) {
        for route in &site.routes {
            pages.push((route.url.as_str(), size(&route.output)?));
        }
    }
    // Largest first: what a reader of the report looks for.
    pages.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let mut bytes = 0;
    for path in build.files.keys() {
        bytes += size(path)?;
    }
    let count = |level| {
        build
            .diagnostics
            .iter()
            .filter(|d| d.level == level)
            .count()
    };
    let ms = |d: Duration| d.as_millis();
    let stages_json = json!({
        "load_ms": ms(stages.load),
        "build_ms": ms(stages.build),
        "write_ms": ms(stages.write),
    });
    let mut report = json!({
        "ok": !build.failed(),
        "errors": count(Level::Error),
        "warnings": count(Level::Warning),
        "stages": stages_json,
        "pages": pages.iter().map(|(url, bytes)| json!({ "url": url, "bytes": bytes })).collect::<Vec<_>>(),
        "files": build.files.len(),
        "bytes": bytes,
        "written": written.map(|w| w.written.len()),
        "deleted": written.map(|w| w.deleted.len()),
        "unknown": written.map(|w| w.unknown.clone()),
    });
    report.sort_all_objects();

    let mut md = String::from("# Build report\n\n");
    if build.failed() {
        md.push_str(&format!(
            "The build failed with {} errors and {} warnings, and wrote nothing.\n",
            count(Level::Error),
            count(Level::Warning)
        ));
    } else {
        md.push_str(&format!(
            "{} Pages and {} files, {} in all. {} errors, {} warnings.\n",
            pages.len(),
            build.files.len(),
            human(bytes),
            count(Level::Error),
            count(Level::Warning)
        ));
    }
    md.push_str("\n| Stage | Time |\n|---|---:|\n");
    for (stage, d) in [
        ("load", stages.load),
        ("build", stages.build),
        ("write", stages.write),
    ] {
        md.push_str(&format!("| {stage} | {} ms |\n", ms(d)));
    }
    if let Some(w) = written {
        md.push_str(&format!(
            "\n{} files written, {} deleted.\n",
            w.written.len(),
            w.deleted.len()
        ));
        if !w.unknown.is_empty() {
            md.push_str(
                "\nFiles in the output directory that yuanme didn't write, left as they are:\n\n",
            );
            for path in &w.unknown {
                md.push_str(&format!("- `{path}`\n"));
            }
        }
    }
    if !pages.is_empty() {
        md.push_str("\n## Pages\n\n| Page | Size |\n|---|---:|\n");
        for (url, bytes) in &pages {
            md.push_str(&format!("| `{url}` | {} |\n", human(*bytes)));
        }
    }

    fs::create_dir_all(dir)?;
    fs::write(dir.join("report.md"), md)?;
    fs::write(
        dir.join("report.json"),
        serde_json::to_string_pretty(&report)? + "\n",
    )
}

fn human(bytes: u64) -> String {
    match bytes {
        b if b < 1_000 => format!("{b} B"),
        b if b < 1_000_000 => format!("{:.1} kB", b as f64 / 1e3),
        b => format!("{:.1} MB", b as f64 / 1e6),
    }
}
