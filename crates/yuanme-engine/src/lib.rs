//! The one seam every caller crosses: `build` turns a snapshot of the
//! project into every file of the Site, in memory (ADR 0011).

mod cache;
mod link;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use yuanme_diag::{Diagnostic, Files, Level};

pub use cache::Cache;
use link::link;

/// A snapshot of the project's files, by path relative to the project root
/// with `/` separators. It is everything a build reads.
#[derive(Clone, Default)]
pub struct Inputs {
    files: BTreeMap<String, Input>,
}

#[derive(Clone)]
pub struct Input {
    pub hash: blake3::Hash,
    /// `None` for a file whose bytes were hashed as a stream and not kept,
    /// such as an image: it is read from the project when it is needed.
    pub bytes: Option<Arc<[u8]>>,
}

impl Inputs {
    pub fn insert(&mut self, path: impl Into<String>, bytes: impl Into<Arc<[u8]>>) {
        let bytes = bytes.into();
        let input = Input {
            hash: blake3::hash(&bytes),
            bytes: Some(bytes),
        };
        self.files.insert(path.into(), input);
    }

    pub fn insert_streamed(&mut self, path: impl Into<String>, hash: blake3::Hash) {
        self.files.insert(path.into(), Input { hash, bytes: None });
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Input)> {
        self.files
            .iter()
            .map(|(path, input)| (path.as_str(), input))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Build,
    Dev,
}

/// Every file of one Site, or the Diagnostics that stopped it.
pub struct Build {
    /// By path in the output directory, such as `blog/pew/index.html`.
    pub files: BTreeMap<String, Output>,
    pub diagnostics: Vec<Diagnostic>,
    /// The files the Diagnostics point into.
    pub sources: Files,
}

pub struct Output {
    pub body: Body,
    /// Changes whenever the bytes do, so the writer can skip unchanged files.
    pub hash: blake3::Hash,
}

/// Where an output file's bytes are. Only small files are held in memory;
/// the writer copies the others with `std::fs::copy`, which clones on APFS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    Bytes(Arc<[u8]>),
    /// A project file, by its path in [`Inputs`].
    Input(String),
    /// A large entry in the on-disk [`Cache`].
    Cached(PathBuf),
}

impl Build {
    /// A failed build must not touch the output directory.
    pub fn failed(&self) -> bool {
        self.diagnostics.iter().any(|d| d.level == Level::Error)
    }
}

pub fn build(inputs: &Inputs, cache: &mut Cache, _mode: Mode) -> Build {
    // parse (#46–#49) and resolve (#46) go here, ahead of render; until they
    // arrive a Site is its Static files. `_mode` is for what dev does
    // differently: Drafts (#46), images on request (#62), the unsubset font (#52).
    let (pages, statics) = rayon::join(render, || static_files(inputs));
    // The assets stage (#56) runs here alongside render and hands link its
    // bundles.
    let mut files = link(pages, Vec::new());
    files.extend(statics);

    let build = Build {
        files,
        diagnostics: Vec::new(),
        sources: Files::default(),
    };
    if !build.failed() {
        cache.sweep();
    }
    build
}

/// Pages with asset placeholders, by output path. Templates arrive with #54.
fn render() -> Vec<(String, Vec<u8>)> {
    Vec::new()
}

/// `static/` is copied to the same path, unhashed.
fn static_files(inputs: &Inputs) -> Vec<(String, Output)> {
    inputs
        .iter()
        .filter_map(|(path, input)| {
            let out = path.strip_prefix("static/")?;
            let body = match &input.bytes {
                Some(bytes) => Body::Bytes(bytes.clone()),
                None => Body::Input(path.to_string()),
            };
            Some((
                out.to_string(),
                Output {
                    body,
                    hash: input.hash,
                },
            ))
        })
        .collect()
}
