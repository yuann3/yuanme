//! `self update` against releases made here as release.yml makes them:
//! dist's archives, sha256.txt, and its minisign signature, from a key
//! made for the test. curl reads them from `file://` URLs, so no test
//! touches the network. The binaries are shell scripts that print a
//! version, which is all `self update` runs.

use std::fs;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use minisign::{KeyPair, SecretKey};
use serde_json::{Value, json};

use super::*;

const TARGET: &str = "x86_64-unknown-linux-musl";
const QUIET: Out = Out {
    json: false,
    terminal: false,
    color: false,
    quiet: true,
};

fn keys() -> (String, SecretKey) {
    let KeyPair { pk, sk } = KeyPair::generate_unencrypted_keypair().unwrap();
    (pk.to_box().unwrap().into_string(), sk)
}

fn script(dir: &Path, prints: &str) -> PathBuf {
    let path = dir.join("yuanme");
    fs::write(&path, format!("#!/bin/sh\n{prints}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

/// `TARGET`'s archive as dist lays it out: one folder, holding the
/// binary and the files #37 lists.
fn archive(dir: &Path, binary: &str) -> Vec<u8> {
    let stage = tempfile::tempdir().unwrap();
    let root = stage.path().join(format!("yuanme-{TARGET}"));
    fs::create_dir(&root).unwrap();
    script(&root, binary);
    for file in [
        "README.md",
        "LICENSE-MIT",
        "LICENSE-APACHE",
        "THIRD_PARTY_NOTICES.md",
    ] {
        fs::write(root.join(file), file).unwrap();
    }
    let out = dir.join("archive.tar.xz");
    let status = Command::new("tar")
        .arg("-cJf")
        .arg(&out)
        .arg("-C")
        .arg(stage.path())
        .arg(format!("yuanme-{TARGET}"))
        .status()
        .unwrap();
    assert!(status.success());
    let bytes = fs::read(&out).unwrap();
    fs::remove_file(out).unwrap();
    bytes
}

/// Publishes `version` under `releases` as release.yml does: the six
/// archives, sha256.txt listing them, and its signature. Only `TARGET`'s
/// archive is real; its binary runs `binary`.
fn publish(releases: &Path, version: &str, sk: &SecretKey, binary: &str) {
    let dir = releases.join(format!("download/v{version}"));
    fs::create_dir_all(&dir).unwrap();
    let mut sums = String::new();
    for target in [
        "aarch64-apple-darwin",
        "aarch64-unknown-linux-musl",
        "x86_64-apple-darwin",
        "x86_64-unknown-linux-musl",
    ] {
        let name = format!("yuanme-{target}.tar.xz");
        let bytes = match target {
            TARGET => archive(&dir, binary),
            _ => target.as_bytes().to_vec(),
        };
        fs::write(dir.join(&name), &bytes).unwrap();
        sums.push_str(&format!("{}  {name}\n", sha256(&bytes)));
    }
    for target in ["aarch64-pc-windows-msvc", "x86_64-pc-windows-msvc"] {
        let name = format!("yuanme-{target}.zip");
        fs::write(dir.join(&name), target).unwrap();
        sums.push_str(&format!("{}  {name}\n", sha256(target.as_bytes())));
    }
    fs::write(dir.join("sha256.txt"), &sums).unwrap();
    sign(&dir, sk, &format!("yuanme v{version}"));
}

fn sign(dir: &Path, sk: &SecretKey, comment: &str) {
    let sums = fs::read(dir.join("sha256.txt")).unwrap();
    let signature = minisign::sign(None, sk, Cursor::new(sums), Some(comment), None).unwrap();
    fs::write(dir.join("sha256.txt.minisig"), signature.into_string()).unwrap();
}

/// GitHub's `releases/latest/download/`, which serves the newest
/// release's files.
fn latest(releases: &Path, version: &str) {
    let dir = releases.join("latest/download");
    fs::create_dir_all(&dir).unwrap();
    for file in ["sha256.txt", "sha256.txt.minisig"] {
        fs::copy(
            releases.join(format!("download/v{version}/{file}")),
            dir.join(file),
        )
        .unwrap();
    }
}

/// Releases 0.1.0 and 0.2.0, the newest, and an install of `installed`
/// made by the install script.
struct Fixture {
    dir: tempfile::TempDir,
    key: String,
    sk: SecretKey,
}

impl Fixture {
    fn new(installed: &str) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let (key, sk) = keys();
        let releases = dir.path().join("releases");
        for version in ["0.1.0", "0.2.0"] {
            publish(&releases, version, &sk, &format!("echo yuanme {version}"));
        }
        latest(&releases, "0.2.0");
        let prefix = dir.path().join("home/.local/bin");
        fs::create_dir_all(&prefix).unwrap();
        script(&prefix, &format!("echo yuanme {installed}"));
        let receipt = dir.path().join("home/.config/yuanme/yuanme-receipt.json");
        fs::create_dir_all(receipt.parent().unwrap()).unwrap();
        // What dist 0.32's installer.sh writes.
        let json = json!({
            "binaries": ["yuanme"],
            "binary_aliases": {},
            "cdylibs": [],
            "cstaticlibs": [],
            "install_layout": "flat",
            "install_prefix": prefix,
            "modify_path": true,
            "provider": { "source": "cargo-dist", "version": "0.32.0" },
            "source": { "app_name": "yuanme", "name": "yuanme", "owner": "yuann3", "release_type": "github" },
            "version": installed,
        });
        fs::write(&receipt, json.to_string()).unwrap();
        Fixture { dir, key, sk }
    }

    fn releases(&self) -> PathBuf {
        self.dir.path().join("releases")
    }

    fn prefix(&self) -> PathBuf {
        self.dir.path().join("home/.local/bin")
    }

    fn receipt(&self) -> PathBuf {
        self.dir
            .path()
            .join("home/.config/yuanme/yuanme-receipt.json")
    }

    fn source(&self) -> (String, String) {
        (
            format!("file://{}", self.releases().display()),
            self.key.clone(),
        )
    }

    fn update(&self, current: &str, version: Option<&str>) -> Result<Outcome, Error> {
        let (url, key) = self.source();
        let source = Source {
            url: &url,
            key: Some(&key),
        };
        let install = Install::find(&self.prefix().join("yuanme"), &[self.receipt()])?;
        update(&source, &install, current, version, TARGET, &QUIET)
    }

    /// What the installed binary prints.
    fn runs(&self) -> String {
        let output = Command::new(self.prefix().join("yuanme"))
            .arg("--version")
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    fn receipt_json(&self) -> Value {
        serde_json::from_str(&fs::read_to_string(self.receipt()).unwrap()).unwrap()
    }

    /// The install prefix's files, so a test sees what an update left.
    fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.prefix())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }
}

fn message(e: Error) -> String {
    match e {
        Error::BadCommand(m) => format!("bad command: {m}"),
        Error::Io(e) => format!("failed: {e}"),
    }
}

#[test]
fn an_update_installs_the_newest_release_and_records_it() {
    let f = Fixture::new("0.1.0");
    let (url, key) = f.source();
    let source = Source {
        url: &url,
        key: Some(&key),
    };
    // `--check`.
    assert_eq!(newest(&source).unwrap(), "0.2.0");

    let outcome = f.update("0.1.0", None).unwrap();
    assert_eq!(outcome.release, "0.2.0");
    assert!(outcome.replaced);
    assert_eq!(f.runs(), "yuanme 0.2.0");
    let receipt = f.receipt_json();
    assert_eq!(receipt["version"], "0.2.0");
    assert_eq!(receipt["install_prefix"], json!(f.prefix()));
    assert_eq!(receipt["provider"]["source"], "cargo-dist");
    assert_eq!(f.files(), ["yuanme"]);

    // Nothing newer: the newest is installed, or this build is ahead of it.
    for current in ["0.2.0", "0.3.0+3f2a9c1.dirty"] {
        let outcome = f.update(current, None).unwrap();
        assert_eq!(outcome.release, "0.2.0");
        assert!(!outcome.replaced, "{current}");
    }
    // 0.10.0 is newer than 0.9.0, which a string comparison gets wrong.
    publish(&f.releases(), "0.10.0", &f.sk, "echo yuanme 0.10.0");
    latest(&f.releases(), "0.10.0");
    assert!(f.update("0.9.0", None).unwrap().replaced);
    assert_eq!(f.runs(), "yuanme 0.10.0");
}

#[test]
fn an_explicit_version_may_downgrade() {
    let f = Fixture::new("0.2.0");
    let outcome = f.update("0.2.0", Some("0.1.0")).unwrap();
    assert_eq!(outcome.release, "0.1.0");
    assert!(outcome.replaced);
    assert_eq!(f.runs(), "yuanme 0.1.0");
    assert_eq!(f.receipt_json()["version"], "0.1.0");
    assert_eq!(f.files(), ["yuanme"]);
}

/// Each way a release can be wrong stops the update with an error and
/// leaves the install as it was.
#[test]
fn a_tampered_release_changes_nothing() {
    type Tamper = fn(&Fixture, &Path);
    let tampers: [(&str, &str, Tamper); 5] = [
        (
            "an archive changed after it was signed",
            "SHA-256",
            |_, dir| {
                let name = format!("yuanme-{TARGET}.tar.xz");
                fs::write(dir.join(name), archive(dir, "echo yuanme 0.2.0; evil")).unwrap();
            },
        ),
        (
            "sha256.txt changed after it was signed",
            "signature",
            |_, dir| {
                let name = format!("yuanme-{TARGET}.tar.xz");
                let bytes = archive(dir, "echo yuanme 0.2.0; evil");
                let sums = fs::read_to_string(dir.join("sha256.txt")).unwrap();
                let line = sums.lines().find(|l| l.ends_with(&name)).unwrap();
                let sums = sums.replace(line, &format!("{}  {name}", sha256(&bytes)));
                fs::write(dir.join("sha256.txt"), sums).unwrap();
                fs::write(dir.join(name), bytes).unwrap();
            },
        ),
        (
            "a release signed with another key",
            "signature",
            |_, dir| sign(dir, &keys().1, "yuanme v0.2.0"),
        ),
        (
            "an older release's signed sha256.txt, served as this one's",
            "yuanme v0.1.0",
            |f, dir| {
                let old = f.releases().join("download/v0.1.0");
                for file in [
                    "sha256.txt",
                    "sha256.txt.minisig",
                    &format!("yuanme-{TARGET}.tar.xz"),
                ] {
                    fs::copy(old.join(file), dir.join(file)).unwrap();
                }
            },
        ),
        (
            "a signed binary that doesn't run",
            "doesn't run",
            |f, dir| {
                fs::remove_dir_all(dir).unwrap();
                publish(&f.releases(), "0.2.0", &f.sk, "exit 3");
            },
        ),
    ];
    for (what, says, tamper) in tampers {
        let f = Fixture::new("0.1.0");
        tamper(&f, &f.releases().join("download/v0.2.0"));
        let receipt = fs::read(f.receipt()).unwrap();
        let error = message(f.update("0.1.0", Some("0.2.0")).err().unwrap());
        assert!(error.starts_with("failed: "), "{what}: {error}");
        assert!(error.contains(says), "{what}: {error}");
        assert_eq!(f.runs(), "yuanme 0.1.0", "{what}");
        assert_eq!(fs::read(f.receipt()).unwrap(), receipt, "{what}");
        assert_eq!(f.files(), ["yuanme"], "{what}");
    }
}

#[test]
fn only_an_install_the_install_script_made_updates_itself() {
    let f = Fixture::new("0.1.0");
    let binary = f.prefix().join("yuanme");
    assert!(Install::find(&binary, &[f.receipt()]).is_ok());

    // cargo install, or Homebrew: no receipt.
    let none = f.dir.path().join("nowhere/yuanme-receipt.json");
    let error = message(Install::find(&binary, &[none]).err().unwrap());
    assert!(error.starts_with("bad command: "), "{error}");
    assert!(error.contains("install script"), "{error}");

    // A receipt, but this binary is another install's.
    let cargo = f.dir.path().join("home/.cargo/bin");
    fs::create_dir_all(&cargo).unwrap();
    let other = script(&cargo, "echo yuanme 0.1.0");
    let error = message(Install::find(&other, &[f.receipt()]).err().unwrap());
    assert!(error.starts_with("bad command: "), "{error}");
    assert!(error.contains(&f.prefix().display().to_string()), "{error}");

    // dist's cargo-home layout keeps binaries in the prefix's bin/.
    let mut receipt = f.receipt_json();
    receipt["install_layout"] = json!("cargo-home");
    receipt["install_prefix"] = json!(f.dir.path().join("home/.cargo"));
    fs::write(f.receipt(), receipt.to_string()).unwrap();
    assert!(Install::find(&other, &[f.receipt()]).is_ok());
}

#[cfg(minisign_key)]
#[test]
fn the_release_key_decodes() {
    PublicKey::decode(KEY.unwrap()).unwrap();
}
