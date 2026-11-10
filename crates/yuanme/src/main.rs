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

use std::fs;
use std::io::{self, IsTerminal};
use std::process::ExitCode;

use clap::Parser;
use yuanme_content::config::{self, Adapter};
use yuanme_diag::Files;

use cli::{Cli, Color, Command, DevArgs, Itself, Manage, Which};
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

#[derive(Debug)]
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
            let host = host.map(|h| Adapter::named(&h).expect("one of its possible values"));
            return init::run(&dir, folder.as_deref(), starter, host, out);
        }
        Command::Schema {
            which: Which::Config,
        } => return Ok(schema::print(out, "config", config::json_schema())),
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
    if let Command::Build { .. } | Command::Check(_) | Command::Dev(DevArgs { manage: None, .. }) =
        cli.command
    {
        // Only for a project this Engine can build: an older one's files
        // are right for it.
        let text = fs::read_to_string(root.join("yuanme.toml"))?;
        let mut files = Files::default();
        let file = files.add("yuanme.toml", text);
        if let (Some(config), _) = config::parse(&files, file, ENGINE_VERSION) {
            init::refresh(&root, config.agents.manage_agents_md, out)?;
        }
    }
    match cli.command {
        Command::Build {
            check,
            strict_budgets,
            clean,
            out: out_dir,
        } => {
            let out_dir = out_dir.map(|d| root.join(d));
            build::run(&root, out_dir, clean, &check, strict_budgets, out)
        }
        Command::Check(args) => check::run(&root, &args, out),
        Command::Dev(args) => match args.manage {
            Some(Manage::Status) => dev::running::status(&root, *out),
            Some(Manage::Stop) => dev::running::stop(&root, *out),
            Some(Manage::Logs { follow }) => dev::running::logs(&root, follow),
            None if args.background => {
                let mut forwarded = Vec::new();
                if let Some(port) = args.port {
                    forwarded.extend(["--port".to_string(), port.to_string()]);
                }
                if args.poll {
                    forwarded.push("--poll".to_string());
                }
                if let Some(dir) = &args.write {
                    let dir = root.join(dir).to_string_lossy().into_owned();
                    forwarded.extend(["--write".to_string(), dir]);
                }
                dev::running::background(&root, &forwarded, *out)
            }
            None => dev::run(
                &root,
                args.port,
                args.poll,
                args.write.as_deref(),
                args.log.as_deref(),
                *out,
            ),
        },
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

/// `name` is not one of `choices`; `otherwise` says where to look when
/// none is close.
fn unknown(what: &str, name: &str, choices: &[&str], otherwise: String) -> Error {
    let near = config::similar(name, choices.iter().copied());
    Error::BadCommand(match near {
        Some(near) => format!("there is no {what} `{name}`; did you mean `{near}`?"),
        None => format!("there is no {what} `{name}`; {otherwise}"),
    })
}
