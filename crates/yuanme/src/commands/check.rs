//! `yuanme check`: every Diagnostic `build` would report, writing no
//! Site.

use std::path::Path;
use std::time::Instant;

use yuanme_engine::Mode;

use crate::cli::CheckArgs;
use crate::events::Event;
use crate::out::{Out, counts, finish, plural};
use crate::project::{Run, engine_build};
use crate::{Error, schemas};

pub(crate) fn check(root: &Path, args: &CheckArgs, out: &Out) -> Result<u8, Error> {
    let started = Instant::now();
    out.event(Event::BuildStarted { command: "check" });
    let mode = Mode::Check {
        drafts: args.drafts,
    };
    let Run { build, .. } = engine_build(root, None, mode, args.strict, false)?;
    schemas::write(
        &root.join(".yuanme/schemas"),
        build.config.as_ref(),
        build.site.as_ref(),
    )?;
    out.diagnostics(&build.diagnostics, &build.sources);
    let (errors, warnings) = counts(&build);
    let ms = started.elapsed().as_millis();
    match build.failed() {
        true => out.say(&format!(
            "check found {} and {} in {ms} ms",
            plural(errors, "error"),
            plural(warnings, "warning")
        )),
        false => out.say(&format!(
            "checked {} in {ms} ms: no errors, {}",
            plural(build.site.as_ref().map_or(0, |s| s.routes.len()), "Page"),
            plural(warnings, "warning")
        )),
    }
    Ok(finish(out, &build, started, Vec::new()))
}
