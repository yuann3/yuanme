//! `yuanme self update` (#37): replaces an install the install script made
//! with another release. A release's `sha256.txt` must carry a minisign
//! signature from the key built into the binary, with the trusted comment
//! `yuanme vX.Y.Z`, and the archive must hash to what it lists. The old
//! binary stays in place until the new one has run.

#[cfg(all(test, unix))]
mod tests;

use std::env;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use minisign_verify::{PublicKey, Signature};
use serde_json::Value;
use yuanme_assets::tool;

use crate::events::Event;
use crate::out::Out;
use crate::{ENGINE_VERSION, Error};

const RELEASES: &str = "https://github.com/yuann3/yuanme/releases";

/// The key release-sign.yml signs with. A release binary trusts it and no
/// other: only tests pass another to [`update`].
#[cfg(minisign_key)]
const KEY: Option<&str> = Some(include_str!("../minisign.pub"));
#[cfg(not(minisign_key))]
const KEY: Option<&str> = None;

/// Where releases are published, and the key that signs them.
pub(crate) struct Source<'a> {
    /// The repository's `/releases`, under which GitHub serves
    /// `download/vX.Y.Z/FILE` and the newest release's
    /// `latest/download/FILE`.
    pub(crate) url: &'a str,
    /// A minisign public key file. `None` in a build from before the
    /// release key was committed.
    pub(crate) key: Option<&'a str>,
}

/// An install the install script made, as its receipt records it.
pub(crate) struct Install {
    binary: PathBuf,
    receipt: PathBuf,
    json: Value,
}

pub(crate) struct Outcome {
    /// The release it looked at: the newest, or the one named.
    pub(crate) release: String,
    pub(crate) replaced: bool,
}

/// `yuanme self update [VERSION]`, or with `check`, `--check`.
pub(crate) fn run(version: Option<String>, check: bool, out: &Out) -> Result<u8, Error> {
    let source = Source {
        url: RELEASES,
        key: KEY,
    };
    let outcome = if check {
        Outcome {
            release: newest(&source)?,
            replaced: false,
        }
    } else {
        let exe = env::current_exe()?;
        let install = Install::find(&exe, &receipts())?;
        update(
            &source,
            &install,
            ENGINE_VERSION,
            version.as_deref(),
            env!("YUANME_TARGET"),
            out,
        )?
    };
    let release = &outcome.release;
    let newer = newer(release, ENGINE_VERSION);
    out.event(Event::SelfUpdate {
        current: ENGINE_VERSION,
        release: release.clone(),
        newer,
        updated: outcome.replaced,
    });
    out.say(&match (outcome.replaced, check, newer) {
        (true, _, _) => format!(
            "updated yuanme {ENGINE_VERSION} to {release}\nnote: a deploy workflow `yuanme init --host` wrote installs the release its `YUANME_VERSION` names; set it to {release} to build with this one"
        ),
        (false, true, true) => format!(
            "yuanme {release} is out, and this is {ENGINE_VERSION}: `yuanme self update` installs it"
        ),
        (false, _, _) if version.is_some() => format!("yuanme {release} is installed already"),
        (false, _, _) => {
            format!("yuanme {release} is the newest release, and this is {ENGINE_VERSION}")
        }
    });
    Ok(0)
}

/// clap's check of `VERSION`: an exact release.
pub(crate) fn version(arg: &str) -> Result<String, NotARelease> {
    match parse(arg) {
        Some(_) if !arg.contains('+') => Ok(arg.to_string()),
        _ => Err(NotARelease),
    }
}

/// A `VERSION` that isn't X.Y.Z, which clap reports.
#[derive(Debug)]
pub(crate) struct NotARelease;

impl fmt::Display for NotARelease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a release is X.Y.Z, such as 0.2.0")
    }
}

impl std::error::Error for NotARelease {}

/// The newest release's version, from its signed `sha256.txt`.
pub(crate) fn newest(source: &Source) -> Result<String, Error> {
    Ok(release(source, None)?.0)
}

/// Replaces the install's binary with the release `version` names, or
/// with the newest when it is newer than `current`, and records it in the
/// receipt. `target` picks the archive.
pub(crate) fn update(
    source: &Source,
    install: &Install,
    current: &str,
    version: Option<&str>,
    target: &str,
    out: &Out,
) -> Result<Outcome, Error> {
    let (release, sums) = release(source, version)?;
    let wanted = match version {
        Some(_) => parse(&release) != parse(current) || current.contains('+'),
        None => newer(&release, current),
    };
    if !wanted {
        return Ok(Outcome {
            release,
            replaced: false,
        });
    }
    let archive = match target {
        "x86_64-pc-windows-msvc" | "aarch64-pc-windows-msvc" => format!("yuanme-{target}.zip"),
        _ => format!("yuanme-{target}.tar.xz"),
    };
    let failed = |message: String| Error::Io(io::Error::other(message));
    let Some(want) = tool::listed(&sums, &archive) else {
        return Err(failed(format!(
            "yuanme {release} has no `{archive}`: there is no release for {target}"
        )));
    };
    let url = format!("{}/download/v{release}/{archive}", source.url);
    out.say(&format!("downloading {url}"));
    let bytes = tool::fetch(&url).map_err(|e| failed(format!("can't download {url}: {e}")))?;
    let have = tool::sha256(&bytes);
    if !have.eq_ignore_ascii_case(&want) {
        return Err(failed(format!(
            "{url} has SHA-256 {have}, not the {want} its signed sha256.txt lists"
        )));
    }
    let dir = install.binary.parent().expect("a binary is in a folder");
    let tmp = dir.join(format!(".yuanme-update-{}", std::process::id()));
    let replaced = fs::create_dir(&tmp)
        .and_then(|()| replace(&bytes, &archive, target, &tmp, &install.binary));
    match fs::remove_dir_all(&tmp) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => {
            eprintln!("warning: could not delete {}: {e}", tmp.display());
        }
        _ => {}
    }
    replaced.map_err(|e| failed(format!("can't install yuanme {release}: {e}")))?;
    let mut json = install.json.clone();
    json["version"] = Value::String(release.clone());
    write(&install.receipt, json.to_string().as_bytes()).map_err(|e| {
        failed(format!(
            "updated yuanme to {release}, but can't record it in {}: {e}",
            install.receipt.display()
        ))
    })?;
    Ok(Outcome {
        release,
        replaced: true,
    })
}

impl Install {
    /// The install `exe`, the running binary, belongs to, from the first
    /// of `receipts` that exists.
    pub(crate) fn find(exe: &Path, receipts: &[PathBuf]) -> Result<Install, Error> {
        let own_tool = "update it the way you installed it, such as with `brew upgrade yuanme` or `cargo install --locked yuanme`";
        let Some((receipt, text)) = receipts
            .iter()
            .find_map(|r| Some((r, fs::read_to_string(r).ok()?)))
        else {
            return Err(Error::BadCommand(format!(
                "only an install made by yuanme's install script can update itself, and it wrote no receipt: {own_tool}"
            )));
        };
        let json: Value = serde_json::from_str(&text).unwrap_or_default();
        let prefix = json["install_prefix"].as_str().map(PathBuf::from);
        let ours =
            json["provider"]["source"] == "cargo-dist" && json["source"]["app_name"] == "yuanme";
        let (Some(prefix), true) = (prefix, ours) else {
            return Err(Error::BadCommand(format!(
                "{} isn't an install receipt yuanme's install script wrote: {own_tool}",
                receipt.display()
            )));
        };
        let binary = exe.canonicalize()?;
        let installed = [prefix.clone(), prefix.join("bin")]
            .iter()
            .filter_map(|d| d.canonicalize().ok())
            .any(|d| Some(d.as_path()) == binary.parent());
        if !installed {
            return Err(Error::BadCommand(format!(
                "this yuanme is {}, but the install script installed yuanme in {}: {own_tool}",
                binary.display(),
                prefix.display()
            )));
        }
        Ok(Install {
            binary,
            receipt: receipt.clone(),
            json,
        })
    }
}

/// Where dist's installers write the receipt, in the order they look.
fn receipts() -> Vec<PathBuf> {
    let var = |name| {
        env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let home = match cfg!(windows) {
        true => var("LOCALAPPDATA"),
        false => var("HOME").map(|h| h.join(".config")),
    };
    [var("XDG_CONFIG_HOME"), home]
        .into_iter()
        .flatten()
        .map(|d| d.join("yuanme/yuanme-receipt.json"))
        .collect()
}

/// The release `version` names, or the newest, and its `sha256.txt`,
/// once its signature is checked.
fn release(source: &Source, version: Option<&str>) -> Result<(String, String), Error> {
    let Some(key) = source.key else {
        return Err(Error::BadCommand(format!(
            "this yuanme was built without the key that signs releases, so it can't check one: install a release with the install script, from {RELEASES}"
        )));
    };
    let key = PublicKey::decode(key).expect("the release key decodes");
    let failed = |message: String| Error::Io(io::Error::other(message));
    let dir = match version {
        Some(v) => format!("{}/download/v{v}", source.url),
        None => format!("{}/latest/download", source.url),
    };
    let get = |file: &str| {
        let url = format!("{dir}/{file}");
        tool::fetch(&url).map_err(|e| failed(format!("can't download {url}: {e}")))
    };
    let sums = get("sha256.txt")?;
    let signature = get("sha256.txt.minisig")?;
    let unsigned = |e: &dyn fmt::Display| {
        failed(format!(
            "{dir}/sha256.txt doesn't carry a valid signature from yuanme's release key: {e}"
        ))
    };
    let signature = std::str::from_utf8(&signature).map_err(|e| unsigned(&e))?;
    let signature = Signature::decode(signature).map_err(|e| unsigned(&e))?;
    key.verify(&sums, &signature, false)
        .map_err(|e| unsigned(&e))?;
    // Signed, but maybe for another release, served to make this one look
    // older or newer than it is.
    let comment = signature.trusted_comment();
    let signed = comment
        .strip_prefix("yuanme v")
        .filter(|v| version.is_none_or(|want| want == *v) && self::version(v).is_ok());
    let Some(signed) = signed else {
        return Err(failed(format!(
            "{dir}/sha256.txt is signed for `{comment}`, not yuanme v{}",
            version.unwrap_or("X.Y.Z")
        )));
    };
    let sums =
        String::from_utf8(sums).map_err(|_| failed(format!("{dir}/sha256.txt isn't text")))?;
    Ok((signed.to_string(), sums))
}

/// Unpacks the binary from `archive`'s `bytes` into `tmp`, runs it, and
/// only then moves it over `binary`.
fn replace(bytes: &[u8], archive: &str, target: &str, tmp: &Path, binary: &Path) -> io::Result<()> {
    // dist's tarballs hold one folder; its zips hold the files.
    let member = match archive.ends_with(".zip") {
        true => "yuanme.exe".to_string(),
        false => format!("yuanme-{target}/yuanme"),
    };
    fs::write(tmp.join(archive), bytes)?;
    // Windows 10's own tar unpacks zips; another on PATH may not.
    let tar = match env::var_os("SystemRoot") {
        Some(root) if cfg!(windows) => Path::new(&root).join("System32/tar.exe"),
        _ => PathBuf::from("tar"),
    };
    let output = Command::new(&tar)
        .arg("-xf")
        .arg(tmp.join(archive))
        .arg("-C")
        .arg(tmp)
        .arg(&member)
        .output()
        .map_err(|e| io::Error::other(format!("can't run `{}`: {e}", tar.display())))?;
    if !output.status.success() {
        return Err(io::Error::other(format!(
            "`tar` can't unpack {archive}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let new = tmp.join(member);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&new, fs::Permissions::from_mode(0o755))?;
    }
    match Command::new(&new).arg("--version").output() {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            return Err(io::Error::other(format!(
                "it doesn't run: `yuanme --version` exited with {}",
                o.status
            )));
        }
        Err(e) => return Err(io::Error::other(format!("it doesn't run: {e}"))),
    }
    if !cfg!(windows) {
        return fs::rename(&new, binary);
    }
    // A running .exe can be renamed but not replaced. The old one is
    // deleted by the next update, once it no longer runs.
    let old = binary.with_extension("exe.old");
    match fs::remove_file(&old) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => {
            eprintln!("warning: could not delete {}: {e}", old.display())
        }
        _ => {}
    }
    fs::rename(binary, &old)?;
    if let Err(e) = fs::rename(&new, binary) {
        fs::rename(&old, binary)?;
        return Err(e);
    }
    Ok(())
}

/// Written beside `path` and renamed, so it is never half written.
fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

/// `X.Y.Z` of a version, leaving out what a checkout's build adds after
/// `+`.
fn parse(version: &str) -> Option<[u64; 3]> {
    let core = version.split('+').next()?;
    let mut parts = core
        .split('.')
        .map(|p| match p.bytes().all(|b| b.is_ascii_digit()) {
            true => p.parse().ok(),
            false => None,
        });
    let parsed = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(parsed)
}

/// Whether `release` is newer than `current`.
fn newer(release: &str, current: &str) -> bool {
    parse(release) > parse(current)
}
