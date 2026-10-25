mod events;
mod load;
mod report;
mod schemas;
mod sync;
#[cfg(test)]
mod tests;

use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde_json::Value;
use yuanme_content::config::{self, Config};
use yuanme_content::site::Site;
use yuanme_content::slug;
use yuanme_diag::{Diagnostic, Files, Level};
use yuanme_engine::{Build, Cache, Mode};

use events::Event;

/// Every cache key starts with it (ADR 0011): the release version, plus the
/// git commit and a dirty flag when built from a checkout.
const ENGINE_VERSION: &str = env!("YUANME_ENGINE_VERSION");

// Exit codes (#21), which are public API (ADR 0015). 0 is success, with
// warnings allowed; a panic exits 101 too.
/// The Site has errors.
const SITE_ERRORS: u8 = 1;
/// The command can't be carried out as written: a bad flag, no project,
/// or a name that matches nothing.
const BAD_COMMAND: u8 = 2;
/// yuanme couldn't finish, such as when it can't write a file.
const INTERNAL: u8 = 101;

#[derive(Parser)]
#[command(
    name = "yuanme",
    version = ENGINE_VERSION,
    about = "A static site generator for Markdown, Org and math, built to be driven by coding agents."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
    /// Print NDJSON events on stdout, one per line. Human text, if any, goes to stderr.
    #[arg(long, global = true)]
    json: bool,
    /// Run as if yuanme was started in DIR.
    #[arg(short = 'C', value_name = "DIR", global = true)]
    dir: Option<PathBuf>,
    /// Colour human output. `auto` colours it when stdout is a terminal.
    #[arg(long, value_enum, default_value_t = Color::Auto, global = true)]
    color: Color,
    /// Print only errors.
    #[arg(long, short, global = true)]
    quiet: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum Color {
    Auto,
    Always,
    Never,
}

#[derive(Subcommand)]
enum Command {
    /// Build the Site into the output directory.
    Build {
        #[command(flatten)]
        check: CheckArgs,
        /// Empty the output directory first, files yuanme didn't write included.
        #[arg(long)]
        clean: bool,
        /// The output directory, from the project root.
        #[arg(long, value_name = "DIR", default_value = "dist")]
        out: PathBuf,
    },
    /// Report every Diagnostic `build` would, rendering every Page in memory and writing none.
    Check(CheckArgs),
    /// Print a JSON Schema.
    Schema {
        #[command(subcommand)]
        which: Which,
    },
}

#[derive(Args)]
struct CheckArgs {
    /// Include Drafts.
    #[arg(long)]
    drafts: bool,
    /// Make every warning an error.
    #[arg(long)]
    strict: bool,
}

#[derive(Subcommand)]
enum Which {
    /// Site config, yuanme.toml.
    Config,
    /// The front matter of a Collection's Entries.
    Entry { collection: String },
    /// A Data file that `[data.NAME]` declares.
    Data { name: String },
    /// What a kind of Route's Template sees: entry, list, term, index, template or feed.
    Context { kind: String },
    /// The events `--json` prints.
    Events,
}

enum Error {
    BadCommand(String),
    Io(io::Error),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::Io(e)
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let terminal = io::stdout().is_terminal();
    let out = Out {
        json: cli.json,
        terminal,
        color: match cli.color {
            Color::Always => true,
            Color::Never => false,
            Color::Auto => terminal,
        },
        quiet: cli.quiet,
    };
    ExitCode::from(match run(cli, &out) {
        Ok(code) => code,
        Err(Error::BadCommand(message)) => {
            eprintln!("error: {message}");
            BAD_COMMAND
        }
        Err(Error::Io(e)) => {
            eprintln!("error: {e}");
            INTERNAL
        }
    })
}

/// Where output goes (#21). It depends only on whether stdout is a
/// terminal, and on `--json`; nothing guesses whether an agent is calling.
struct Out {
    json: bool,
    terminal: bool,
    color: bool,
    quiet: bool,
}

impl Out {
    fn event(&self, event: Event<'_>) {
        if self.json {
            stdout(&event.to_json().to_string());
        }
    }

    /// Rendered with snippets on a terminal, else one line each.
    fn diagnostics(&self, diagnostics: &[Diagnostic], files: &Files) {
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
    fn say(&self, text: &str) {
        if !self.json && !self.quiet {
            eprintln!("{text}");
        }
    }
}

/// A closed pipe, as `yuanme routes | head` makes, ends the output, not
/// the command.
fn stdout(line: &str) {
    let _ = writeln!(io::stdout().lock(), "{line}");
}

fn run(cli: Cli, out: &Out) -> Result<u8, Error> {
    let cwd = std::env::current_dir()?;
    let dir = cli.dir.map_or(cwd.clone(), |d| cwd.join(d));
    match cli.command {
        Command::Schema {
            which: Which::Config,
        } => return Ok(print_schema(out, "config", config::json_schema())),
        Command::Schema {
            which: Which::Events,
        } => return Ok(print_schema(out, "events", events::schema())),
        _ => {}
    }
    if !dir.join("yuanme.toml").is_file() {
        return Err(Error::BadCommand(format!(
            "no yuanme.toml in {}",
            dir.display()
        )));
    }
    let root = dir;
    match cli.command {
        Command::Build {
            check,
            clean,
            out: out_dir,
        } => build(&root, &root.join(out_dir), clean, &check, out),
        Command::Check(check) => self::check(&root, &check, out),
        Command::Schema { which } => schema(&root, which, out),
    }
}

struct Run {
    build: Build,
    load: Duration,
    took: Duration,
}

/// One build in memory. `out` is the output directory, which the walk
/// leaves out.
fn engine_build(root: &Path, out: &Path, drafts: bool, strict: bool) -> io::Result<Run> {
    let started = Instant::now();
    let inputs = load::load(root, out)?;
    let load = started.elapsed();
    let mut cache = Cache::open(root.join(".yuanme/cache"), ENGINE_VERSION)?;
    let mode = if drafts { Mode::Drafts } else { Mode::Build };
    let mut build = yuanme_engine::build(&inputs, &mut cache, mode);
    cache.save()?;
    if strict {
        for d in &mut build.diagnostics {
            if d.level == Level::Warning {
                *d = d.clone().note("`--strict` makes every warning an error");
                d.level = Level::Error;
            }
        }
    }
    Ok(Run {
        build,
        load,
        took: started.elapsed() - load,
    })
}

fn counts(build: &Build) -> (usize, usize) {
    let count = |level| {
        build
            .diagnostics
            .iter()
            .filter(|d| d.level == level)
            .count()
    };
    (count(Level::Error), count(Level::Warning))
}

fn plural(n: usize, one: &str) -> String {
    match n {
        1 => format!("1 {one}"),
        _ => format!("{n} {one}s"),
    }
}

/// `build-finished`, and the exit code.
fn finish(out: &Out, build: &Build, started: Instant, changed_urls: Vec<String>) -> u8 {
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

fn build(
    root: &Path,
    out_dir: &Path,
    clean: bool,
    args: &CheckArgs,
    out: &Out,
) -> Result<u8, Error> {
    if root.starts_with(out_dir) {
        return Err(Error::BadCommand(format!(
            "--out {} would hold the project itself",
            out_dir.display()
        )));
    }
    let started = Instant::now();
    out.event(Event::BuildStarted { command: "build" });
    let Run { build, load, took } = engine_build(root, out_dir, args.drafts, args.strict)?;
    schemas::write(
        &root.join(".yuanme/schemas"),
        build.config.as_ref(),
        build.site.as_ref(),
    )?;
    out.diagnostics(&build.diagnostics, &build.sources);

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
    let mut changed: Vec<String> = written
        .map(|w| w.written.iter().map(|p| url_of(p)).collect())
        .unwrap_or_default();
    changed.sort();
    Ok(finish(out, &build, started, changed))
}

/// An output path's URL: `blog/pew/index.html` is `/blog/pew/`.
fn url_of(output: &str) -> String {
    let path = match output.strip_suffix("index.html") {
        Some(dir) if dir.is_empty() || dir.ends_with('/') => dir,
        _ => output,
    };
    slug::url_path(&format!("/{path}"))
}

fn check(root: &Path, args: &CheckArgs, out: &Out) -> Result<u8, Error> {
    let started = Instant::now();
    out.event(Event::BuildStarted { command: "check" });
    let Run { build, .. } = engine_build(root, &root.join("dist"), args.drafts, args.strict)?;
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

/// Site config, from a build so that its Diagnostics are the build's.
fn config_or_report<'b>(run: &'b Run, out: &Out) -> Option<&'b Config> {
    if run.build.config.is_none() {
        out.diagnostics(&run.build.diagnostics, &run.build.sources);
    }
    run.build.config.as_ref()
}

/// The Site, or its errors reported. Template and asset errors don't stop
/// it: they are `check`'s to report.
fn site_or_report<'b>(run: &'b Run, out: &Out) -> Option<(&'b Config, &'b Site)> {
    match (&run.build.config, &run.build.site) {
        (Some(config), Some(site)) => Some((config, site)),
        _ => {
            out.diagnostics(&run.build.diagnostics, &run.build.sources);
            None
        }
    }
}

fn print_schema(out: &Out, name: &str, schema: Value) -> u8 {
    if out.json {
        out.event(Event::Schema {
            name: name.to_string(),
            schema,
        });
    } else {
        stdout(&serde_json::to_string_pretty(&schema).expect("JSON serializes"));
    }
    0
}

/// `name` is not one of `choices`; `otherwise` says where to look when
/// none is close.
fn unknown(what: &str, name: &str, choices: &[&str], otherwise: String) -> Error {
    let near = config::similar(name, choices.iter().copied());
    Error::BadCommand(match near {
        Some(near) => format!("there is no {what} `{name}`; did you mean `{near}`?"),
        None => format!("there is no {what} `{name}`; {otherwise}"),
    })
}

fn schema(root: &Path, which: Which, out: &Out) -> Result<u8, Error> {
    let run = engine_build(root, &root.join("dist"), false, false)?;
    let (name, mut schemas) = match which {
        Which::Entry { collection } => {
            let Some(config) = config_or_report(&run, out) else {
                return Ok(SITE_ERRORS);
            };
            (
                format!("entry/{collection}"),
                schemas::entry_and_data(config),
            )
        }
        Which::Data { name } => {
            let Some(config) = config_or_report(&run, out) else {
                return Ok(SITE_ERRORS);
            };
            (format!("data/{name}"), schemas::entry_and_data(config))
        }
        Which::Context { kind } => {
            let Some((config, site)) = site_or_report(&run, out) else {
                return Ok(SITE_ERRORS);
            };
            (format!("context/{kind}"), schemas::context(config, site))
        }
        Which::Config | Which::Events => unreachable!("answered without a project"),
    };
    let Some(schema) = schemas.remove(&name) else {
        let (group, item) = name.split_once('/').expect("group/name");
        let what = match group {
            "entry" => "Collection",
            "data" => "Data file with a schema",
            _ => "kind of Route",
        };
        let choices: Vec<&str> = schemas
            .keys()
            .filter_map(|k| k.strip_prefix(&format!("{group}/")))
            .collect();
        let otherwise = match choices.is_empty() {
            true => "the Site has none".to_string(),
            false => format!("the Site has {}", choices.join(", ")),
        };
        return Err(unknown(what, item, &choices, otherwise));
    };
    Ok(print_schema(out, &name, schema))
}
