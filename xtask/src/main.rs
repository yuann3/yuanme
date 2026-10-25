use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("package-check") => package_check(),
        Some("vendor-check") => vendor_check(),
        _ => {
            eprintln!("usage: cargo xtask package-check | vendor-check");
            ExitCode::FAILURE
        }
    }
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is inside the workspace")
}

/// Fails if a vendored fork differs from its fork's repository at the
/// commit its manifest names (#27). The fork is where it is changed and
/// rebased; vendor/ is a copy, so `cargo install` needs no git source.
/// Only the manifest, and the files it lists as `own`, are yuanme's.
fn vendor_check() -> ExitCode {
    let mut ok = true;
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root().join("vendor"))
        .expect("read vendor/")
        .map(|e| e.expect("read a vendored crate").path())
        .collect();
    dirs.sort();
    for dir in dirs {
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).expect("read its manifest");
        let Some(fork) = fork_metadata(&manifest) else {
            continue;
        };
        let (Some(repository), Some(rev)) = (fork.get("repository"), fork.get("rev")) else {
            eprintln!(
                "{}: [package.metadata.fork] needs `repository` and `rev`",
                dir.display()
            );
            ok = false;
            continue;
        };
        let prefix = fork.get("dir").map_or(String::new(), |d| format!("{d}/"));
        let own: Vec<&str> = fork
            .get("own")
            .map_or(Vec::new(), |o| o.split('"').skip(1).step_by(2).collect());
        let clone =
            std::env::temp_dir().join(format!("yuanme-vendor-check-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&clone);
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .arg("-C")
                .arg(&clone)
                .args(args)
                .output()
                .expect("run git");
            if !out.status.success() {
                panic!(
                    "git {args:?} failed:\n{}",
                    String::from_utf8_lossy(&out.stderr)
                );
            }
            String::from_utf8(out.stdout).expect("git prints UTF-8")
        };
        std::fs::create_dir_all(&clone).expect("make a scratch clone");
        git(&["init", "-q"]);
        git(&["fetch", "-q", "--depth", "1", repository, rev]);
        // Each file of the fork's tree, by its blob hash. A symlink, such as
        // ravif's LICENSE, is vendored as the file it points to.
        let tree: BTreeMap<String, (String, String)> = git(&["ls-tree", "-r", "FETCH_HEAD"])
            .lines()
            .filter_map(|line| {
                let (meta, path) = line.split_once('\t')?;
                let mut meta = meta.split_whitespace();
                let mode = meta.next()?.to_string();
                let hash = meta.nth(1)?.to_string();
                Some((path.to_string(), (mode, hash)))
            })
            .collect();
        let theirs: BTreeMap<String, String> = tree
            .iter()
            .filter_map(|(path, (mode, hash))| {
                let hash = if mode == "120000" {
                    let target = git(&["cat-file", "-p", hash]);
                    let parent = Path::new(path).parent().unwrap_or(Path::new(""));
                    let resolved = normalize(&parent.join(target.trim()));
                    tree.get(&resolved)?.1.clone()
                } else {
                    hash.clone()
                };
                Some((path.strip_prefix(&prefix)?.to_string(), hash))
            })
            .collect();
        for file in files(&dir) {
            let path = file
                .strip_prefix(&dir)
                .expect("under it")
                .to_str()
                .expect("UTF-8 path")
                .to_string();
            if path == "Cargo.toml" || own.contains(&path.as_str()) {
                continue;
            }
            let ours = git(&["hash-object", file.to_str().expect("UTF-8 path")]);
            match theirs.get(&path) {
                Some(hash) if *hash == ours.trim() => {}
                Some(_) => {
                    eprintln!(
                        "{}: {path} differs from {repository} at {rev}",
                        dir.display()
                    );
                    ok = false;
                }
                None => {
                    eprintln!("{}: {path} isn't in {repository} at {rev}", dir.display());
                    ok = false;
                }
            }
        }
        let _ = std::fs::remove_dir_all(&clone);
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// A repository path with `.` and `..` resolved.
fn normalize(path: &Path) -> String {
    let mut out: Vec<&str> = Vec::new();
    for part in path.to_str().expect("UTF-8 path").split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            part => out.push(part),
        }
    }
    out.join("/")
}

/// The `[package.metadata.fork]` table, as `key = value` lines.
fn fork_metadata(manifest: &str) -> Option<BTreeMap<&str, &str>> {
    let start = manifest.find("\n[package.metadata.fork]\n")?;
    let table = manifest[start + 1..]
        .lines()
        .skip(1)
        .take_while(|l| !l.starts_with('['));
    Some(
        table
            .filter_map(|l| l.split_once('='))
            .map(|(k, v)| (k.trim(), v.trim().trim_matches('"')))
            .collect(),
    )
}

fn files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).expect("read a vendored folder") {
        let path = entry.expect("read a vendored file").path();
        if path.is_dir() {
            out.extend(files(&path));
        } else {
            out.push(path);
        }
    }
    out
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
    let root = root();
    // Every published crate lives in crates/ or vendor/; xtask sits outside
    // them because it never ships.
    let mut crates: Vec<_> = ["crates", "vendor"]
        .iter()
        .flat_map(|dir| std::fs::read_dir(root.join(dir)).expect("read the crates' directory"))
        .map(|entry| entry.expect("read a crate's directory").path())
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
            // A vendored crate ships its licence.
            if !(CARGO_FILES.contains(&file)
                || file.starts_with("src/")
                || file == "build.rs"
                || file == "LICENSE"
                || file == "COPYING")
            {
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
