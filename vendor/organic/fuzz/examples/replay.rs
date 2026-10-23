//! Times every input in a fuzz corpus against the parse-time bound (#15),
//! outside libFuzzer, whose instrumentation slows parsing down:
//! `cargo run --release --example replay -- adversarial|bytes DIR...`. It is
//! an example so that `cargo fuzz build`, which instruments every binary,
//! leaves it alone.

use std::path::PathBuf;
use std::process::ExitCode;

use yuanme_org_fuzz::adversarial;
use yuanme_org_fuzz::bound;
use yuanme_org_fuzz::time;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let target = args.next().expect("usage: replay adversarial|bytes DIR...");
    let mut paths: Vec<PathBuf> = args
        .flat_map(|dir| std::fs::read_dir(dir).expect("read a corpus directory"))
        .map(|entry| entry.expect("read a corpus entry").path())
        .collect();
    paths.sort();

    // The first parse in a process pays for cold caches.
    time("* A heading\nSome *bold* text.\n");

    let mut slowest = (0.0, PathBuf::new());
    let mut over = 0;
    for path in &paths {
        let data = std::fs::read(path).expect("read a corpus input");
        let text = match target.as_str() {
            "adversarial" => adversarial(&data),
            "bytes" => match String::from_utf8(data) {
                Ok(text) => text,
                Err(_) => continue,
            },
            _ => panic!("no fuzz target {target}"),
        };
        let bound = bound(&text);
        let mut took = time(&text);
        // A runner can stall for longer than a small input's whole bound,
        // so a parse over it is timed up to twenty more times, and the best
        // time counts.
        for _ in 0..20 {
            if took <= bound {
                break;
            }
            eprintln!(
                "re-timing {}: took {took:?}, bound {bound:?}",
                path.display()
            );
            took = took.min(time(&text));
        }
        let share = took.as_secs_f64() / bound.as_secs_f64();
        if share > slowest.0 {
            slowest = (share, path.clone());
        }
        if took > bound {
            eprintln!(
                "{}: {} bytes took {took:?}, over the bound of {bound:?}",
                path.display(),
                text.len()
            );
            over += 1;
        }
    }
    println!(
        "{} inputs, {over} over the bound; the slowest took {:.2} of it ({})",
        paths.len(),
        slowest.0,
        slowest.1.display()
    );
    if over == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
