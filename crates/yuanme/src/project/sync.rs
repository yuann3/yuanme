//! write: syncs a Build into the output directory, never wiping it (#24).
//! Creating directories is what costs, and more threads make it slower on
//! both macOS and Linux, so they are made on one thread and files written
//! on four. Syncing 10,000 Pages with 10 changed took 7 ms; wiping and
//! rewriting them took 1.8 s on macOS.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use rayon::prelude::*;
use yuanme_engine::{Body, Build};

const HEADER: &str = "yuanme output manifest 1";

/// Output paths, each sorted.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub written: Vec<String>,
    pub deleted: Vec<String>,
    /// Files the Engine never wrote. They are left alone.
    pub unknown: Vec<String>,
}

/// What the previous build wrote to one path, and what `stat` said after.
struct Record {
    hash: blake3::Hash,
    len: u64,
    mtime: u128,
}

/// Writes the files of `build` that differ from what is in `out`, and
/// deletes the files the previous build wrote that this one didn't.
/// `manifest` holds what the previous build wrote; every file is checked
/// with `stat` against it, so a file edited outside the Engine is rewritten.
pub fn sync(build: &Build, root: &Path, out: &Path, manifest: &Path) -> io::Result<Report> {
    if build.failed() {
        return Err(io::Error::other("a failed build writes nothing"));
    }
    let previous = read_manifest(manifest, out)?;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build()
        .map_err(io::Error::other)?;

    let mut written: Vec<&str> = pool.install(|| {
        build
            .files
            .par_iter()
            .filter(|(path, output)| {
                let record = previous.get(path.as_str());
                !record.is_some_and(|r| r.hash == output.hash && on_disk(&out.join(path), r))
            })
            .map(|(path, _)| path.as_str())
            .collect()
    });
    written.sort_unstable();

    let dirs: BTreeSet<&Path> = written
        .iter()
        .filter_map(|path| Path::new(path).parent())
        .collect();
    for dir in dirs {
        fs::create_dir_all(out.join(dir)).map_err(|e| at(&out.join(dir), e))?;
    }
    pool.install(|| {
        written.par_iter().try_for_each(|path| {
            let to = out.join(path);
            match &build.files[*path].body {
                Body::Bytes(bytes) => fs::write(&to, bytes),
                Body::Input(input) => fs::copy(root.join(input), &to).map(drop),
                Body::Cached(from) => fs::copy(from, &to).map(drop),
            }
            .map_err(|e| at(&to, e))
        })
    })?;

    let mut deleted = Vec::new();
    for path in previous.keys().filter(|p| !build.files.contains_key(*p)) {
        let file = out.join(path);
        match fs::remove_file(&file) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(at(&file, e)),
        }
        // A directory the Engine emptied goes too; one with anything left
        // in it fails to delete, which ends the climb.
        let mut dir = file.parent();
        while let Some(d) = dir.filter(|d| *d != out) {
            if fs::remove_dir(d).is_err() {
                break;
            }
            dir = d.parent();
        }
        deleted.push(path.to_string());
    }
    deleted.sort_unstable();

    let mut unknown = Vec::new();
    walk(out, out, &mut |path| {
        if !build.files.contains_key(&path) && !previous.contains_key(path.as_str()) {
            unknown.push(path);
        }
    })?;
    unknown.sort_unstable();

    let mut lines = vec![HEADER.to_string(), format!("out {}", out.display())];
    for (path, output) in &build.files {
        // A file not written was just checked against its record.
        let (len, mtime) = match written.binary_search(&path.as_str()) {
            Ok(_) => stat(&out.join(path))?,
            Err(_) => {
                let record = &previous[path.as_str()];
                (record.len, record.mtime)
            }
        };
        lines.push(format!("{} {len} {mtime} {path}", output.hash.to_hex()));
    }
    lines.push(String::new());
    let tmp = manifest.with_extension("tmp");
    fs::create_dir_all(manifest.parent().expect("the manifest is in .yuanme/cache"))?;
    fs::write(&tmp, lines.join("\n"))?;
    fs::rename(&tmp, manifest)?;

    Ok(Report {
        written: written.into_iter().map(String::from).collect(),
        deleted,
        unknown,
    })
}

/// No manifest, or one for another output directory, means everything is
/// written.
fn read_manifest(manifest: &Path, out: &Path) -> io::Result<HashMap<String, Record>> {
    let text = match fs::read_to_string(manifest) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(HashMap::new()),
        Err(e) => return Err(at(manifest, e)),
    };
    let mut lines = text.lines();
    let out_line = format!("out {}", out.display());
    if lines.next() != Some(HEADER) || lines.next() != Some(out_line.as_str()) {
        return Ok(HashMap::new());
    }
    lines
        .map(|line| {
            let corrupt = || at(manifest, io::Error::other(format!("bad line `{line}`")));
            let mut fields = line.splitn(4, ' ');
            let mut next = || fields.next().ok_or_else(corrupt);
            let hash = blake3::Hash::from_hex(next()?).map_err(|_| corrupt())?;
            let len = next()?.parse().map_err(|_| corrupt())?;
            let mtime = next()?.parse().map_err(|_| corrupt())?;
            let path = next()?.to_string();
            Ok((path, Record { hash, len, mtime }))
        })
        .collect()
}

/// Whether the file is still exactly what the previous build wrote.
fn on_disk(path: &Path, record: &Record) -> bool {
    stat(path).is_ok_and(|(len, mtime)| len == record.len && mtime == record.mtime)
}

fn stat(path: &Path) -> io::Result<(u64, u128)> {
    let meta = fs::metadata(path).map_err(|e| at(path, e))?;
    let mtime = meta
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    Ok((meta.len(), mtime))
}

/// Calls `found` with the `/`-separated path of every file under `dir`.
fn walk(base: &Path, dir: &Path, found: &mut impl FnMut(String)) -> io::Result<()> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(at(dir, e)),
    };
    for entry in entries {
        let entry = entry?;
        let path: PathBuf = entry.path();
        // The type from `readdir` costs no `stat`.
        if entry.file_type()?.is_dir() {
            walk(base, &path, found)?;
        } else {
            let relative = path.strip_prefix(base).expect("walked from base");
            found(
                relative
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/"),
            );
        }
    }
    Ok(())
}

fn at(path: &Path, e: io::Error) -> io::Error {
    io::Error::new(e.kind(), format!("{}: {e}", path.display()))
}
