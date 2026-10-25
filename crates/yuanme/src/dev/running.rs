//! The dev server running for a project (#21): `.yuanme/dev.json` says
//! where it is, and `--background`, `status`, `stop` and `logs` read it.

use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::serve::{PID, STATUS, STOP};
use crate::events::Event;
use crate::{ENGINE_VERSION, Error, Out, SITE_ERRORS};

/// From the project root.
const FILE: &str = ".yuanme/dev.json";
/// Where a server started with `--background` writes what it prints.
pub const LOG: &str = ".yuanme/dev.log";

/// A dev server that answers as the one `dev.json` names.
pub struct Running {
    /// `dev.json`.
    pub info: Value,
    pub pid: u32,
    pub url: String,
    /// `/__yuanme/status`.
    pub build: Value,
}

/// The server `dev.json` names, if it answers as that server. A file
/// whose server is gone, as one killed with SIGKILL leaves, is removed.
pub fn find(root: &Path) -> Option<Running> {
    let path = root.join(FILE);
    let text = fs::read_to_string(&path).ok()?;
    let found = (|| {
        let info: Value = serde_json::from_str(&text).ok()?;
        let pid = u32::try_from(info["pid"].as_u64()?).ok()?;
        let url = info["url"].as_str()?.to_string();
        let (status, head, body) = request(&url, "GET", STATUS, pid).ok()?;
        let answers = head.contains(&format!("{PID}: {pid}\r\n"));
        (status == 200 && answers).then_some(())?;
        let build = serde_json::from_str(&body).ok()?;
        Some(Running {
            info,
            pid,
            url,
            build,
        })
    })();
    // Unless another server wrote its own since.
    if found.is_none() && fs::read_to_string(&path).is_ok_and(|now| now == text) {
        let _ = fs::remove_file(&path);
    }
    found
}

/// Writes `dev.json` for this process, serving at `url`.
pub fn record(root: &Path, url: &str, port: u16, log: Option<&str>) -> io::Result<()> {
    let info = json!({
        "pid": std::process::id(),
        "port": port,
        "url": url,
        "version": ENGINE_VERSION,
        "started": jiff::Timestamp::now().to_string(),
        "log": log,
    });
    let path = root.join(FILE);
    // Whole or not at all, for a `status` that reads it now.
    let tmp = path.with_extension(format!("json.{}", std::process::id()));
    fs::write(&tmp, serde_json::to_string_pretty(&info).expect("JSON"))?;
    fs::rename(&tmp, &path)
}

/// Removes `dev.json` if it is still this process's.
pub fn forget(root: &Path) {
    let path = root.join(FILE);
    if recorded_pid(root) == Some(std::process::id()) {
        let _ = fs::remove_file(path);
    }
}

fn recorded_pid(root: &Path) -> Option<u32> {
    let info: Value = serde_json::from_str(&fs::read_to_string(root.join(FILE)).ok()?).ok()?;
    u32::try_from(info["pid"].as_u64()?).ok()
}

/// One HTTP/1.1 request to the dev server at `url`: its status, its head
/// with each line ended by CRLF and names lowercased, and its body.
fn request(url: &str, method: &str, path: &str, pid: u32) -> io::Result<(u16, String, String)> {
    let host = url
        .strip_prefix("http://")
        .map(|u| u.trim_end_matches('/'))
        .ok_or_else(|| io::Error::other(format!("{url} isn't a dev server's URL")))?;
    let address: SocketAddr = host.parse().map_err(io::Error::other)?;
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {host}\r\n{PID}: {pid}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| io::Error::other("no HTTP response"))?;
    let status = head
        .split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| io::Error::other("no HTTP status"))?;
    Ok((
        status,
        format!("{}\r\n", head.to_ascii_lowercase()),
        body.to_string(),
    ))
}

/// What `dev` says when a server already runs for the project.
pub fn already(running: &Running, out: Out) {
    out.say(&format!(
        "yuanme dev is already running here, at {} (pid {})",
        running.url, running.pid
    ));
    out.event(Event::ServerReady {
        url: running.url.clone(),
        pid: running.pid,
    });
}

/// `yuanme dev --background`: starts `dev` with `args` in a process of its
/// own, which prints into [`LOG`], and relays what it prints until its
/// first build has finished and it serves. 1 if that build failed, though
/// the server runs on.
pub fn background(root: &Path, args: &[String], out: Out) -> Result<u8, Error> {
    if let Some(running) = find(root) {
        already(&running, out);
        return Ok(0);
    }
    let log = root.join(LOG);
    let file = File::create(&log)?;
    let mut command = Command::new(std::env::current_exe()?);
    command.arg("-C").arg(root).args(["--color", "never"]);
    if out.json {
        command.arg("--json");
    }
    if out.quiet {
        command.arg("--quiet");
    }
    command
        .arg("dev")
        .args(args)
        .args(["--log", LOG])
        .stdin(Stdio::null())
        .stdout(file.try_clone()?)
        .stderr(file);
    // Out of the terminal's process group, so its Ctrl-C and hangup don't
    // reach the server.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    #[cfg(windows)]
    {
        const DETACHED_PROCESS: u32 = 0x8;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x200;
        std::os::windows::process::CommandExt::creation_flags(
            &mut command,
            DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP,
        );
    }
    let mut child = command.spawn()?;
    let mut relayed = 0;
    loop {
        relay(&log, &mut relayed, false, out)?;
        if let Some(status) = child.try_wait()? {
            relay(&log, &mut relayed, true, out)?;
            return Ok(status.code().map_or(crate::INTERNAL, |c| c as u8));
        }
        if recorded_pid(root) == Some(child.id()) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    // It writes `dev.json` after `server-ready`.
    relay(&log, &mut relayed, true, out)?;
    let Some(running) = find(root) else {
        return Err(Error::Io(io::Error::other(format!(
            "yuanme dev (pid {}) started but doesn't answer; see {LOG}",
            child.id()
        ))));
    };
    out.say(&format!(
        "yuanme dev runs in the background (pid {}): `yuanme dev logs` prints its output, and `yuanme dev stop` stops it",
        running.pid
    ));
    Ok(if running.build["ok"] == true {
        0
    } else {
        SITE_ERRORS
    })
}

/// Prints the lines of `log` from byte `from` on, and with `all` a last
/// one without its line break: an event on stdout, the rest on stderr.
fn relay(log: &Path, from: &mut u64, all: bool, out: Out) -> io::Result<()> {
    let mut file = File::open(log)?;
    file.seek(SeekFrom::Start(*from))?;
    let mut text = String::new();
    file.read_to_string(&mut text)?;
    let end = match all {
        true => text.len(),
        false => text.rfind('\n').map_or(0, |i| i + 1),
    };
    for line in text[..end].lines() {
        if out.json && line.starts_with('{') {
            crate::stdout(line);
        } else {
            eprintln!("{line}");
        }
    }
    *from += end as u64;
    Ok(())
}

/// `yuanme dev status`.
pub fn status(root: &Path, out: Out) -> Result<u8, Error> {
    let Some(running) = find(root) else {
        out.say("no yuanme dev is running here");
        out.event(Event::DevStatus(None));
        return Ok(0);
    };
    let build = &running.build;
    let count = |key: &str, one: &str| {
        let n = build[key].as_u64().unwrap_or(0) as usize;
        crate::plural(n, one)
    };
    let last = match build["ok"] == true {
        true => format!(
            "its last build took {} ms, with {}",
            build["duration_ms"],
            count("warnings", "warning")
        ),
        false => format!(
            "its last build has {} and {}, which `yuanme dev logs` shows",
            count("errors", "error"),
            count("warnings", "warning")
        ),
    };
    out.say(&format!(
        "yuanme dev is running at {} (pid {}, since {}); {last}",
        running.url,
        running.pid,
        running.info["started"].as_str().unwrap_or("?")
    ));
    out.event(Event::DevStatus(Some(&running)));
    Ok(0)
}

/// `yuanme dev stop`: asks the server to stop, and waits until it has
/// saved its cache and stopped its Tailwind CLIs.
pub fn stop(root: &Path, out: Out) -> Result<u8, Error> {
    if let Some(running) = find(root) {
        let (status, _, body) = request(&running.url, "POST", STOP, running.pid)?;
        if status != 202 {
            return Err(Error::Io(io::Error::other(format!(
                "yuanme dev (pid {}) won't stop: {status} {body}",
                running.pid
            ))));
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        while recorded_pid(root) == Some(running.pid) {
            if Instant::now() > deadline {
                return Err(Error::Io(io::Error::other(format!(
                    "yuanme dev (pid {}) didn't stop within 30 s",
                    running.pid
                ))));
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        out.say(&format!("stopped yuanme dev (pid {})", running.pid));
    } else {
        out.say("no yuanme dev is running here");
    }
    out.event(Event::DevStatus(None));
    Ok(0)
}

/// `yuanme dev logs`: what the server started with `--background` last
/// printed, and with `follow`, what it prints next until it stops.
pub fn logs(root: &Path, follow: bool) -> Result<u8, Error> {
    let mut file = File::open(root.join(LOG)).map_err(|e| match e.kind() {
        io::ErrorKind::NotFound => Error::BadCommand(
            "no yuanme dev has run in the background here: `yuanme dev --background` starts one"
                .to_string(),
        ),
        _ => Error::Io(e),
    })?;
    let mut stdout = io::stdout().lock();
    // A closed pipe, as `| head` makes, ends the output, not the command.
    let mut copy = |file: &mut File| {
        io::copy(file, &mut stdout)
            .map(drop)
            .or_else(|e| match e.kind() {
                io::ErrorKind::BrokenPipe => Ok(()),
                _ => Err(e),
            })
    };
    copy(&mut file)?;
    let Some(running) = follow.then(|| find(root)).flatten() else {
        return Ok(0);
    };
    let mut looked = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(100));
        copy(&mut file)?;
        // `dev.json` goes once the server has stopped; one that died
        // leaves it, so now and then, ask the server.
        let gone = recorded_pid(root) != Some(running.pid)
            || (looked.elapsed() > Duration::from_secs(1) && {
                looked = Instant::now();
                find(root).is_none()
            });
        if gone {
            copy(&mut file)?;
            return Ok(0);
        }
    }
}
