use std::path::Path;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("package-check") => package_check(),
        _ => {
            eprintln!("usage: cargo xtask package-check");
            ExitCode::FAILURE
        }
    }
}

/// Files Cargo adds to every package on its own.
const CARGO_FILES: [&str; 4] = [
    ".cargo_vcs_info.json",
    "Cargo.lock",
    "Cargo.toml",
    "Cargo.toml.orig",
];

/// Fails if a published crate would ship anything but its sources. The
/// repository holds GPL and GFDL test data (#14) that must never reach
/// crates.io (#27).
fn package_check() -> ExitCode {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is inside the workspace");
    // Every published crate lives in crates/; xtask sits outside it because it never ships.
    let mut crates: Vec<_> = std::fs::read_dir(root.join("crates"))
        .expect("read crates/")
        .map(|entry| entry.expect("read crates/ entry").path())
        .collect();
    crates.sort();

    let mut ok = true;
    for dir in crates {
        let output = Command::new(env!("CARGO"))
            .args(["package", "--list", "--allow-dirty", "--manifest-path"])
            .arg(dir.join("Cargo.toml"))
            .output()
            .expect("run cargo package --list");
        if !output.status.success() {
            eprintln!(
                "{}: cargo package --list failed:\n{}",
                dir.display(),
                String::from_utf8_lossy(&output.stderr)
            );
            ok = false;
            continue;
        }
        for file in String::from_utf8_lossy(&output.stdout).lines() {
            if !(CARGO_FILES.contains(&file) || file.starts_with("src/")) {
                eprintln!("{}: would publish {file}", dir.display());
                ok = false;
            }
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
