//! Parsed Entries, a leaf cache (ADR 0011): each content file read into an
//! Entry, keyed by everything [`site::read`] reads, so a build parses only
//! the files that changed.

use std::cell::RefCell;

use serde::{Deserialize, Serialize};
use yuanme_content::config::Config;
use yuanme_content::entry::Entry;
use yuanme_content::site;
use yuanme_diag::{Diagnostic, FileId, Files};

use crate::{Cache, Inputs};

const KIND: &str = "entry";

/// An Entry read without a Diagnostic, and what its read asked `is_file`
/// with the answers, which must all still hold for it to be used.
#[derive(Serialize, Deserialize)]
pub(crate) struct Read {
    entry: Entry,
    asked: Vec<(String, bool)>,
}

/// The key of the content file at `path`: its bytes, its path and Site
/// config, since an Entry reads its Collection, its Taxonomies, the
/// timezone and the math macros from there.
pub(crate) fn key(cache: &Cache, inputs: &Inputs, path: &str) -> blake3::Hash {
    let mut key = cache.key(KIND);
    key.update(inputs.files["yuanme.toml"].hash.as_bytes());
    key.update(inputs.files[path].hash.as_bytes());
    key.update(path.as_bytes());
    key.finalize()
}

/// Reads each content file, of its Collection, into an Entry, from the
/// cache when it has it. A file with Diagnostics is read afresh each time,
/// so they are reported again.
pub(crate) fn read(
    inputs: &Inputs,
    config: &Config,
    files: &[(String, FileId)],
    sources: &Files,
    cache: &mut Cache,
) -> Vec<(Option<Entry>, Vec<Diagnostic>)> {
    let is_file = |p: &str| inputs.files.contains_key(p);
    // Only this build's, so the decoded Entries in memory stay as many as
    // the Site has.
    let mut decoded = std::mem::take(&mut cache.entries);
    let mut out = Vec::with_capacity(files.len());
    for (collection, file) in files {
        let path = sources.path(*file);
        let key = key(cache, inputs, path);
        let cached = cache
            .get(KIND, &key)
            .and_then(|bytes| decoded.remove(&key).or_else(|| decode(&bytes, path)))
            .filter(|read| read.asked.iter().all(|(p, found)| is_file(p) == *found));
        if let Some(mut read) = cached {
            // Another file may have come before it in this build.
            read.entry.file = *file;
            out.push((Some(read.entry.clone()), Vec::new()));
            cache.entries.insert(key, read);
            continue;
        }
        let asked = RefCell::new(Vec::new());
        let asking = |p: &str| {
            let found = is_file(p);
            asked.borrow_mut().push((p.to_string(), found));
            found
        };
        let (entry, diagnostics) = site::read(config, collection, *file, sources, &asking);
        if let Some(entry) = &entry
            && diagnostics.is_empty()
        {
            let read = Read {
                entry: entry.clone(),
                asked: asked.into_inner(),
            };
            let bytes = serde_json::to_vec(&read).expect("an Entry is JSON");
            cache.insert(KIND, key, bytes);
            cache.entries.insert(key, read);
        }
        out.push((entry, diagnostics));
    }
    out
}

pub(crate) fn decode(bytes: &[u8], path: &str) -> Option<Read> {
    serde_json::from_slice(bytes)
        .map_err(|e| eprintln!("warning: the cached Entry of `{path}` is unreadable, so yuanme reads the file again: {e}"))
        .ok()
}
