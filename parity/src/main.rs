//! `cargo run -p parity -- check BASELINE SITE`: URL equality and the
//! normalized DOM. BASELINE is an unpacked Parity baseline (`fetch.sh`),
//! SITE a built Site. `build` builds one with the Engine's CLI, and
//! `self-test` checks the harness against the baseline itself.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

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
        _ => Err(USAGE.to_string()),
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

fn ids() -> Result<BTreeSet<String>, String> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/parity-exceptions.md");
    exception_ids(&file).map_err(|e| format!("{}: {e}", file.display()))
}

/// The Site inside an unpacked Parity baseline.
fn baseline_site(baseline: &Path) -> Result<PathBuf, String> {
    let site = baseline.join("site");
    match site.is_dir() {
        true => Ok(site),
        false => Err(format!(
            "{} isn't an unpacked Parity baseline: it has no site/",
            baseline.display()
        )),
    }
}

fn check(baseline: &Path, site: &Path) -> Result<bool, String> {
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
fn build(project: &Path, out: &Path, yuanme: &str) -> Result<bool, String> {
    let out = std::path::absolute(out).map_err(|e| e.to_string())?;
    let run = Command::new(yuanme)
        .arg("-C")
        .arg(project)
        .args(["build", "--json", "--out"])
        .arg(&out)
        .output()
        .map_err(|e| format!("can't run {yuanme}: {e}"))?;
    let mut finished = None;
    for line in String::from_utf8_lossy(&run.stdout).lines() {
        let event: Value =
            serde_json::from_str(line).map_err(|e| format!("{yuanme} printed {line:?}: {e}"))?;
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
        return Err(format!(
            "{yuanme} build ended ({}) without a build-finished event:\n{}",
            run.status,
            String::from_utf8_lossy(&run.stderr)
        ));
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
fn self_test(baseline: &Path) -> Result<bool, String> {
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
        let copy = tempfile::tempdir().map_err(|e| e.to_string())?;
        seeds::copy(&site, copy.path())?;
        seed.apply(copy.path())
            .map_err(|e| format!("seed {}: {e}", seed.name))?;
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
