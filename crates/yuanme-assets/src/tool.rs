//! Managed tools (ADR 0003): programs the Engine runs beside itself, such
//! as the Tailwind CLI. Each is downloaded on first use at a pinned
//! version, checked against a SHA-256, and kept in a per-user cache. An
//! environment variable can name a local binary instead, for offline
//! builds and CI.
//!
//! Downloads go through `curl`, which macOS, Windows 10 and later, and
//! CI runners ship. A Rust HTTP client would bring a TLS stack and its
//! root certificates into the Engine for one download per release.

#[cfg(test)]
mod tests;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest, Sha256 as Hasher};

pub struct Tool {
    /// As messages name it, such as `Tailwind CSS`.
    pub name: &'static str,
    /// The binary's file name in the cache, such as `tailwindcss`.
    pub file: String,
    pub version: String,
    /// The environment variable that names a local binary to run instead.
    pub env: &'static str,
    /// Where this platform's binary is downloaded from.
    pub url: String,
    pub sha256: Sha256,
}

/// What the download must hash to.
pub enum Sha256 {
    /// In hex: the Engine embeds it for the version it pins.
    Known(String),
    /// Listed beside the binary's file name in the checksum file at this
    /// URL, as `sha256sum` writes one: for a version the Author chose.
    Listed(String),
}

#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub help: String,
}

impl Tool {
    /// The binary to run: the one the environment variable names, or else
    /// the cached download, downloaded first if need be. `cache` is the
    /// folder of every Managed tool, such as [`cache_dir`].
    pub fn binary(&self, cache: &Path) -> Result<PathBuf, Error> {
        if let Some(path) = std::env::var_os(self.env) {
            let path = PathBuf::from(path);
            if !path.is_file() {
                return Err(Error {
                    message: format!(
                        "`{}` names `{}`, which is no file",
                        self.env,
                        path.display()
                    ),
                    help: format!(
                        "point `{}` at a {} binary, or unset it",
                        self.env, self.name
                    ),
                });
            }
            return Ok(path);
        }
        let dir = cache.join(format!("{}-{}", self.file, self.version));
        let binary = dir.join(&self.file);
        if binary.is_file() {
            return Ok(binary);
        }
        let offline = |message: String| Error {
            message,
            help: format!(
                "yuanme downloads {} once per version; connect to the internet, or set `{}` to a local binary",
                self.name, self.env
            ),
        };
        let want = match &self.sha256 {
            Sha256::Known(hex) => hex.clone(),
            Sha256::Listed(url) => {
                let list = fetch(url).map_err(|e| {
                    offline(format!(
                        "can't download {}'s checksums from {url}: {e}",
                        self.name
                    ))
                })?;
                let list = String::from_utf8_lossy(&list);
                let asset = self.url.rsplit('/').next().unwrap_or(&self.url);
                listed(&list, asset).ok_or_else(|| Error {
                    message: format!("{url} lists no checksum for `{asset}`"),
                    help: format!(
                        "{} {} may not exist for this platform",
                        self.name, self.version
                    ),
                })?
            }
        };
        let bytes = fetch(&self.url).map_err(|e| {
            offline(format!(
                "can't download {} {} from {}: {e}",
                self.name, self.version, self.url
            ))
        })?;
        let have = hex(&Hasher::digest(&bytes));
        if !have.eq_ignore_ascii_case(&want) {
            return Err(Error {
                message: format!(
                    "the download of {} has SHA-256 {have}, not {want}",
                    self.url
                ),
                help: "the file was changed or cut short on the way; build again, and report it if it persists".into(),
            });
        }
        install(&bytes, &binary).map_err(|e| Error {
            message: format!("can't write {} to {}: {e}", self.name, binary.display()),
            help: format!(
                "make the folder writable, or set `{}` to a local binary",
                self.env
            ),
        })?;
        Ok(binary)
    }
}

/// The folder Managed tools are cached in, per user: the platform's cache
/// folder's `yuanme/tools`.
pub fn cache_dir() -> Option<PathBuf> {
    let var = |name| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if cfg!(windows) {
        var("LOCALAPPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|home| home.join("Library/Caches"))
    } else {
        var("XDG_CACHE_HOME").or_else(|| var("HOME").map(|home| home.join(".cache")))
    };
    Some(base?.join("yuanme").join("tools"))
}

/// The bytes at `url`, or what went wrong as curl says it.
fn fetch(url: &str) -> Result<Vec<u8>, String> {
    let output = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", url])
        .output();
    match output {
        Ok(output) if output.status.success() => Ok(output.stdout),
        Ok(output) => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            Err("`curl` isn't installed, and yuanme downloads with it".into())
        }
        Err(e) => Err(format!("can't run `curl`: {e}")),
    }
}

/// The checksum `list` gives `file`. Tailwind's names each file as
/// `./<file>`.
fn listed(list: &str, file: &str) -> Option<String> {
    list.lines().find_map(|line| {
        let (hash, name) = line.split_once(char::is_whitespace)?;
        let name = name.trim_start().trim_start_matches('*');
        (name.trim_start_matches("./") == file).then(|| hash.to_string())
    })
}

/// Written beside `to` and renamed, so a build running at the same time
/// never runs half a binary.
fn install(bytes: &[u8], to: &Path) -> io::Result<()> {
    let dir = to.parent().expect("in its version's folder");
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".download-{}", std::process::id()));
    fs::write(&tmp, bytes)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o755))?;
    }
    fs::rename(&tmp, to)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
