//! Sets YUANME_ENGINE_VERSION, which every cache key starts with (ADR 0011).
//! A release is its version. A checkout adds the commit and whether it was
//! dirty, so an Engine developer's change never reads stale cache entries.

use std::path::Path;
use std::process::Command;

fn main() {
    let version = std::env::var("CARGO_PKG_VERSION").expect("set by Cargo");
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("set by Cargo");
    let workspace = Path::new(&manifest)
        .ancestors()
        .nth(2)
        .expect("the crate is in crates/");
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(workspace)
            .output()
            .ok()
            .filter(|o| o.status.success())?;
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    };

    // Unpacked from crates.io, the crate may still sit inside some other
    // repository, such as a home directory kept in git: only yuanme's own
    // checkout counts.
    let ours = git(&["rev-parse", "--show-toplevel"])
        .is_some_and(|top| Path::new(&top) == workspace.canonicalize().unwrap_or_default());
    // The release workflow builds from a checkout too, and sets this
    // (.github/build-setup.yml).
    println!("cargo::rerun-if-env-changed=YUANME_RELEASE");
    let release = std::env::var_os("YUANME_RELEASE").is_some();
    let engine = match git(&["rev-parse", "HEAD"]).filter(|_| ours && !release) {
        Some(commit) => {
            let status = git(&["status", "--porcelain", "--untracked-files=no"]);
            let dirty = status.is_none_or(|s| !s.is_empty());
            format!("{version}+{commit}{}", if dirty { ".dirty" } else { "" })
        }
        None => version,
    };
    println!("cargo::rustc-env=YUANME_ENGINE_VERSION={engine}");

    if ours && let Some(git_dir) = git(&["rev-parse", "--absolute-git-dir"]) {
        // A commit or checkout appends to logs/HEAD; staging rewrites the
        // index; an edit anywhere in the Engine's sources changes crates/.
        println!("cargo::rerun-if-changed={git_dir}/logs/HEAD");
        println!("cargo::rerun-if-changed={git_dir}/index");
        println!(
            "cargo::rerun-if-changed={}",
            workspace.join("crates").display()
        );
        println!(
            "cargo::rerun-if-changed={}",
            workspace.join("Cargo.lock").display()
        );
    }
}
