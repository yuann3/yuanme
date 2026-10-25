use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("package-check") => package_check(),
        Some("vendor-check") => vendor_check(),
        Some("docs") => docs(),
        Some("notices") => notices(),
        Some("release-version") => match std::env::args().nth(2) {
            Some(version) => release_version(&version),
            None => {
                eprintln!("usage: cargo xtask release-version X.Y.Z");
                ExitCode::FAILURE
            }
        },
        _ => {
            eprintln!(
                "usage: cargo xtask package-check | vendor-check | docs | notices | release-version X.Y.Z"
            );
            ExitCode::FAILURE
        }
    }
}

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is inside the workspace")
}

/// Writes the docs site's copy of every page `yuanme docs` prints into
/// `site/content/` (#38). The pages are the binary's, so the test that
/// fails while the copies are stale also writes them.
fn docs() -> ExitCode {
    let status = Command::new(env!("CARGO"))
        .args(["test", "--locked", "-p", "yuanme", "--bin", "yuanme", "--"])
        .args([
            "--exact",
            "tests::the_docs_site_has_every_page_as_this_binary_makes_it",
        ])
        .env("YUANME_UPDATE_DOCS", "1")
        .current_dir(root())
        .status()
        .expect("run cargo test");
    if status.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
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

/// Writes THIRD_PARTY_NOTICES.md, which every release archive carries
/// (#27): each crate compiled into the binary under its licence, from
/// cargo-about, then each NOTICE file beside data a crate embeds, such as
/// the grammars and themes. Fails if a licence isn't one about.toml
/// accepts, or if a vendored fork isn't listed with its own licence text.
fn notices() -> ExitCode {
    let root = root();
    // Offline, so the text depends only on the lockfile's crates.
    let fetch = Command::new(env!("CARGO"))
        .args(["fetch", "--locked"])
        .current_dir(root)
        .status()
        .expect("run cargo fetch");
    if !fetch.success() {
        return ExitCode::FAILURE;
    }
    let path = root.join("THIRD_PARTY_NOTICES.md");
    // To a file: on Windows, cargo-about refuses to print to a pipe.
    let about = Command::new(env!("CARGO"))
        .args(["about", "generate", "--frozen", "--config", "about.toml"])
        .args(["--manifest-path", "crates/yuanme/Cargo.toml", "about.hbs"])
        .arg("--output-file")
        .arg(&path)
        .current_dir(root)
        .status()
        .expect("run cargo about, from `cargo install --locked cargo-about`");
    if !about.success() {
        return ExitCode::FAILURE;
    }
    let mut out = std::fs::read_to_string(&path).expect("read cargo about's output");
    std::fs::remove_file(&path).expect("remove it until it is complete");

    let mut ok = true;
    for dir in std::fs::read_dir(root.join("vendor")).expect("read vendor/") {
        let dir = dir.expect("read a vendored crate").path();
        let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).expect("read its manifest");
        let name = manifest_value(&manifest, "name");
        let licence = ["LICENSE", "COPYING"]
            .iter()
            .find_map(|f| std::fs::read_to_string(dir.join(f)).ok())
            .expect("a vendored crate ships its licence");
        let listed = out.split("\n### ").any(|section| {
            section.contains(&format!("\n- [{name} ")) && words(section).contains(&words(&licence))
        });
        if !listed {
            eprintln!("THIRD_PARTY_NOTICES.md doesn't list {name} with its own licence");
            ok = false;
        }
    }

    let mut embedded: Vec<PathBuf> = std::fs::read_dir(root.join("crates"))
        .expect("read crates/")
        .flat_map(|c| files(&c.expect("read a crate").path().join("src")))
        .filter(|f| f.file_name() == Some("NOTICE".as_ref()))
        .collect();
    embedded.sort();
    if embedded.is_empty() {
        eprintln!("no NOTICE in crates/*/src: the grammars' licences would be missing");
        ok = false;
    }
    for notice in embedded {
        let folder = notice.parent().expect("in a folder");
        let folder: Vec<_> = folder
            .strip_prefix(root)
            .expect("under the root")
            .iter()
            .map(|c| c.to_string_lossy())
            .collect();
        let text = std::fs::read_to_string(&notice).expect("read a NOTICE");
        out.push_str(&format!(
            "\n## {}\n\n```text\n{}\n```\n",
            folder.join("/"),
            text.trim_end()
        ));
    }

    if !ok {
        return ExitCode::FAILURE;
    }
    std::fs::write(&path, out).expect("write THIRD_PARTY_NOTICES.md");
    ExitCode::SUCCESS
}

/// Sets the versions a release publishes (ADR 0015): `yuanme` at the
/// release's own, and every library crate at the next 0.0.N, since
/// crates.io takes each version once. Then updates the lockfiles that list
/// them, the fuzz workspaces' too.
fn release_version(version: &str) -> ExitCode {
    let numbers: Vec<&str> = version.split('.').collect();
    if numbers.len() != 3 || numbers.iter().any(|n| n.parse::<u64>().is_err()) {
        eprintln!("{version} isn't X.Y.Z; a release's tag is vX.Y.Z");
        return ExitCode::FAILURE;
    }
    let root = root();
    let path = root.join("Cargo.toml");
    let workspace = std::fs::read_to_string(&path).expect("read Cargo.toml");
    let old = manifest_value(&workspace, "version");
    let n: u64 = old
        .strip_prefix("0.0.")
        .and_then(|n| n.parse().ok())
        .expect("the library crates are at 0.0.N");
    // [workspace.package] and every requirement in [workspace.dependencies].
    let workspace = workspace.replace(&format!("\"{old}\""), &format!("\"0.0.{}\"", n + 1));
    std::fs::write(&path, workspace).expect("write Cargo.toml");

    let path = root.join("crates/yuanme/Cargo.toml");
    let manifest = std::fs::read_to_string(&path).expect("read yuanme's manifest");
    let line = manifest
        .lines()
        .find(|l| l.starts_with("version"))
        .expect("yuanme has a version");
    let manifest = manifest.replacen(line, &format!("version = \"{version}\""), 1);
    std::fs::write(&path, manifest).expect("write yuanme's manifest");

    for manifest in [
        "Cargo.toml",
        "fuzz/Cargo.toml",
        "vendor/organic/fuzz/Cargo.toml",
    ] {
        let status = Command::new(env!("CARGO"))
            .args(["update", "--workspace", "--manifest-path", manifest])
            .current_dir(root)
            .status()
            .expect("run cargo update");
        if !status.success() {
            return ExitCode::FAILURE;
        }
    }
    println!("yuanme {version}, library crates 0.0.{}", n + 1);
    ExitCode::SUCCESS
}

/// A `key = "value"` line's value, from the manifest's first table.
fn manifest_value<'a>(manifest: &'a str, key: &str) -> &'a str {
    manifest
        .lines()
        .filter_map(|l| l.split_once('='))
        .find(|(k, _)| k.trim() == key)
        .map(|(_, v)| v.trim().trim_matches('"'))
        .expect("the manifest sets it")
}

/// Text with its line breaks and indentation ignored.
fn words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
