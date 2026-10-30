//! Leaf caches: the results of expensive pure transforms, keyed by a hash of
//! everything they read (ADR 0011). On disk they live in `.yuanme/cache`, so
//! a cold `yuanme dev`, `yuanme build` or CI run starts warm.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::assets::tailwind::Watching;
use crate::{Body, entries};

/// Entries no build has read for this many successful builds are deleted.
const KEEP: u64 = 10;

const MAGIC: &[u8; 8] = b"ympack01";

pub struct Cache {
    version: String,
    /// The number of the build in progress. It advances only after a
    /// successful build, so failed builds can't age entries out.
    generation: u64,
    /// `None` keeps everything in memory, for tests and fixtures.
    dir: Option<PathBuf>,
    /// Small entries, one pack per kind, such as parsed Entries.
    packs: BTreeMap<String, Pack>,
    /// Large entries, such as image variants. On disk each is its own file
    /// and the pack holds only its key and last use.
    large: Pack,
    /// The Tailwind CLIs `yuanme dev` keeps running between builds, by
    /// stylesheet (#59). Dropping the cache stops them.
    pub(crate) tailwind: BTreeMap<String, Watching>,
    /// The last build's parsed Entries, decoded, by their key in the
    /// `entry` pack: `yuanme dev` decoding every Entry on each save would
    /// cost more than parsing the one that changed.
    pub(crate) entries: HashMap<blake3::Hash, entries::Read>,
}

#[derive(Default)]
struct Pack {
    entries: HashMap<blake3::Hash, Entry>,
    /// An entry was added or removed since the pack was loaded.
    changed: bool,
}

struct Entry {
    bytes: Arc<[u8]>,
    /// The last build that read it.
    last_used: AtomicU64,
    /// `last_used` as the pack file has it.
    saved: u64,
}

impl Cache {
    pub fn memory(version: &str) -> Cache {
        Cache {
            version: version.to_string(),
            generation: 0,
            dir: None,
            packs: BTreeMap::new(),
            large: Pack::default(),
            tailwind: BTreeMap::new(),
            entries: HashMap::new(),
        }
    }

    pub fn open(dir: PathBuf, version: &str) -> io::Result<Cache> {
        let mut cache = Cache::memory(version);
        match fs::read_to_string(dir.join("generation")) {
            Ok(text) => {
                cache.generation = text
                    .trim()
                    .parse()
                    .map_err(|e| invalid(&dir.join("generation"), e))?;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        cache.large = read_pack(&dir.join("large.index"))?;
        let files = match fs::read_dir(&dir) {
            Ok(files) => files,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                cache.dir = Some(dir);
                return Ok(cache);
            }
            Err(e) => return Err(e),
        };
        for file in files {
            let path = file?.path();
            if let Some(kind) = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".pack"))
            {
                cache.packs.insert(kind.to_string(), read_pack(&path)?);
            }
        }
        cache.dir = Some(dir);
        Ok(cache)
    }

    /// The version every key starts with, which is also the one
    /// `yuanme = "X.Y"` in Site config is checked against.
    pub fn engine_version(&self) -> &str {
        &self.version
    }

    /// Starts the key of a `kind` entry. Every key begins with the Engine
    /// version, so a new Engine never reads an old Engine's results.
    pub fn key(&self, kind: &str) -> blake3::Hasher {
        let mut hasher = blake3::Hasher::new();
        for part in [&self.version, kind] {
            hasher.update(&(part.len() as u64).to_le_bytes());
            hasher.update(part.as_bytes());
        }
        hasher
    }

    pub fn get(&self, kind: &str, key: &blake3::Hash) -> Option<Arc<[u8]>> {
        let entry = self.packs.get(kind)?.entries.get(key)?;
        entry.last_used.store(self.generation, Ordering::Relaxed);
        Some(entry.bytes.clone())
    }

    pub fn insert(&mut self, kind: &str, key: blake3::Hash, bytes: impl Into<Arc<[u8]>>) {
        let pack = self.packs.entry(kind.to_string()).or_default();
        pack.insert(key, bytes.into(), self.generation);
    }

    pub fn get_large(&self, key: &blake3::Hash) -> Option<Body> {
        let entry = self.large.entries.get(key)?;
        entry.last_used.store(self.generation, Ordering::Relaxed);
        Some(match &self.dir {
            Some(dir) => Body::Cached(large_path(dir, key)),
            None => Body::Bytes(entry.bytes.clone()),
        })
    }

    pub fn insert_large(&mut self, key: blake3::Hash, bytes: &[u8]) -> io::Result<()> {
        let bytes: Arc<[u8]> = match &self.dir {
            Some(dir) => {
                write_atomic(&large_path(dir, &key), bytes)?;
                Arc::new([])
            }
            None => bytes.into(),
        };
        self.large.insert(key, bytes, self.generation);
        Ok(())
    }

    /// Ends a successful build: deletes what the last [`KEEP`] builds didn't
    /// read, and starts the next build's generation.
    pub(crate) fn sweep(&mut self) {
        let generation = self.generation;
        let stale = |entry: &Entry| generation - entry.last_used.load(Ordering::Relaxed) >= KEEP;
        for pack in self.packs.values_mut() {
            pack.retain(|_, entry| !stale(entry));
        }
        let dir = &self.dir;
        self.large.retain(|key, entry| {
            if !stale(entry) {
                return true;
            }
            if let Some(dir) = dir
                && let Err(e) = fs::remove_file(large_path(dir, key))
                && e.kind() != io::ErrorKind::NotFound
            {
                // Left behind, it costs only disk space: say so and go on.
                eprintln!("warning: could not delete a cache file: {e}");
            }
            false
        });
        self.generation += 1;
    }

    /// Writes the packs that changed, each atomically, so a crash mid-write
    /// leaves the previous pack. A memory cache has nothing to save.
    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = &self.dir else {
            return Ok(());
        };
        fs::create_dir_all(dir)?;
        for (kind, pack) in &mut self.packs {
            pack.save(&dir.join(format!("{kind}.pack")))?;
        }
        self.large.save(&dir.join("large.index"))?;
        write_atomic(
            &dir.join("generation"),
            self.generation.to_string().as_bytes(),
        )
    }
}

impl Pack {
    fn insert(&mut self, key: blake3::Hash, bytes: Arc<[u8]>, generation: u64) {
        let entry = Entry {
            bytes,
            last_used: AtomicU64::new(generation),
            saved: generation,
        };
        self.entries.insert(key, entry);
        self.changed = true;
    }

    fn retain(&mut self, mut keep: impl FnMut(&blake3::Hash, &Entry) -> bool) {
        let before = self.entries.len();
        self.entries.retain(|key, entry| keep(key, entry));
        self.changed |= self.entries.len() != before;
    }

    /// Rewrites the pack only if it changed. A build that read an entry
    /// changes it, since the entry's last use is part of the pack.
    fn save(&mut self, path: &Path) -> io::Result<()> {
        let used = self
            .entries
            .values()
            .any(|e| e.last_used.load(Ordering::Relaxed) != e.saved);
        if !(self.changed || used) {
            return Ok(());
        }
        // Sorted, so the same entries always make the same file.
        let mut entries: Vec<_> = self.entries.iter_mut().collect();
        entries.sort_unstable_by_key(|(key, _)| *key.as_bytes());
        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&(entries.len() as u64).to_le_bytes());
        for (key, entry) in entries {
            entry.saved = *entry.last_used.get_mut();
            out.extend_from_slice(key.as_bytes());
            out.extend_from_slice(&entry.saved.to_le_bytes());
            out.extend_from_slice(&(entry.bytes.len() as u64).to_le_bytes());
            out.extend_from_slice(&entry.bytes);
        }
        write_atomic(path, &out)?;
        self.changed = false;
        Ok(())
    }
}

fn read_pack(path: &Path) -> io::Result<Pack> {
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Pack::default()),
        Err(e) => return Err(e),
    };
    let corrupt = || invalid(path, "truncated or not a yuanme cache pack");
    let mut rest = data.strip_prefix(MAGIC).ok_or_else(corrupt)?;
    let mut take = |n: usize| -> io::Result<&[u8]> {
        let (head, tail) = rest.split_at_checked(n).ok_or_else(corrupt)?;
        rest = tail;
        Ok(head)
    };
    let u64_at = |bytes: &[u8]| u64::from_le_bytes(bytes.try_into().expect("8 bytes"));
    let count = u64_at(take(8)?);
    let mut pack = Pack::default();
    for _ in 0..count {
        let key = blake3::Hash::from_bytes(take(32)?.try_into().expect("32 bytes"));
        let last_used = u64_at(take(8)?);
        let len = usize::try_from(u64_at(take(8)?)).map_err(|_| corrupt())?;
        let entry = Entry {
            bytes: take(len)?.into(),
            last_used: AtomicU64::new(last_used),
            saved: last_used,
        };
        pack.entries.insert(key, entry);
    }
    Ok(pack)
}

fn large_path(dir: &Path, key: &blake3::Hash) -> PathBuf {
    dir.join("large").join(key.to_hex().as_str())
}

/// Temp file and rename: readers see the old file or the new one, never half.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().expect("cache files are inside the cache");
    fs::create_dir_all(dir)?;
    let tmp = path.with_extension("tmp");
    let mut file = fs::File::create(&tmp)?;
    file.write_all(bytes)?;
    drop(file);
    fs::rename(&tmp, path)
}

fn invalid(path: &Path, e: impl std::fmt::Display) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!(
            "{}: {e}; delete it to start from a cold cache",
            path.display()
        ),
    )
}
