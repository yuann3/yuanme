//! load: the snapshot of the project that one build reads (#24), and, for
//! `yuanme dev`, bringing it up to date after a change (#11).

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use rayon::prelude::*;
use yuanme_engine::Inputs;

/// What the Engine reads outside `static/`, which is copied whatever it
/// holds. An allowlist rather than a denylist (#11), so editor droppings
/// such as `#a.md#`, `a.md~` and Vim's `4913` never reach a build.
const EXTENSIONS: &[&str] = &[
    "md", "org", // Source formats
    "html", "xml", "txt", "css", "js", "mjs", "ts", "mts", // Templates, Sidecars, Scripts
    "toml", "yaml", "yml", "json", // Site config, Data files
    "avif", "gif", "jpeg", "jpg", "png", "webp", "svg", "ico", // images
    "woff", "woff2", "ttf", "otf", // fonts
];

/// Images are hashed as a stream and never held: the image pipeline reads
/// them only on a cache miss (ADR 0011).
const STREAMED: &[&str] = &["avif", "gif", "jpeg", "jpg", "png", "webp"];

/// Walks the project at `root`, honouring `.gitignore` and `.ignore`, and
/// hashes every file a build could read. `out` is the output directory,
/// as a path under `root`.
pub fn load(root: &Path, out: &Path) -> io::Result<Inputs> {
    let mut inputs = Inputs::at(root.to_path_buf());
    let (files, dirs) = walk(root, out, "", None)?;
    for dir in dirs {
        inputs.insert_dir(dir);
    }
    add(&mut inputs, root, files)?;
    Ok(inputs)
}

/// Brings `inputs`, loaded from `root`, up to date after a change at each
/// of `paths`. Each is read again whatever the change was said to be: an
/// editor's save can arrive as a rename alone (#11). A folder is walked
/// again whole, and so is the project on `rescan`, when the watcher lost
/// track. Returns whether anything a build reads changed.
pub fn refresh(
    inputs: &mut Inputs,
    root: &Path,
    out: &Path,
    paths: &BTreeSet<PathBuf>,
    rescan: bool,
) -> io::Result<bool> {
    // A new ignore file can hide or show any file.
    let ignores = paths.iter().any(|p| {
        p.file_name()
            .is_some_and(|n| n == ".gitignore" || n == ".ignore")
    });
    if rescan || ignores {
        *inputs = load(root, out)?;
        return Ok(true);
    }
    // In order, so a folder comes before what is in it.
    let paths: BTreeSet<String> = paths
        .iter()
        .filter_map(|p| p.strip_prefix(root).ok()?.to_str())
        .map(|p| p.replace(std::path::MAIN_SEPARATOR, "/"))
        .collect();
    let mut changed = false;
    for path in &paths {
        if path.is_empty() {
            *inputs = load(root, out)?;
            return Ok(true);
        }
        // The nearest folder the walk took that is still there, and its
        // child on the way to `path`: a file or folder that is new, gone or
        // changed. Below a folder the walk skipped, such as `.git` or
        // `node_modules`, the child is that folder, which stays skipped.
        let parts: Vec<&str> = path.split('/').collect();
        let (dir, child) = (0..parts.len())
            .rev()
            .map(|i| (parts[..i].join("/"), parts[..=i].join("/")))
            .find(|(dir, _)| inputs.contains_dir(dir) && root.join(dir).is_dir())
            .expect("the project's folder is there while it is watched");
        let (files, dirs) = walk(root, out, &dir, Some(1))?;
        let was: Vec<_> = inputs
            .remove_dir(&child)
            .into_iter()
            .chain(inputs.remove(&child).map(|i| (child.clone(), i)))
            .map(|(p, i)| (p, i.hash))
            .collect();
        if files.contains(&child) {
            add(inputs, root, vec![child.clone()])?;
        } else if dirs.contains(&child) {
            let (files, dirs) = walk(root, out, &child, None)?;
            for dir in dirs {
                inputs.insert_dir(dir);
            }
            add(inputs, root, files)?;
        }
        let under = format!("{child}/");
        let now: Vec<_> = inputs
            .iter()
            .filter(|(p, _)| *p == child || p.starts_with(&under))
            .map(|(p, i)| (p.to_string(), i.hash))
            .collect();
        changed |= was != now;
    }
    Ok(changed)
}

/// Every file a build could read and every folder, from the project root,
/// in the folder `dir` of the project at `root`, down to `depth`.
fn walk(
    root: &Path,
    out: &Path,
    dir: &str,
    depth: Option<usize>,
) -> io::Result<(Vec<String>, Vec<String>)> {
    let skip = [out.to_path_buf(), root.join("node_modules")];
    let walk = WalkBuilder::new(root.join(dir))
        .hidden(false)
        .require_git(false)
        .follow_links(true)
        .max_depth(depth)
        .filter_entry(move |entry| {
            let name = entry.file_name().to_string_lossy();
            // Hidden files are editor locks, `.git`, `.yuanme` and
            // `.DS_Store`; `.well-known` is a Static file a host serves.
            let hidden = name.starts_with('.') && name != ".well-known";
            !hidden && !skip.iter().any(|s| entry.path() == s)
        })
        .build();
    let (mut files, mut dirs) = (Vec::new(), Vec::new());
    for entry in walk {
        let entry = entry.map_err(io::Error::other)?;
        let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
        if !is_dir && !entry.file_type().is_some_and(|t| t.is_file()) {
            continue;
        }
        let relative = entry.path().strip_prefix(root).expect("walked from root");
        let relative = relative.to_str().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: the path is not UTF-8", entry.path().display()),
            )
        })?;
        let relative = relative.replace(std::path::MAIN_SEPARATOR, "/");
        if is_dir {
            dirs.push(relative);
        } else if relative.starts_with("static/")
            || EXTENSIONS.contains(&extension(&relative).as_str())
        {
            files.push(relative);
        }
    }
    Ok((files, dirs))
}

/// Reads and hashes each of `paths`, from the project root, into `inputs`.
fn add(inputs: &mut Inputs, root: &Path, paths: Vec<String>) -> io::Result<()> {
    let files: Vec<_> = paths
        .into_par_iter()
        .map(|relative| {
            let path = root.join(&relative);
            let at = |e: io::Error| io::Error::new(e.kind(), format!("{}: {e}", path.display()));
            let file = if STREAMED.contains(&extension(&relative).as_str()) {
                let mut hasher = blake3::Hasher::new();
                hasher
                    .update_reader(File::open(&path).map_err(at)?)
                    .map_err(at)?;
                Loaded::Streamed(hasher.finalize())
            } else {
                Loaded::Bytes(fs::read(&path).map_err(at)?)
            };
            Ok((relative, file))
        })
        .collect::<io::Result<_>>()?;
    for (path, file) in files {
        match file {
            Loaded::Bytes(bytes) => inputs.insert(path, bytes),
            Loaded::Streamed(hash) => inputs.insert_streamed(path, hash),
        }
    }
    Ok(())
}

enum Loaded {
    Bytes(Vec<u8>),
    Streamed(blake3::Hash),
}

/// Lowercased, since cameras write `IMG_0001.JPG`.
fn extension(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.')
        .map_or(String::new(), |(_, ext)| ext.to_ascii_lowercase())
}
