//! The project's files, as one build reads them, and the Templates and
//! Static files among them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use yuanme_diag::{Code, Diagnostic, FileId, Files, Span, codes};

use crate::{Body, Output};

/// A snapshot of the project's files, by path relative to the project root
/// with `/` separators. It is everything a build reads.
#[derive(Clone, Default)]
pub struct Inputs {
    /// Where streamed files are read from. `None` for Inputs built in
    /// memory, which hold every file's bytes.
    pub(crate) root: Option<PathBuf>,
    pub(crate) files: BTreeMap<String, Input>,
    /// Folders the walk found, so an empty Collection folder still exists.
    dirs: BTreeSet<String>,
}

#[derive(Clone)]
pub struct Input {
    pub hash: blake3::Hash,
    /// `None` for a file whose bytes were hashed as a stream and not kept,
    /// such as an image: it is read from the project when it is needed.
    pub bytes: Option<Arc<[u8]>>,
}

impl Inputs {
    /// The Inputs of the project at `root`.
    pub fn at(root: PathBuf) -> Inputs {
        Inputs {
            root: Some(root),
            ..Inputs::default()
        }
    }

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

    pub fn insert_dir(&mut self, path: impl Into<String>) {
        self.dirs.insert(path.into());
    }

    /// The project's folder, unless the Inputs were built in memory.
    pub fn root(&self) -> Option<&std::path::Path> {
        self.root.as_deref()
    }

    pub fn get(&self, path: &str) -> Option<&Input> {
        self.files.get(path)
    }

    /// Whether the walk found the folder `dir`; the project's own is `""`.
    pub fn contains_dir(&self, dir: &str) -> bool {
        self.dirs.contains(dir)
    }

    pub fn remove(&mut self, path: &str) -> Option<Input> {
        self.files.remove(path)
    }

    /// Removes the folder `dir` and everything in it, returning the files.
    pub fn remove_dir(&mut self, dir: &str) -> Vec<(String, Input)> {
        let under = format!("{dir}/");
        let inside = |p: &String| p.starts_with(&under);
        self.dirs.remove(dir);
        self.dirs.retain(|d| !inside(d));
        let files: Vec<String> = self
            .files
            .range(under.clone()..)
            .map(|(p, _)| p)
            .take_while(|p| inside(p))
            .cloned()
            .collect();
        files
            .into_iter()
            .map(|p| {
                let input = self
                    .files
                    .remove(&p)
                    .expect("`files` was listed from self.files just above");
                (p, input)
            })
            .collect()
    }

    pub(crate) fn is_dir(&self, dir: &str) -> bool {
        let under = format!("{dir}/");
        self.dirs.contains(dir)
            || self
                .files
                .range(under.clone()..)
                .next()
                .is_some_and(|(p, _)| p.starts_with(&under))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &Input)> {
        self.files
            .iter()
            .map(|(path, input)| (path.as_str(), input))
    }

    /// Every Template, by its path in `templates/`: every text file there but
    /// the Sidecars beside them.
    pub(crate) fn templates(
        &self,
        sources: &mut Files,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> BTreeMap<String, FileId> {
        let mut out = BTreeMap::new();
        for (path, input) in self.iter() {
            let Some(name) = path.strip_prefix("templates/") else {
                continue;
            };
            if [".css", ".js", ".mjs", ".ts", ".mts"]
                .iter()
                .any(|e| name.ends_with(e))
            {
                continue;
            }
            // An image, which is streamed, is no Template.
            let Some(bytes) = input.bytes.as_deref() else {
                continue;
            };
            match utf8(path, bytes, &codes::TEMPLATE_SYNTAX, sources) {
                Ok(file) => {
                    out.insert(name.to_string(), file);
                }
                Err(d) => diagnostics.push(d),
            }
        }
        out
    }

    /// `static/` is copied to the same path, unhashed.
    pub(crate) fn statics(&self) -> Vec<(String, Output)> {
        self.iter()
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
}

/// Adds a text file to `sources`. Not UTF-8 is an error rather than a lossy
/// copy, which could parse as something the Author never wrote.
pub(crate) fn utf8(
    path: &str,
    bytes: &[u8],
    code: &'static Code,
    sources: &mut Files,
) -> Result<FileId, Diagnostic> {
    match std::str::from_utf8(bytes) {
        Ok(text) => Ok(sources.add(path, text)),
        Err(e) => {
            // The lossy copy matches the bytes up to the first bad one.
            let file = sources.add(path, String::from_utf8_lossy(bytes));
            let at = e.valid_up_to();
            let span = Span {
                file,
                range: at..at,
            };
            Err(Diagnostic::error(
                code,
                format!("{path} is not UTF-8"),
                span,
            ))
        }
    }
}
