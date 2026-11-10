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

use std::fmt;
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
    /// Where this platform's download is, in the order they are tried:
    /// each after the first serves the same file, for when those before
    /// it fail.
    pub urls: Vec<String>,
    pub sha256: Sha256,
    /// For a download that is a `.tgz`, the folder in it holding the
    /// binary and the files it needs beside it, such as `package/lib`,
    /// which is what the cache keeps. `None` for one that is the binary.
    pub archive: Option<&'static str>,
}

/// What the download must hash to.
pub enum Sha256 {
    /// In hex: the Engine embeds it for the version it pins.
    Known(String),
    /// Listed beside the binary's file name in the checksum file at this
    /// URL, as `sha256sum` writes one: for a version the Author chose.
    Listed(String),
}

/// Why there is no binary for a Managed tool. Each says what to do in
/// [`Error::help`].
#[derive(Debug)]
pub enum Error {
    /// The environment variable names `path`, which is no file.
    NotAFile {
        env: &'static str,
        name: &'static str,
        path: PathBuf,
    },
    /// The checksum file at `url` couldn't be downloaded.
    Checksums {
        tool: Named,
        url: String,
        error: FetchError,
    },
    /// The checksum file at `url` has no line for `asset`.
    NotListed {
        tool: Named,
        url: String,
        asset: String,
    },
    /// Every URL failed, each with its error.
    Download {
        tool: Named,
        failures: Vec<(String, FetchError)>,
    },
    /// The download from `url` hashed to `have`.
    Checksum {
        url: String,
        have: String,
        want: String,
    },
    /// The download couldn't be installed at `binary`.
    Install {
        tool: Named,
        binary: PathBuf,
        error: io::Error,
    },
}

/// A tool as its errors name it.
#[derive(Debug)]
pub struct Named {
    pub name: &'static str,
    pub version: String,
    pub env: &'static str,
}

impl Error {
    /// What the Author can do about it.
    pub fn help(&self) -> String {
        let offline = |tool: &Named| {
            format!(
                "yuanme downloads {} once per version; connect to the internet, or set `{}` to a local binary",
                tool.name, tool.env
            )
        };
        match self {
            Error::NotAFile { env, name, .. } => {
                format!("point `{env}` at a {name} binary, or unset it")
            }
            Error::Checksums { tool, .. } | Error::Download { tool, .. } => offline(tool),
            Error::NotListed { tool, .. } => {
                format!("{} {} may not exist for this platform", tool.name, tool.version)
            }
            Error::Checksum { .. } => "the file was changed or cut short on the way; build again, and report it if it persists".into(),
            Error::Install { tool, .. } => format!(
                "make the folder writable, or set `{}` to a local binary",
                tool.env
            ),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotAFile { env, path, .. } => {
                write!(f, "`{env}` names `{}`, which is no file", path.display())
            }
            Error::Checksums { tool, url, error } => write!(
                f,
                "can't download {}'s checksums from {url}: {error}",
                tool.name
            ),
            Error::NotListed { url, asset, .. } => {
                write!(f, "{url} lists no checksum for `{asset}`")
            }
            Error::Download { tool, failures } => {
                write!(f, "can't download {} {} ", tool.name, tool.version)?;
                for (i, (url, error)) in failures.iter().enumerate() {
                    let or = if i > 0 { ", or " } else { "" };
                    write!(f, "{or}from {url}: {error}")?;
                }
                Ok(())
            }
            Error::Checksum { url, have, want } => {
                write!(f, "the download of {url} has SHA-256 {have}, not {want}")
            }
            Error::Install {
                tool,
                binary,
                error,
            } => write!(
                f,
                "can't write {} to {}: {error}",
                tool.name,
                binary.display()
            ),
        }
    }
}

impl std::error::Error for Error {}

impl Tool {
    /// The binary to run: the one the environment variable names, or else
    /// the cached download, downloaded first if need be. `cache` is the
    /// folder of every Managed tool, such as [`cache_dir`].
    pub fn binary(&self, cache: &Path) -> Result<PathBuf, Error> {
        if let Some(path) = std::env::var_os(self.env) {
            let path = PathBuf::from(path);
            if !path.is_file() {
                return Err(Error::NotAFile {
                    env: self.env,
                    name: self.name,
                    path,
                });
            }
            return Ok(path);
        }
        let dir = cache.join(format!("{}-{}", self.file, self.version));
        let binary = dir.join(&self.file);
        if binary.is_file() {
            return Ok(binary);
        }
        let want = match &self.sha256 {
            Sha256::Known(hex) => hex.clone(),
            Sha256::Listed(url) => {
                let list = fetch(url).map_err(|error| Error::Checksums {
                    tool: self.named(),
                    url: url.clone(),
                    error,
                })?;
                let list = String::from_utf8_lossy(&list);
                let first = &self.urls[0];
                let asset = first.rsplit('/').next().unwrap_or(first);
                listed(&list, asset).ok_or_else(|| Error::NotListed {
                    tool: self.named(),
                    url: url.clone(),
                    asset: asset.to_string(),
                })?
            }
        };
        let mut failures = Vec::new();
        let (url, bytes) = loop {
            let Some(url) = self.urls.get(failures.len()) else {
                return Err(Error::Download {
                    tool: self.named(),
                    failures,
                });
            };
            match fetch(url) {
                Ok(bytes) => break (url, bytes),
                Err(e) => {
                    if let Some(next) = self.urls.get(failures.len() + 1) {
                        eprintln!(
                            "warning: can't download {} {} from {url}: {e}; trying {next}",
                            self.name, self.version
                        );
                    }
                    failures.push((url.clone(), e));
                }
            }
        };
        let have = sha256(&bytes);
        if !have.eq_ignore_ascii_case(&want) {
            return Err(Error::Checksum {
                url: url.clone(),
                have,
                want,
            });
        }
        let installed = match self.archive {
            Some(folder) => unpack(&bytes, folder, &dir).and_then(|()| {
                if binary.is_file() {
                    Ok(())
                } else {
                    let message = format!("{url} has no `{folder}/{}`", self.file);
                    Err(io::Error::new(io::ErrorKind::NotFound, message))
                }
            }),
            None => install(&bytes, &binary),
        };
        installed.map_err(|error| Error::Install {
            tool: self.named(),
            binary: binary.clone(),
            error,
        })?;
        Ok(binary)
    }

    fn named(&self) -> Named {
        Named {
            name: self.name,
            version: self.version.clone(),
            env: self.env,
        }
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

/// Why `curl` couldn't download a file.
#[derive(Debug)]
pub enum FetchError {
    NoCurl,
    Run(io::Error),
    /// What curl printed.
    Curl(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::NoCurl => {
                f.write_str("`curl` isn't installed, and yuanme downloads with it")
            }
            FetchError::Run(e) => write!(f, "can't run `curl`: {e}"),
            FetchError::Curl(said) => f.write_str(said),
        }
    }
}

impl std::error::Error for FetchError {}

/// The bytes at `url`.
pub fn fetch(url: &str) -> Result<Vec<u8>, FetchError> {
    let output = Command::new("curl")
        .args(["--fail", "--location", "--silent", "--show-error", url])
        .output();
    match output {
        Ok(output) if output.status.success() => Ok(output.stdout),
        Ok(output) => Err(FetchError::Curl(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        )),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(FetchError::NoCurl),
        Err(e) => Err(FetchError::Run(e)),
    }
}

/// The checksum `list` gives `file`. Tailwind's names each file as
/// `./<file>`.
pub fn listed(list: &str, file: &str) -> Option<String> {
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

/// The folder `folder` of the `.tgz` `bytes`, unpacked with `tar` beside
/// `to` and renamed to it, for the same reason as [`install`]. `tar`
/// ships with macOS, Linux and Windows 10 and later, as curl does.
fn unpack(bytes: &[u8], folder: &str, to: &Path) -> io::Result<()> {
    let parent = to.parent().expect("in the cache folder");
    let tmp = parent.join(format!(".unpack-{}", std::process::id()));
    fs::create_dir_all(&tmp)?;
    let archive = tmp.join("download.tgz");
    let unpacked = fs::write(&archive, bytes).and_then(|()| {
        let output = Command::new("tar")
            .arg("-xzf")
            .arg(&archive)
            .arg("-C")
            .arg(&tmp)
            .arg(folder)
            .output();
        match output {
            Ok(output) if output.status.success() => match fs::rename(tmp.join(folder), to) {
                // Another build unpacked it first.
                Err(_) if to.is_dir() => Ok(()),
                renamed => renamed,
            },
            Ok(output) => Err(io::Error::other(format!(
                "`tar` can't unpack it: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err(io::Error::other(
                "`tar` isn't installed, and yuanme unpacks with it",
            )),
            Err(e) => Err(e),
        }
    });
    if let Err(e) = fs::remove_dir_all(&tmp) {
        // Left behind, it costs the download's size in the cache folder.
        eprintln!("warning: could not delete {}: {e}", tmp.display());
    }
    unpacked
}

/// `bytes`' SHA-256, in hex.
pub fn sha256(bytes: &[u8]) -> String {
    Hasher::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
