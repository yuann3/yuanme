//! Type checking (#20): TypeScript 7's native `tsc`, run as a Managed
//! tool. It is a Go binary, so it needs no Node, but it needs its
//! `lib.*.d.ts` beside it, so its download is a `.tgz` the cache keeps
//! unpacked.

use std::io;
use std::path::Path;
use std::process::Command;

use crate::tool::{Sha256, Tool};

/// The release this Engine runs.
pub const VERSION: &str = "7.0.2";

/// The environment variable naming a local `tsc` to run instead, which
/// must have its `lib.*.d.ts` beside it as the release has.
pub const ENV: &str = "YUANME_TSC_BIN";

/// The types of what the Engine gives Scripts: `yuanme:router`, the
/// Router's events, `mount`'s `ctx` and Data file imports.
pub const TYPES: &str = include_str!("js/yuanme.d.ts");

/// [`VERSION`]'s SHA-256s, which its GitHub release lists. The npm
/// registry's `@typescript/typescript-<platform>` tarballs are the same
/// files, byte for byte.
const SHA256S: &[(&str, &str)] = &[
    (
        "darwin-arm64",
        "902e2fe1cf0799198ef902c6b8c310a450fef629a6baba41d45641ef75c04ebd",
    ),
    (
        "darwin-x64",
        "eba158cb54050f723d5ff781438f33de5640054440bb4f2bd170cfe9bc2eb551",
    ),
    (
        "linux-arm64",
        "c83d931ac9dd7549cde6e71246aa9d6a9812843023df3e277fe3b5dcf41dd0ea",
    ),
    (
        "linux-x64",
        "7ecad6f67377e831856367ab062ef394f21506a611405bf8ac0ff039348637d3",
    ),
    (
        "win32-arm64",
        "0a73534e6ee50cdbb2a29ac48657ca0ad13cf0f424cf63808e4df7baeb87b8be",
    ),
    (
        "win32-x64",
        "61fc4e141d2bc687db580e71bbfa63b9c209f0310645d82ca1b457eb3a24fd19",
    ),
];

/// This platform's name in TypeScript's releases. Go builds are static,
/// so one Linux binary runs on musl too.
fn platform() -> Option<&'static str> {
    Some(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin-arm64",
        ("macos", "x86_64") => "darwin-x64",
        ("linux", "aarch64") => "linux-arm64",
        ("linux", "x86_64") => "linux-x64",
        ("windows", "aarch64") => "win32-arm64",
        ("windows", "x86_64") => "win32-x64",
        _ => return None,
    })
}

/// The pinned `tsc`, or `None` on a platform TypeScript publishes no
/// binary for. Microsoft hasn't promised to keep its GitHub release
/// layout, so the npm registry's copy is the second place to get it (#27).
pub fn tool() -> Option<Tool> {
    let platform = platform()?;
    let (_, sha256) = SHA256S.iter().find(|(p, _)| *p == platform)?;
    let package = format!("typescript-{platform}");
    Some(Tool {
        name: "TypeScript",
        file: format!("tsc{}", std::env::consts::EXE_SUFFIX),
        version: VERSION.to_string(),
        env: ENV,
        urls: vec![
            format!(
                "https://github.com/microsoft/typescript-go/releases/download/typescript%2Fv{VERSION}/{package}.tgz"
            ),
            format!("https://registry.npmjs.org/@typescript/{package}/-/{package}-{VERSION}.tgz"),
        ],
        sha256: Sha256::Known((*sha256).to_string()),
        archive: Some("package/lib"),
    })
}

/// Runs `tsc` at `binary` from the project root `root` on the tsconfig at
/// `config`, which must set `noEmit`, and returns what it prints: one
/// line per error, as [`errors`] reads them, or nothing.
pub fn run(binary: &Path, root: &Path, config: &Path) -> Result<String, String> {
    let output = Command::new(binary)
        .current_dir(root)
        .arg("--project")
        .arg(config)
        .args(["--pretty", "false"])
        .output()
        .map_err(|e: io::Error| format!("can't run `{}`: {e}", binary.display()))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    // 1 and 2 are type errors found, with or without output written.
    match output.status.code() {
        Some(0) => Ok(stdout),
        Some(1 | 2) if !errors(&stdout).is_empty() => Ok(stdout),
        _ => Err(format!(
            "`{}` failed ({}): {}{}",
            binary.display(),
            output.status,
            stdout.trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct TypeError {
    /// The file it is in, as `tsc` names it from the project root, and
    /// where: the line from 1 and the column from 1 in UTF-16 units.
    /// `None` for one about no file, such as a tsconfig it can't read.
    pub at: Option<(String, u32, u32)>,
    /// Such as `TS2322`.
    pub code: String,
    pub message: String,
    /// The lines that explain the message, outermost first, such as
    /// `Types of property 'a' are incompatible.`
    pub details: Vec<String>,
}

/// The errors in `tsc --pretty false`'s output: each is a line
/// `file(line,column): error TS1234: message`, or `error TS1234: message`
/// for none, and the indented lines after it.
pub fn errors(output: &str) -> Vec<TypeError> {
    let mut out: Vec<TypeError> = Vec::new();
    for line in output.lines() {
        if line.starts_with(char::is_whitespace) {
            if let Some(last) = out.last_mut()
                && !line.trim().is_empty()
            {
                last.details.push(line.trim().to_string());
            }
            continue;
        }
        let (before, after) = match line.find("error TS") {
            Some(i) if i == 0 || line[..i].ends_with("): ") => (&line[..i], &line[i + 6..]),
            _ => continue,
        };
        let Some((code, message)) = after.split_once(": ") else {
            continue;
        };
        let at = before
            .strip_suffix("): ")
            .and_then(|b| b.rsplit_once('('))
            .and_then(|(file, position)| {
                let (line, column) = position.split_once(',')?;
                Some((file.to_string(), line.parse().ok()?, column.parse().ok()?))
            });
        out.push(TypeError {
            at,
            code: code.to_string(),
            message: message.to_string(),
            details: Vec::new(),
        });
    }
    out
}
