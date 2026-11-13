mod cli;
mod commands;
mod dev;
mod events;
mod out;
mod project;
mod report;
mod schemas;
#[cfg(test)]
mod tests;

use std::fmt;
use std::fs;
use std::io::{self, IsTerminal};
use std::process::ExitCode;

use clap::Parser;
use yuanme_content::{Adapter, config_schema, parse_config, similar};
use yuanme_diag::Files;

use cli::{Cli, Color, Command, DevArgs, Itself, Which};
use commands::{build, check, docs, init, new, routes, schema, update};
use out::Out;

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

/// Why a command couldn't finish, which picks its exit code. A module
/// with errors of its own converts them into one of these.
#[derive(Debug)]
enum Error {
    BadCommand(String),
    Io(io::Error),
}

impl Error {
    /// `name` is not one of `choices`; `otherwise` says where to look when
    /// none is close.
    fn unknown(what: &str, name: &str, choices: &[&str], otherwise: String) -> Error {
        let near = similar(name, choices.iter().copied());
        Error::BadCommand(match near {
            Some(near) => format!("there is no {what} `{name}`; did you mean `{near}`?"),
            None => format!("there is no {what} `{name}`; {otherwise}"),
        })
    }

    fn exit_code(&self) -> u8 {
        match self {
            Error::BadCommand(_) => BAD_COMMAND,
            Error::Io(_) => INTERNAL,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::BadCommand(message) => f.write_str(message),
            Error::Io(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

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
        Err(e) => {
            eprintln!("error: {e}");
            e.exit_code()
        }
    })
}

fn run(cli: Cli, out: &Out) -> Result<u8, Error> {
    let cwd = std::env::current_dir()?;
    let dir = cli.dir.map_or(cwd.clone(), |d| cwd.join(d));
    match cli.command {
        Command::Docs { topic } => return docs::run(topic.as_deref().unwrap_or("index"), out),
        Command::Itself(Itself::Update { version, check }) => {
            return update::run(version, check, out);
        }
        Command::Init {
            folder,
            starter,
            host,
        } => {
            let host =
                host.map(|h| Adapter::named(&h).expect("clap accepts only an Adapter's name"));
            return init::run(&dir, folder.as_deref(), starter, host, out);
        }
        Command::Schema {
            which: Which::Config,
        } => return Ok(schema::print(out, "config", config_schema())),
        Command::Schema {
            which: Which::Events,
        } => return Ok(schema::print(out, "events", events::schema())),
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
    // For the Author's editor, whether or not the build checks types.
    yuanme_engine::write_types(&root)?;
    if let Command::Build(_) | Command::Check(_) | Command::Dev(DevArgs { manage: None, .. }) =
        cli.command
    {
        // Only for a project this Engine can build: an older one's files
        // are right for it.
        let text = fs::read_to_string(root.join("yuanme.toml"))?;
        let mut files = Files::default();
        let file = files.add("yuanme.toml", text);
        if let (Some(config), _) = parse_config(&files, file, ENGINE_VERSION) {
            init::refresh(&root, config.agents.manage_agents_md, out)?;
        }
    }
    match cli.command {
        Command::Build(args) => build::run(&root, args, out),
        Command::Check(args) => check::run(&root, &args, out),
        Command::Dev(args) => dev::run(&root, &args, *out),
        Command::New {
            collection,
            title,
            format,
        } => new::run(&root, &collection, &title, format, out),
        Command::Routes { drafts } => routes::run(&root, drafts, out),
        Command::Schema { which } => schema::run(&root, which, out),
        Command::Docs { .. } | Command::Init { .. } | Command::Itself(_) => {
            unreachable!("answered without a project")
        }
    }
}
