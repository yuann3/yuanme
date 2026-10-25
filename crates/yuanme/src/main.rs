mod docs;
mod events;
mod load;
mod new;
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
use yuanme_content::site::{RouteKind, Site};
use yuanme_content::slug;
use yuanme_diag::{Diagnostic, Files, Level};
use yuanme_engine::{Build, Cache, Mode};

use events::Event;

#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

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
        /// The output directory, from the project root. By default `dist`, or
        /// `.vercel/output` with the vercel adapter.
        #[arg(long, value_name = "DIR")]
        out: Option<PathBuf>,
    },
    /// Report every Diagnostic `build` would, rendering every Page in memory and writing none.
    Check(CheckArgs),
    /// Create an Entry whose required fields are filled in, and print its path and Route.
    New {
        /// The Collection, by its name in yuanme.toml.
        collection: String,
        /// The Entry's title, from which its Slug and file name come.
        title: String,
        /// The Source format.
        #[arg(long, value_enum, default_value_t = new::Format::Md)]
        format: new::Format,
    },
    /// List every Route and what produces it.
    Routes {
        /// Include Drafts.
        #[arg(long)]
        drafts: bool,
    },
    /// Print a JSON Schema.
    Schema {
        #[command(subcommand)]
        which: Which,
    },
    /// Print a docs page by topic or Diagnostic code; with none, the index.
    Docs {
        /// A topic, such as `templates`, or a code, such as `entry/unknown-key`.
        topic: Option<String>,
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
        Command::Docs { topic } => return docs(topic.as_deref().unwrap_or("index"), out),
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
    // The docs always match the binary that runs in the project (#21).
    docs::write(&root.join(".yuanme/docs"), ENGINE_VERSION)?;
    match cli.command {
        Command::Build {
            check,
            clean,
            out: out_dir,
        } => build(&root, out_dir.map(|d| root.join(d)), clean, &check, out),
        Command::Check(check) => self::check(&root, &check, out),
        Command::New {
            collection,
            title,
            format,
        } => new(&root, &collection, &title, format, out),
        Command::Routes { drafts } => routes(&root, drafts, out),
        Command::Schema { which } => schema(&root, which, out),
        Command::Docs { .. } => unreachable!("answered without a project"),
    }
}

struct Run {
    build: Build,
    /// The output directory: `--out`, or else the adapter's.
    out: PathBuf,
    load: Duration,
    took: Duration,
}

/// One build in memory. `out` is `--out`, which the walk leaves out; the
/// adapter's output directory is `dist` or hidden. Only `yuanme build`
/// encodes images: every other command builds as `check` does.
fn engine_build(root: &Path, out: Option<&Path>, mode: Mode, strict: bool) -> io::Result<Run> {
    let started = Instant::now();
    let dist = root.join("dist");
    let inputs = load::load(root, out.unwrap_or(&dist))?;
    let load = started.elapsed();
    let mut cache = Cache::open(root.join(".yuanme/cache"), ENGINE_VERSION)?;
    let mut build = yuanme_engine::build(&inputs, &mut cache, mode);
    cache.save()?;
    let out = match (out, &build.config) {
        (Some(out), _) => out.to_path_buf(),
        (None, Some(config)) => root.join(config.host.adapter.out_dir()),
        (None, None) => dist,
    };
    // The Worker's configuration, which isn't an input.
    if let Ok(text) = fs::read_to_string(root.join("wrangler.jsonc")) {
        let dir = out.strip_prefix(root).unwrap_or(&out);
        let dir = dir.to_string_lossy().replace('\\', "/");
        yuanme_engine::wrangler(&mut build, &text, &dir);
    }
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
        out,
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
    out_dir: Option<PathBuf>,
    clean: bool,
    args: &CheckArgs,
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
    } = engine_build(root, out_dir.as_deref(), mode, args.strict)?;
    let out_dir = out_dir.as_path();
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
    let mode = Mode::Check {
        drafts: args.drafts,
    };
    let Run { build, .. } = engine_build(root, None, mode, args.strict)?;
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

fn new(
    root: &Path,
    collection: &str,
    title: &str,
    format: new::Format,
    out: &Out,
) -> Result<u8, Error> {
    let run = engine_build(root, None, Mode::Check { drafts: false }, false)?;
    let Some(config) = config_or_report(&run, out) else {
        return Ok(SITE_ERRORS);
    };
    let found = new::collection(config, collection).map_err(Error::BadCommand)?;
    let zone =
        jiff::tz::TimeZone::get(&config.site.timezone).expect("Site config checked the zone");
    let today = jiff::Zoned::now().with_time_zone(zone).date();
    let created = new::entry(&found, title, format, today).map_err(Error::BadCommand)?;
    let path = root.join(&created.path);
    if path.exists() {
        return Err(Error::BadCommand(format!(
            "{} already exists",
            created.path
        )));
    }
    fs::create_dir_all(path.parent().expect("in a Collection's folder"))?;
    fs::write(&path, &created.text)?;
    for field in &created.unfilled {
        out.say(&format!(
            "warning: fill in `{field}` in {}: it needs an image",
            created.path
        ));
    }
    if out.json {
        out.event(Event::EntryCreated {
            path: created.path,
            url: created.url,
            collection: collection.to_string(),
            unfilled: created.unfilled,
        });
    } else {
        stdout(&format!("{}\n{}", created.path, created.url));
    }
    Ok(0)
}

fn routes(root: &Path, drafts: bool, out: &Out) -> Result<u8, Error> {
    let run = engine_build(root, None, Mode::Check { drafts }, false)?;
    let Some((config, site)) = site_or_report(&run, out) else {
        return Ok(SITE_ERRORS);
    };
    let templates = yuanme_render::template::route_templates(config, site);
    let routes: Vec<events::Route> = site
        .routes
        .iter()
        .zip(templates)
        .map(|(r, template)| {
            let mut route = events::Route {
                url: r.url.clone(),
                output: r.output.clone(),
                kind: "",
                template,
                entry: None,
                collection: None,
                taxonomy: None,
                term: None,
                page: None,
                name: None,
            };
            match &r.kind {
                RouteKind::Entry(i) => {
                    route.kind = "entry";
                    route.entry = Some(site.entries[*i].path.clone());
                    route.collection = Some(site.entries[*i].collection.clone());
                }
                RouteKind::List {
                    collection, page, ..
                } => {
                    route.kind = "list";
                    route.collection = Some(collection.clone());
                    route.page = Some(*page);
                }
                RouteKind::Term {
                    taxonomy,
                    term,
                    page,
                    ..
                } => {
                    route.kind = "term";
                    route.term = Some(site.taxonomies[taxonomy][*term].name.clone());
                    route.taxonomy = Some(taxonomy.clone());
                    route.page = Some(*page);
                }
                RouteKind::Terms { taxonomy } => {
                    route.kind = "index";
                    route.taxonomy = Some(taxonomy.clone());
                }
                RouteKind::Template(name) => {
                    route.kind = "template";
                    route.name = Some(name.clone());
                }
                RouteKind::Feed { name, .. } => {
                    route.kind = "feed";
                    route.name = Some(name.clone());
                }
            }
            route
        })
        .collect();
    if out.json {
        for route in routes {
            out.event(Event::Route(route));
        }
        return Ok(0);
    }
    let from = |r: &events::Route| {
        let page = |p: Option<usize>| match p {
            Some(n) if n > 1 => format!(", page {n}"),
            _ => String::new(),
        };
        match r.kind {
            "entry" => r.entry.clone().unwrap_or_default(),
            "list" => format!(
                "{} list{}",
                r.collection.as_deref().unwrap_or(""),
                page(r.page)
            ),
            "term" => format!(
                "{} `{}`{}",
                r.taxonomy.as_deref().unwrap_or(""),
                r.term.as_deref().unwrap_or(""),
                page(r.page)
            ),
            "index" => format!("{} index", r.taxonomy.as_deref().unwrap_or("")),
            _ => r.name.clone().unwrap_or_default(),
        }
    };
    let rows: Vec<[String; 4]> = routes
        .iter()
        .map(|r| {
            [
                r.url.clone(),
                r.kind.to_string(),
                from(r),
                r.template.clone(),
            ]
        })
        .collect();
    let width = |i: usize| rows.iter().map(|r| r[i].chars().count()).max().unwrap_or(0);
    let widths = [width(0), width(1), width(2)];
    for row in &rows {
        let mut line = String::new();
        for (cell, w) in row.iter().zip(widths) {
            line.push_str(&format!("{cell:w$}  "));
        }
        line.push_str(&row[3]);
        stdout(&line);
    }
    Ok(0)
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
    let run = engine_build(root, None, Mode::Check { drafts: false }, false)?;
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

fn docs(name: &str, out: &Out) -> Result<u8, Error> {
    let Some(markdown) = docs::page(name) else {
        let names: Vec<&str> = docs::names().collect();
        let otherwise = "`yuanme docs` lists every page".to_string();
        return Err(unknown("docs page", name, &names, otherwise));
    };
    if out.json {
        out.event(Event::Docs {
            page: name.to_string(),
            markdown,
        });
    } else {
        stdout(markdown.trim_end());
    }
    Ok(0)
}
