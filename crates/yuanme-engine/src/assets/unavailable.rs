//! Why a Managed tool can't run, which a build reports as an
//! `asset/tool-unavailable` with what to do about it.

use std::fmt;
use std::io;
use std::path::PathBuf;

use yuanme_assets::{TAILWIND_ENV, TSC_ENV, TSC_VERSION, ToolError, TscError};

#[derive(Clone, Copy, Debug)]
pub(super) enum Tool {
    Tailwind,
    Tsc,
}

#[derive(Debug)]
pub(super) enum Unavailable {
    /// The tool publishes no binary for this platform.
    NoRelease(Tool),
    /// There is no folder to cache it in: `HOME` isn't set.
    NoCache(Tool),
    /// It couldn't be downloaded or installed.
    Download(ToolError),
    /// The tsconfig `tsc` runs on couldn't be written to `.yuanme`.
    Tsconfig { path: PathBuf, error: io::Error },
    /// `tsc` didn't run.
    Tsc(TscError),
}

impl Unavailable {
    pub(super) fn help(&self) -> String {
        match self {
            Unavailable::NoRelease(tool) | Unavailable::NoCache(tool) => match tool {
                Tool::Tailwind => format!("set `{TAILWIND_ENV}` to a Tailwind CLI binary"),
                Tool::Tsc => {
                    format!("set `{TSC_ENV}` to the `tsc` of a TypeScript {TSC_VERSION} release")
                }
            },
            Unavailable::Download(e) => e.help(),
            Unavailable::Tsconfig { .. } => "make `.yuanme` writable".into(),
            Unavailable::Tsc(_) => format!("set `{TSC_ENV}` to a `tsc` that runs here"),
        }
    }
}

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let platform = || format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        match self {
            Unavailable::NoRelease(Tool::Tailwind) => {
                write!(f, "Tailwind publishes no CLI for {}", platform())
            }
            Unavailable::NoRelease(Tool::Tsc) => {
                write!(f, "TypeScript publishes no `tsc` for {}", platform())
            }
            Unavailable::NoCache(Tool::Tailwind) => {
                f.write_str("there is no folder to keep the Tailwind CLI in: `HOME` isn't set")
            }
            Unavailable::NoCache(Tool::Tsc) => {
                f.write_str("there is no folder to keep `tsc` in: `HOME` isn't set")
            }
            Unavailable::Download(e) => e.fmt(f),
            Unavailable::Tsconfig { path, error } => {
                write!(f, "can't write {}: {error}", path.display())
            }
            Unavailable::Tsc(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Unavailable {}
