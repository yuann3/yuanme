//! Parse time over the Org corpus (#14), single-threaded:
//! `cargo run --release -p yuanme-org --example bench`.
//!
//! Each file's time is the best of 5 parses; the total is their sum.

use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use yuanme_org::parser::parse_with_settings;
use yuanme_org::settings::FileAccessInterface;
use yuanme_org::settings::GlobalSettings;

struct NoFiles;

impl FileAccessInterface for NoFiles {
    fn read_file(&self, _path: &str) -> Result<String, std::io::Error> {
        Ok(String::new())
    }
}

fn org_files(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read a corpus directory") {
        let path = entry.expect("read a corpus entry").path();
        if path.is_dir() {
            org_files(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "org") {
            files.push(path);
        }
    }
}

fn main() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../corpus/org");
    let mut files = Vec::new();
    org_files(&corpus, &mut files);
    files.sort();

    let settings = GlobalSettings {
        file_access: &NoFiles,
        ..GlobalSettings::default()
    };
    let mut total = Duration::ZERO;
    let mut bytes = 0;
    let mut slowest = (Duration::ZERO, PathBuf::new());
    for path in &files {
        let text = std::fs::read_to_string(path).expect("read an Org file");
        let best = (0..5)
            .map(|_| {
                let start = Instant::now();
                let parsed = parse_with_settings(&text, &settings);
                let elapsed = start.elapsed();
                drop(parsed);
                elapsed
            })
            .min()
            .expect("five runs");
        total += best;
        bytes += text.len();
        if best > slowest.0 {
            slowest = (best, path.clone());
        }
    }
    let kb = bytes as f64 / 1024.0;
    println!(
        "{} files, {kb:.0} KB: {:.1} ms, {:.1} µs/KB; slowest {:.2} ms ({})",
        files.len(),
        total.as_secs_f64() * 1e3,
        total.as_secs_f64() * 1e6 / kb,
        slowest.0.as_secs_f64() * 1e3,
        slowest
            .1
            .strip_prefix(&corpus)
            .unwrap_or(&slowest.1)
            .display(),
    );
}
