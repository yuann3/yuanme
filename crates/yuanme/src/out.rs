//! What yuanme prints: NDJSON events with `--json`, and otherwise text
//! for a person on stderr.

use std::io::{self, Write};
use std::time::Instant;

use yuanme_diag::{Diagnostic, Files, Level};
use yuanme_engine::Build;

use crate::SITE_ERRORS;
use crate::events::Event;

/// Where output goes (#21). It depends only on whether stdout is a
/// terminal, and on `--json`; nothing guesses whether an agent is calling.
#[derive(Clone, Copy)]
pub(crate) struct Out {
    pub(crate) json: bool,
    pub(crate) terminal: bool,
    pub(crate) color: bool,
    pub(crate) quiet: bool,
}

impl Out {
    pub(crate) fn event(&self, event: Event<'_>) {
        if self.json {
            stdout(&event.to_json().to_string());
        }
    }

    /// Rendered with snippets on a terminal, else one line each.
    pub(crate) fn diagnostics(&self, diagnostics: &[Diagnostic], files: &Files) {
        for d in diagnostics {
            if self.json {
                self.event(Event::Diagnostic(d, files));
            } else if self.quiet && d.level == Level::Warning {
            } else if self.terminal {
                eprintln!("{}\n", d.render(files, self.color));
            } else {
                eprintln!("{}", d.render_short(files));
            }
        }
    }

    /// Human text, on stderr so stdout stays parseable.
    pub(crate) fn say(&self, text: &str) {
        if !self.json && !self.quiet {
            eprintln!("{text}");
        }
    }
}

/// A closed pipe, as `yuanme routes | head` makes, ends the output, not
/// the command.
pub(crate) fn stdout(line: &str) {
    let _ = writeln!(io::stdout().lock(), "{line}");
}

pub(crate) fn counts(build: &Build) -> (usize, usize) {
    let count = |level| {
        build
            .diagnostics
            .iter()
            .filter(|d| d.level == level)
            .count()
    };
    (count(Level::Error), count(Level::Warning))
}

pub(crate) fn plural(n: usize, one: &str) -> String {
    match n {
        1 => format!("1 {one}"),
        _ => format!("{n} {one}s"),
    }
}

/// `build-finished`, and the exit code.
pub(crate) fn finish(out: &Out, build: &Build, started: Instant, changed_urls: Vec<String>) -> u8 {
    let (errors, warnings) = counts(build);
    let pages = match (&build.site, build.failed()) {
        (Some(site), false) => site.routes.len(),
        _ => 0,
    };
    out.event(Event::BuildFinished {
        ok: !build.failed(),
        duration_ms: started.elapsed().as_millis(),
        pages,
        changed_urls,
        errors,
        warnings,
    });
    if build.failed() { SITE_ERRORS } else { 0 }
}
