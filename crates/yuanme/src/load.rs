//! load: the snapshot of the project that one build reads (#24).

use std::fs::{self, File};
use std::io;
use std::path::Path;

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
    let skip = [out.to_path_buf(), root.join("node_modules")];
    let mut paths = Vec::new();
    let walk = WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .follow_links(true)
        .filter_entry(move |entry| {
            let name = entry.file_name().to_string_lossy();
            // Hidden files are editor locks, `.git`, `.yuanme` and
            // `.DS_Store`; `.well-known` is a Static file a host serves.
            let hidden = name.starts_with('.') && name != ".well-known";
            !hidden && !skip.iter().any(|s| entry.path() == s)
        })
        .build();
    let mut dirs = Vec::new();
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
            paths.push(relative);
        }
    }

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

    let mut inputs = Inputs::at(root.to_path_buf());
    for dir in dirs {
        inputs.insert_dir(dir);
    }
    for (path, file) in files {
        match file {
            Loaded::Bytes(bytes) => inputs.insert(path, bytes),
            Loaded::Streamed(hash) => inputs.insert_streamed(path, hash),
        }
    }
    Ok(inputs)
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
