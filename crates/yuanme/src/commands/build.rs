//! `yuanme build`: the Site into its output directory, and the build
//! report.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Instant;

use yuanme_engine::Mode;

use crate::cli::CheckArgs;
use crate::events::Event;
use crate::out::{Out, counts, finish, plural};
use crate::project::{self, Run, sync, url_of};
use crate::{Error, report, schemas};

pub(crate) fn run(
    root: &Path,
    out_dir: Option<PathBuf>,
    clean: bool,
    args: &CheckArgs,
    strict_budgets: bool,
    out: &Out,
) -> Result<u8, Error> {
    if let Some(dir) = out_dir.as_ref().filter(|d| root.starts_with(d)) {
        return Err(Error::BadCommand(format!(
            "--out {} would hold the project itself",
            dir.display()
        )));
    }
    let started = Instant::now();
    out.event(Event::BuildStarted { command: "build" });
    let mode = if args.drafts {
        Mode::Drafts
    } else {
        Mode::Build
    };
    let Run {
        build,
        out: out_dir,
        load,
        took,
    } = project::build(root, out_dir.as_deref(), mode, args.strict, strict_budgets)?;
    let out_dir = out_dir.as_path();
    schemas::write(
        &root.join(".yuanme/schemas"),
        build.config.as_ref(),
        build.site.as_ref(),
    )?;
    out.diagnostics(&build.diagnostics, &build.sources);
    if let Some(budgets) = build.config.as_ref().and_then(|c| c.budgets.as_ref()) {
        for over in report::over(&build.budgets, budgets) {
            out.event(Event::Budget(over));
        }
    }

    let writing = Instant::now();
    let written = if build.failed() {
        None
    } else {
        let manifest = root.join(".yuanme/cache/output.manifest");
        if clean {
            for result in [fs::remove_dir_all(out_dir), fs::remove_file(&manifest)] {
                match result {
                    Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e.into()),
                    _ => {}
                }
            }
        }
        Some(sync::sync(&build, root, out_dir, &manifest)?)
    };
    let stages = report::Stages {
        load,
        build: took,
        write: writing.elapsed(),
    };
    report::write(
        &root.join(".yuanme"),
        root,
        &build,
        &stages,
        written.as_ref(),
    )?;

    let (errors, warnings) = counts(&build);
    let shown = out_dir.strip_prefix(root).unwrap_or(out_dir).display();
    match &written {
        None => out.say(&format!(
            "build failed with {} and {}; {shown}/ is unchanged",
            plural(errors, "error"),
            plural(warnings, "warning")
        )),
        Some(w) => {
            if !w.unknown.is_empty() {
                out.say(&format!(
                    "warning: {shown}/ holds files yuanme didn't write, left as they are: {}",
                    w.unknown.join(", ")
                ));
            }
            let pages = build.site.as_ref().map_or(0, |s| s.routes.len());
            let summary = [
                format!(
                    "built {} into {shown}/ in {} ms",
                    plural(pages, "Page"),
                    started.elapsed().as_millis()
                ),
                format!(
                    "{} written, {} deleted, {}",
                    plural(w.written.len(), "file"),
                    w.deleted.len(),
                    plural(warnings, "warning")
                ),
                "report: .yuanme/report.md".to_string(),
            ];
            match out.terminal {
                true => out.say(&summary.join("\n")),
                false => out.say(&summary.join("; ")),
            }
        }
    }
    // The Site's URLs, not the files only the host reads.
    let host = build.config.as_ref().map(|c| &c.host);
    let mut changed: Vec<String> = written
        .map(|w| {
            w.written
                .iter()
                .filter_map(|p| host.map_or(Some(p.as_str()), |h| yuanme_engine::served(h, p)))
                .map(url_of)
                .collect()
        })
        .unwrap_or_default();
    changed.sort();
    Ok(finish(out, &build, started, changed))
}
