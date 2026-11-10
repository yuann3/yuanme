//! The command line, as clap parses it. Its doc comments are the help
//! text, which `docs::cli_reference` renders into the `cli` docs page.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use yuanme_content::config::Adapter;

use crate::ENGINE_VERSION;
use crate::commands::{init, new, update};

#[derive(Parser)]
#[command(
    name = "yuanme",
    version = ENGINE_VERSION,
    about = "A static site generator for Markdown, Org and math, built to be driven by coding agents."
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
    /// Print NDJSON events on stdout, one per line. Human text, if any, goes to stderr.
    #[arg(long, global = true)]
    pub(crate) json: bool,
    /// Run as if yuanme was started in DIR.
    #[arg(short = 'C', value_name = "DIR", global = true)]
    pub(crate) dir: Option<PathBuf>,
    /// Colour human output. `auto` colours it when stdout is a terminal.
    #[arg(long, value_enum, default_value_t = Color::Auto, global = true)]
    pub(crate) color: Color,
    /// Print only errors.
    #[arg(long, short, global = true)]
    pub(crate) quiet: bool,
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Color {
    Auto,
    Always,
    Never,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Create a Site from a Starter, with the files coding agents read. It overwrites no file.
    Init {
        /// The Site's folder, which may hold other files already. Defaults to the current one.
        // Not `dir`, which is `-C`'s id.
        #[arg(value_name = "DIR")]
        folder: Option<PathBuf>,
        /// The Starter to begin from.
        #[arg(long, value_enum, default_value_t = init::Starter::Blog)]
        starter: init::Starter,
        /// Deploy to this host: set `[host] adapter`, and write its GitHub Actions deploy workflow and files.
        #[arg(long, value_parser = Adapter::ALL.map(Adapter::name))]
        host: Option<String>,
    },
    /// Build the Site into the output directory.
    Build(BuildArgs),
    /// Report every Diagnostic `build` would, rendering every Page in memory and writing none.
    Check(CheckArgs),
    /// Serve the Site with its Drafts from memory, and rebuild it on every change.
    Dev(DevArgs),
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
    /// Update yuanme itself.
    #[command(name = "self", subcommand)]
    Itself(Itself),
}

#[derive(Subcommand)]
pub(crate) enum Itself {
    /// Replace this yuanme with the newest release, or with VERSION, which
    /// may be older, once its signature and hash check. Only for an install
    /// made by the install script.
    Update {
        /// An exact release, such as 0.2.0.
        #[arg(value_parser = update::version)]
        version: Option<String>,
        /// Only say whether a newer release is out.
        #[arg(long, conflicts_with = "version")]
        check: bool,
    },
}

#[derive(Args)]
pub(crate) struct BuildArgs {
    #[command(flatten)]
    pub(crate) check: CheckArgs,
    /// Make every warning about a Site budget an error, as `budgets.strict` does.
    #[arg(long)]
    pub(crate) strict_budgets: bool,
    /// Empty the output directory first, files yuanme didn't write included.
    #[arg(long)]
    pub(crate) clean: bool,
    /// The output directory, from the project root. By default `dist`, or
    /// `.vercel/output` with the vercel adapter.
    #[arg(long, value_name = "DIR")]
    pub(crate) out: Option<PathBuf>,
}

#[derive(Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(crate) struct DevArgs {
    #[command(subcommand)]
    pub(crate) manage: Option<Manage>,
    /// The port on 127.0.0.1. By default 4590, or the next free one.
    #[arg(long)]
    pub(crate) port: Option<u16>,
    /// Look for changes every 100 ms instead of being told of them, for
    /// file systems that tell nothing, such as some network and container mounts.
    #[arg(long)]
    pub(crate) poll: bool,
    /// Run in the background, and return once the first build has finished.
    /// `yuanme dev status`, `logs` and `stop` manage it.
    #[arg(long)]
    pub(crate) background: bool,
    /// Also write each build that succeeds to DIR, from the project root, for
    /// a look at its files. Never the output directory: dev includes Drafts.
    #[arg(long, value_name = "DIR")]
    pub(crate) write: Option<PathBuf>,
    /// Where a server started with `--background` prints, which it records.
    #[arg(long, hide = true, value_name = "FILE")]
    pub(crate) log: Option<String>,
}

/// What manages the dev server running for the project.
#[derive(Subcommand)]
pub(crate) enum Manage {
    /// Say whether a dev server runs for the project, where, and how its last build went.
    Status,
    /// Stop the dev server running for the project.
    Stop,
    /// Print what the dev server started with `--background` printed.
    Logs {
        /// Go on printing what it prints until it stops.
        #[arg(long)]
        follow: bool,
    },
}

#[derive(Args)]
pub(crate) struct CheckArgs {
    /// Include Drafts.
    #[arg(long)]
    pub(crate) drafts: bool,
    /// Make every warning an error.
    #[arg(long)]
    pub(crate) strict: bool,
}

#[derive(Subcommand)]
pub(crate) enum Which {
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
