//! Checks that no Markdown file in the corpus comes near running out of
//! fuel (#47): `cargo run --release --example margin -- corpus/md`. Each
//! file's margin is the fuel `parse_document_bounded` gives it over the
//! least fuel it parses with.

use std::path::Path;
use std::process::ExitCode;

use comrak::{Arena, Options, fuel_for, parse_document_with_fuel};

/// The least margin a corpus file may have. The bounded parse can't tell a
/// real document from a runaway one any better than this, so a file under
/// it would fail a build if it grew a little more nested.
const LEAST: f64 = 10.0;

/// The least fuel `text` parses with.
fn needs(text: &str, options: &Options) -> u64 {
    let parses = |fuel| parse_document_with_fuel(&Arena::new(), text, options, fuel).is_ok();
    let (mut low, mut high) = (0, 1);
    while !parses(high) {
        (low, high) = (high, high * 2);
    }
    while low + 1 < high {
        let mid = low + (high - low) / 2;
        if parses(mid) {
            high = mid;
        } else {
            low = mid;
        }
    }
    high
}

fn walk(dir: &Path, out: &mut Vec<(f64, u64, usize, String)>, options: &Options) {
    for entry in std::fs::read_dir(dir).expect("read a corpus directory") {
        let path = entry.expect("read a corpus entry").path();
        if path.is_dir() {
            walk(&path, out, options);
        } else if path.extension().is_some_and(|e| e == "md") {
            let text = std::fs::read_to_string(&path).expect("read a corpus file");
            let needs = needs(&text, options);
            let margin = fuel_for(text.len()) as f64 / needs as f64;
            out.push((margin, needs, text.len(), path.display().to_string()));
        }
    }
}

fn main() -> ExitCode {
    let options = yuanme_content::markdown::options();
    let mut files = Vec::new();
    for dir in std::env::args().skip(1) {
        walk(Path::new(&dir), &mut files, &options);
    }
    files.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (margin, needs, len, path) in files.iter().take(5) {
        println!("{margin:8.1}x  needs {needs:6} units for {len:7} bytes  {path}");
    }
    let Some(&(least, ..)) = files.first() else {
        eprintln!("no Markdown files found");
        return ExitCode::FAILURE;
    };
    println!("{} files; the least margin is {least:.1}x", files.len());
    if least < LEAST {
        eprintln!("a file has less than {LEAST}x the fuel it needs");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
