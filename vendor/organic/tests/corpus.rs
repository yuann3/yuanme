//! The Org corpus (#14) and upstream's samples, parsed the way the Engine
//! parses them.

use std::fmt::Write;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::path::PathBuf;

use yuanme_org::parser::parse_with_settings;
use yuanme_org::settings::FileAccessInterface;
use yuanme_org::settings::GlobalSettings;

/// What the Engine does with a `#+SETUPFILE`: it reports a missing one
/// itself and parses on without it (#30).
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

/// FNV-1a, so the snapshot doesn't depend on std's unspecified hasher.
fn fnv1a(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Every file parses, and each gets one line with a hash of its tree. The snapshot began as the
/// trees organic e44be41 produced, so a changed line is a change in how
/// that file reads, and needs a reason.
#[test]
fn every_file_parses_to_the_tree_in_the_snapshot() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    org_files(&crate_dir.join("../../corpus/org"), &mut files);
    org_files(&crate_dir.join("org_mode_samples"), &mut files);
    files.sort();
    assert!(files.len() > 400, "found only {} Org files", files.len());

    let settings = GlobalSettings {
        file_access: &NoFiles,
        ..GlobalSettings::default()
    };
    let mut trees = String::new();
    let mut failed = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path).expect("read an Org file");
        // The settings hold a &dyn, so they aren't UnwindSafe; nothing is reused after a panic.
        let tree = std::panic::catch_unwind(AssertUnwindSafe(|| {
            parse_with_settings(&text, &settings).map(|document| fnv1a(&format!("{document:?}")))
        }));
        let parsed = matches!(tree, Ok(Ok(_)));
        let outcome = match tree {
            Ok(Ok(hash)) => format!("{hash:016x}"),
            Ok(Err(error)) => format!("error: {error}"),
            Err(_) => "panic".to_owned(),
        };
        let name = path.strip_prefix(crate_dir).unwrap_or(&path).display();
        if !parsed {
            failed.push(format!("{name} {outcome}"));
        }
        writeln!(trees, "{name} {outcome}").unwrap();
    }
    insta::assert_snapshot!(trees);
    assert!(failed.is_empty(), "{failed:#?}");
}
