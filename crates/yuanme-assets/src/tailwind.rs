//! Tailwind (#20): the official standalone CLI, run as a Managed tool. It
//! never scans the project: the Engine imports Tailwind with
//! `source(none)` and points `@source` at a token file it writes from
//! what the Site uses.

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use crate::tool::{Sha256, Tool};

/// The release this Engine runs unless `[tailwind] version` says otherwise.
pub const VERSION: &str = "4.3.3";

/// The environment variable naming a local Tailwind CLI to run instead.
pub const ENV: &str = "YUANME_TAILWIND_BIN";

/// [`VERSION`]'s `sha256sums.txt`.
const SHA256S: &[(&str, &str)] = &[
    (
        "tailwindcss-linux-arm64",
        "55fd0b241214eff3de1e8ee4f22796662f2d2e7a49bcfca7477cfd0bac398195",
    ),
    (
        "tailwindcss-linux-arm64-musl",
        "71ea4be79c9de9827545682df3e040053fb535d37c71ed2cfdedf9385a0868e0",
    ),
    (
        "tailwindcss-linux-x64",
        "dc61b3ac6b8c9ca874c0cc4c57b2409791a64c5540404ca5f5367360babc313a",
    ),
    (
        "tailwindcss-linux-x64-musl",
        "a04d34ceacc8f52cbe8920ad846cdeb61d3d0021dba32db0d1f77c9d9fad7a6c",
    ),
    (
        "tailwindcss-macos-arm64",
        "cdf646702987a743464dff4d9c60fd4480d1c1e73dd819a9a67f1078815dce9d",
    ),
    (
        "tailwindcss-macos-x64",
        "7922e0953f2110c05976e3bf58f14e643d90427575e766b7d433f5f80cbee7e1",
    ),
    (
        "tailwindcss-windows-x64.exe",
        "e0e260ce048014e9268f6237ff18f8ccf02cef521cbd0ae04e82c2cdf7aa3955",
    ),
];

/// This platform's binary. Windows on ARM runs the x64 one under
/// emulation (#20). A musl build of the Engine runs on a musl system.
fn asset() -> Option<&'static str> {
    let musl = cfg!(target_env = "musl");
    Some(match (std::env::consts::OS, std::env::consts::ARCH, musl) {
        ("macos", "aarch64", _) => "tailwindcss-macos-arm64",
        ("macos", "x86_64", _) => "tailwindcss-macos-x64",
        ("linux", "aarch64", false) => "tailwindcss-linux-arm64",
        ("linux", "aarch64", true) => "tailwindcss-linux-arm64-musl",
        ("linux", "x86_64", false) => "tailwindcss-linux-x64",
        ("linux", "x86_64", true) => "tailwindcss-linux-x64-musl",
        ("windows", "x86_64" | "aarch64", _) => "tailwindcss-windows-x64.exe",
        _ => return None,
    })
}

/// The Managed tool for the Tailwind release `version`, or `None` on a
/// platform Tailwind publishes no binary for.
pub fn tool(version: &str) -> Option<Tool> {
    let asset = asset()?;
    let release =
        format!("https://github.com/tailwindlabs/tailwindcss/releases/download/v{version}");
    let sha256 = match SHA256S.iter().find(|(a, _)| *a == asset) {
        Some((_, hex)) if version == VERSION => Sha256::Known((*hex).to_string()),
        _ => Sha256::Listed(format!("{release}/sha256sums.txt")),
    };
    Some(Tool {
        name: "Tailwind CSS",
        file: format!("tailwindcss{}", std::env::consts::EXE_SUFFIX),
        version: version.to_string(),
        env: ENV,
        urls: vec![format!("{release}/{asset}")],
        sha256,
        archive: None,
    })
}

/// The stylesheet `css` as the CLI gets it, if it imports Tailwind: the
/// import that scans gets `source(none)`, in place of any `source()` of
/// the Author's, and `@source` names the token file at `tokens`.
pub fn input(css: &str, tokens: &Path) -> Option<String> {
    if !imported(css) {
        return None;
    }
    let mut out = String::with_capacity(css.len() + 64);
    let mut done = 0;
    for import in imports(css) {
        if ![
            "tailwindcss",
            "tailwindcss/utilities",
            "tailwindcss/utilities.css",
        ]
        .contains(&import.specifier.as_str())
        {
            continue;
        }
        out.push_str(&css[done..import.string_end]);
        out.push_str(" source(none)");
        out.push_str(&without_source(&css[import.string_end..import.end]));
        if !import.terminated {
            out.push(';');
        }
        done = import.end;
    }
    out.push_str(&css[done..]);
    let mut path = tokens.to_string_lossy().into_owned();
    if cfg!(windows) {
        path = path.replace('\\', "/");
    }
    let path = path.replace('\\', "\\\\").replace('"', "\\\"");
    out.push_str(&format!("\n@source \"{path}\";\n"));
    Some(out)
}

/// Whether the stylesheet `css` imports Tailwind, which turns it on.
pub fn imported(css: &str) -> bool {
    imports(css).iter().any(|i| is_tailwind(&i.specifier))
}

/// `tailwindcss`, or a file of it, such as `tailwindcss/theme.css`.
pub fn is_tailwind(specifier: &str) -> bool {
    specifier == "tailwindcss" || specifier.starts_with("tailwindcss/")
}

struct Import {
    specifier: String,
    /// Just past the specifier's closing quote.
    string_end: usize,
    /// Where the statement's `;` is, or the end of the text.
    end: usize,
    terminated: bool,
}

/// Each top-level `@import` of a string in `css`.
fn imports(css: &str) -> Vec<Import> {
    let bytes = css.as_bytes();
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'*') => i = comment_end(css, i),
            q @ (b'"' | b'\'') => i = string_end(bytes, i, q),
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            b'@' if depth == 0
                && css
                    .get(i + 1..i + 7)
                    .is_some_and(|w| w.eq_ignore_ascii_case("import"))
                && !bytes
                    .get(i + 7)
                    .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_') =>
            {
                i += 7;
                loop {
                    while bytes.get(i).is_some_and(u8::is_ascii_whitespace) {
                        i += 1;
                    }
                    if css[i..].starts_with("/*") {
                        i = comment_end(css, i);
                    } else {
                        break;
                    }
                }
                let Some(&q @ (b'"' | b'\'')) = bytes.get(i) else {
                    continue;
                };
                let string_end = string_end(bytes, i, q);
                let specifier = css[i + 1..string_end.saturating_sub(1).max(i + 1)].to_string();
                let (end, terminated) = statement_end(css, string_end);
                out.push(Import {
                    specifier,
                    string_end,
                    end,
                    terminated,
                });
                i = end;
            }
            _ => i += 1,
        }
    }
    out
}

/// Just past the comment starting at `i`.
fn comment_end(css: &str, i: usize) -> usize {
    css[i + 2..].find("*/").map_or(css.len(), |e| i + 2 + e + 2)
}

/// Just past the string starting with the quote `q` at `i`.
fn string_end(bytes: &[u8], mut i: usize, q: u8) -> usize {
    i += 1;
    while let Some(&b) = bytes.get(i) {
        i += 1;
        match b {
            b'\\' => i += 1,
            b'\n' => break,
            _ if b == q => break,
            _ => {}
        }
    }
    i.min(bytes.len())
}

/// Where the statement going on at `i` ends: its `;`, outside strings,
/// comments and parentheses, or the end of the text.
fn statement_end(css: &str, mut i: usize) -> (usize, bool) {
    let bytes = css.as_bytes();
    let mut parens = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'*') => i = comment_end(css, i),
            q @ (b'"' | b'\'') => i = string_end(bytes, i, q),
            b'(' => {
                parens += 1;
                i += 1;
            }
            b')' => {
                parens = parens.saturating_sub(1);
                i += 1;
            }
            b';' if parens == 0 => return (i, true),
            _ => i += 1,
        }
    }
    (bytes.len(), false)
}

/// An import's conditions without `source(…)`.
fn without_source(conditions: &str) -> String {
    let bytes = conditions.as_bytes();
    let mut out = String::new();
    let mut done = 0;
    let mut i = 0;
    while let Some(at) = conditions[i..].find("source(").map(|a| a + i) {
        let starts_word = at == 0 || bytes[at - 1].is_ascii_whitespace() || bytes[at - 1] == b')';
        i = at + "source(".len();
        if !starts_word {
            continue;
        }
        let close = paren_end(conditions, i);
        let start = conditions[..at].trim_end().len();
        out.push_str(&conditions[done..start]);
        done = close;
        i = close;
    }
    out.push_str(&conditions[done..]);
    out
}

/// Just past the `)` closing the parenthesis opened before `i`.
fn paren_end(css: &str, mut i: usize) -> usize {
    let bytes = css.as_bytes();
    let mut depth = 1usize;
    while i < bytes.len() {
        match bytes[i] {
            q @ (b'"' | b'\'') => i = string_end(bytes, i, q),
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                depth -= 1;
                i += 1;
                if depth == 0 {
                    return i;
                }
            }
            _ => i += 1,
        }
    }
    bytes.len()
}

/// The token file: every whitespace-separated token of `texts`, once each,
/// in order, one per line. Tailwind finds class candidates in any text,
/// and none holds whitespace.
pub fn tokens<'a>(texts: impl IntoIterator<Item = &'a str>) -> String {
    let tokens: BTreeSet<&str> = texts.into_iter().flat_map(str::split_whitespace).collect();
    let mut out = String::new();
    for token in tokens {
        out.push_str(token);
        out.push('\n');
    }
    out
}

#[derive(Debug)]
pub enum Error {
    /// The binary couldn't be started, or stopped answering.
    Run(String),
    /// What Tailwind says is wrong, such as `Cannot apply unknown utility
    /// class `nope``.
    Tailwind(String),
}

/// The CLI at `binary`, run from `dir`, with `tmp` as its temporary
/// folder: Bun, which builds the CLI, writes lightningcss's native module
/// there to load it, and two CLIs sharing a folder made one fail to load
/// it.
fn command(binary: &Path, dir: &Path, tmp: &Path) -> Command {
    let mut command = Command::new(binary);
    command
        .current_dir(dir)
        .env("TMPDIR", tmp)
        .env("TEMP", tmp)
        .env("TMP", tmp)
        // picocolors still dims some text without a terminal.
        .env("NO_COLOR", "1")
        .stdin(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

/// Runs the CLI at `binary` once on `input`, as [`input`] makes it, from
/// the folder `dir` of the stylesheet it was, so that its relative
/// imports and plugins resolve. `tmp` is a folder of its own. The CSS is
/// unminified.
pub fn run(binary: &Path, dir: &Path, input: &str, tmp: &Path) -> Result<String, Error> {
    let mut child = command(binary, dir, tmp)
        .args(["--input", "-", "--silent"])
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| Error::Run(format!("can't run `{}`: {e}", binary.display())))?;
    let mut stdin = child.stdin.take().expect("piped");
    // Written beside the read of its output, so neither pipe fills up.
    let input = input.to_string();
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
    let output = child
        .wait_with_output()
        .map_err(|e| Error::Run(format!("`{}` failed: {e}", binary.display())))?;
    // A write cut off because Tailwind exited early fails with its error.
    let _ = writer.join();
    if !output.status.success() {
        return Err(Error::Tailwind(message(&String::from_utf8_lossy(
            &output.stderr,
        ))));
    }
    String::from_utf8(output.stdout)
        .map_err(|_| Error::Run(format!("`{}` wrote CSS that isn't UTF-8", binary.display())))
}

/// The message of an error Tailwind printed as
/// `Error:\n┌\n│ Error: <message>\n└`, or else all it printed.
fn message(stderr: &str) -> String {
    let mut lines = Vec::new();
    for part in stderr.split(['\n', '\r']) {
        if let Some(text) = plain(part).strip_prefix("\u{2502} ") {
            lines.push(text.trim_end().to_string());
        }
    }
    if lines.is_empty() {
        return plain(stderr).trim().to_string();
    }
    let text = lines.join("\n");
    text.strip_prefix("Error: ").unwrap_or(&text).to_string()
}

/// `text` without its ANSI escapes.
fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// A long-lived CLI for `yuanme dev` (#20): `--watch=always --poll=50`,
/// which notices a rewritten token file in 15–31 ms, against about 150 ms
/// for a fresh process. Like the CLI's own watch, its CSS keeps classes
/// whose tokens went away until it is started again.
pub struct Watch {
    child: Child,
    tokens: PathBuf,
    output: PathBuf,
    written: String,
    /// One message per build: `Some` with Tailwind's error for a failed one.
    builds: Receiver<Option<String>>,
}

/// How long a build may take before the CLI counts as hung.
const BUILD_TIMEOUT: Duration = Duration::from_secs(30);

impl Watch {
    /// Starts the CLI at `binary` from `dir` on `input`, as [`input`] makes
    /// it naming the file `tokens`, which this writes with `first`, and
    /// returns its first CSS. It writes each build to `output`. The folder
    /// of `tokens` is its own, and its temporary folder too.
    pub fn start(
        binary: &Path,
        dir: &Path,
        input: &str,
        tokens: PathBuf,
        first: &str,
        output: PathBuf,
    ) -> Result<(Watch, String), Error> {
        write_atomic(&tokens, first)?;
        let tmp = tokens.parent().expect("a file is in a folder");
        let mut child = command(binary, dir, tmp)
            .arg("--input")
            .arg("-")
            .arg("--output")
            .arg(&output)
            .args(["--watch=always", "--poll=50"])
            .stdout(Stdio::null())
            .spawn()
            .map_err(|e| Error::Run(format!("can't run `{}`: {e}", binary.display())))?;
        let mut stdin = child.stdin.take().expect("piped");
        let written = stdin.write_all(input.as_bytes());
        // `--watch=always` keeps watching once its input has ended.
        drop(stdin);
        written.map_err(|e| Error::Run(format!("can't write to Tailwind: {e}")))?;
        let stderr = child.stderr.take().expect("piped");
        let (send, builds) = mpsc::channel();
        std::thread::spawn(move || {
            // What it printed since the last build ended.
            let mut printed = String::new();
            for line in BufReader::new(stderr).split(b'\n') {
                let Ok(line) = line else { break };
                let line = String::from_utf8_lossy(&line);
                let done = line.split('\r').any(|p| plain(p).starts_with("Done in"));
                printed.push_str(&line);
                printed.push('\n');
                if done {
                    let failed = printed.split(['\n', '\r']).any(|p| plain(p) == "Error:");
                    let error = failed.then(|| message(&printed));
                    printed.clear();
                    if send.send(error).is_err() {
                        return;
                    }
                }
            }
        });
        let mut watch = Watch {
            child,
            tokens,
            output,
            written: first.to_string(),
            builds,
        };
        let css = watch.next()?;
        Ok((watch, css))
    }

    /// Rewrites the token file with `tokens` if they changed, and returns
    /// the CSS once Tailwind has built it; unchanged tokens return the
    /// last CSS without waiting for Tailwind.
    pub fn update(&mut self, tokens: &str) -> Result<String, Error> {
        if tokens == self.written {
            return self.css();
        }
        write_atomic(&self.tokens, tokens)?;
        self.written = tokens.to_string();
        self.next()
    }

    fn next(&mut self) -> Result<String, Error> {
        match self.builds.recv_timeout(BUILD_TIMEOUT) {
            Ok(None) => self.css(),
            Ok(Some(error)) => Err(Error::Tailwind(error)),
            Err(_) => Err(Error::Run(match self.child.try_wait() {
                Ok(Some(status)) => format!("Tailwind stopped: {status}"),
                _ => format!("Tailwind built nothing in {} s", BUILD_TIMEOUT.as_secs()),
            })),
        }
    }

    fn css(&self) -> Result<String, Error> {
        std::fs::read_to_string(&self.output)
            .map_err(|e| Error::Run(format!("can't read {}: {e}", self.output.display())))
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        // It may have exited already, which leaves nothing to stop.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Written whole and renamed, so a poll never reads half a file.
fn write_atomic(path: &Path, text: &str) -> Result<(), Error> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)
        .and_then(|()| std::fs::rename(&tmp, path))
        .map_err(|e: io::Error| Error::Run(format!("can't write {}: {e}", path.display())))
}
