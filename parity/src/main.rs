//! `cargo run -p parity -- check BASELINE SITE`: URL equality and the
//! normalized DOM. BASELINE is an unpacked Parity baseline (`fetch.sh`),
//! SITE a built Site. `build` builds one with the Engine's CLI, and
//! `self-test` checks the harness against the baseline itself.

use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus};

use parity::{Difference, Report, compare, exception_ids};
use serde_json::Value;

mod seeds;

const USAGE: &str = "usage:
  parity check BASELINE SITE
  parity build PROJECT OUT [--yuanme BIN]
  parity self-test BASELINE";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args[..] {
        ["check", baseline, site] => check(Path::new(baseline), Path::new(site)),
        ["build", project, out] => build(Path::new(project), Path::new(out), "yuanme"),
        ["build", project, out, "--yuanme", bin] => build(Path::new(project), Path::new(out), bin),
        ["self-test", baseline] => self_test(Path::new(baseline)),
        _ => Err(Error::Usage),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("parity: {e}");
            ExitCode::from(2)
        }
    }
}

/// Why a command couldn't finish, which exits with 2.
#[derive(Debug)]
enum Error {
    Usage,
    /// `docs/parity-exceptions.md` couldn't be read.
    Exceptions(PathBuf, io::Error),
    NotABaseline(PathBuf),
    Compare(parity::Error),
    Out(io::Error),
    Run {
        yuanme: String,
        error: io::Error,
    },
    /// A line of yuanme's stdout that isn't a JSON event.
    Event {
        yuanme: String,
        line: String,
        error: serde_json::Error,
    },
    NotFinished {
        yuanme: String,
        status: ExitStatus,
        stderr: String,
    },
    Temp(io::Error),
    Copy(seeds::Error),
    Seed {
        name: &'static str,
        error: seeds::Error,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage => f.write_str(USAGE),
            Error::Exceptions(file, e) => write!(f, "{}: {e}", file.display()),
            Error::NotABaseline(baseline) => write!(
                f,
                "{} isn't an unpacked Parity baseline: it has no site/",
                baseline.display()
            ),
            Error::Compare(e) => e.fmt(f),
            Error::Out(e) | Error::Temp(e) => e.fmt(f),
            Error::Run { yuanme, error } => write!(f, "can't run {yuanme}: {error}"),
            Error::Event {
                yuanme,
                line,
                error,
            } => write!(f, "{yuanme} printed {line:?}: {error}"),
            Error::NotFinished {
                yuanme,
                status,
                stderr,
            } => write!(
                f,
                "{yuanme} build ended ({status}) without a build-finished event:\n{stderr}"
            ),
            Error::Copy(e) => e.fmt(f),
            Error::Seed { name, error } => write!(f, "seed {name}: {error}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<parity::Error> for Error {
    fn from(e: parity::Error) -> Error {
        Error::Compare(e)
    }
}

fn ids() -> Result<BTreeSet<String>, Error> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/parity-exceptions.md");
    exception_ids(&file).map_err(|e| Error::Exceptions(file, e))
}

/// The Site inside an unpacked Parity baseline.
fn baseline_site(baseline: &Path) -> Result<PathBuf, Error> {
    let site = baseline.join("site");
    match site.is_dir() {
        true => Ok(site),
        false => Err(Error::NotABaseline(baseline.to_path_buf())),
    }
}

fn check(baseline: &Path, site: &Path) -> Result<bool, Error> {
    let report = compare(&baseline_site(baseline)?, site, &ids()?)?;
    for d in &report.differences {
        println!("{d}");
    }
    print_tolerated(&report);
    println!(
        "URLs and DOM: {} differences from the Parity baseline",
        report.differences.len()
    );
    Ok(report.differences.is_empty())
}

fn print_tolerated(report: &Report) {
    for (id, n) in &report.tolerated {
        println!("`{id}` applied to {n} URLs, paths or elements");
    }
}

/// Builds PROJECT into OUT as #65's CLI does for an agent, and reads its
/// `build-finished` event. The report is in PROJECT/.yuanme/report.json.
fn build(project: &Path, out: &Path, yuanme: &str) -> Result<bool, Error> {
    let out = std::path::absolute(out).map_err(Error::Out)?;
    let run = Command::new(yuanme)
        .arg("-C")
        .arg(project)
        .args(["build", "--json", "--out"])
        .arg(&out)
        .output()
        .map_err(|error| Error::Run {
            yuanme: yuanme.to_string(),
            error,
        })?;
    let mut finished = None;
    for line in String::from_utf8_lossy(&run.stdout).lines() {
        let event: Value = serde_json::from_str(line).map_err(|error| Error::Event {
            yuanme: yuanme.to_string(),
            line: line.to_string(),
            error,
        })?;
        match event["type"].as_str() {
            Some("diagnostic") => eprintln!(
                "{}",
                event["rendered"]
                    .as_str()
                    .or(event["message"].as_str())
                    .unwrap_or(line)
            ),
            Some("build-finished") => finished = Some(event),
            _ => {}
        }
    }
    let Some(finished) = finished else {
        return Err(Error::NotFinished {
            yuanme: yuanme.to_string(),
            status: run.status,
            stderr: String::from_utf8_lossy(&run.stderr).into_owned(),
        });
    };
    if finished["ok"] != true {
        eprintln!(
            "parity: the build failed with {} errors",
            finished["errors"]
        );
        return Ok(false);
    }
    println!("built {} Pages into {}", finished["pages"], out.display());
    Ok(true)
}

/// The baseline compared with itself passes, and every seeded change is
/// caught, or let through, as it should be.
fn self_test(baseline: &Path) -> Result<bool, Error> {
    let site = baseline_site(baseline)?;
    let ids = ids()?;
    let mut ok = true;
    let report = compare(&site, &site, &ids)?;
    if !report.differences.is_empty() {
        ok = false;
        for d in &report.differences {
            println!("{d}");
        }
    }
    println!(
        "{} the baseline against itself",
        if ok { "ok  " } else { "FAIL" }
    );
    for seed in seeds::SEEDS {
        let copy = tempfile::tempdir().map_err(Error::Temp)?;
        seeds::copy(&site, copy.path()).map_err(Error::Copy)?;
        seed.apply(copy.path()).map_err(|error| Error::Seed {
            name: seed.name,
            error,
        })?;
        let report = compare(&site, copy.path(), &ids)?;
        let pass = (seed.expect)(&report.differences);
        ok &= pass;
        println!(
            "{} {}: {}",
            if pass { "ok  " } else { "FAIL" },
            seed.name,
            describe(&report.differences)
        );
        if !pass {
            for d in &report.differences {
                println!("{d}");
            }
        }
    }
    Ok(ok)
}

fn describe(differences: &[Difference]) -> String {
    match differences {
        [] => "no differences".to_string(),
        [one] => one
            .to_string()
            .lines()
            .next()
            .unwrap_or_default()
            .to_string(),
        many => format!("{} differences", many.len()),
    }
}
