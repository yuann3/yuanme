//! The build Gates on the Scale Site (#15, #73): each of #15's build
//! numbers, timed with hyperfine on the release binary, against `main`'s
//! binary timed in the same run. More than 10% slower than main fails; a
//! number over its budget is reported, until the Engine meets them (#73).

use std::fmt::Write as _;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::Value;

pub enum Stat {
    Median,
    P95,
    Max,
}

pub struct Gate {
    pub name: &'static str,
    /// In seconds, or bytes for memory.
    pub limit: f64,
    /// What is held to the limit.
    pub stat: Stat,
    /// Whether it may be no more than 10% slower than `main`, which is
    /// judged on medians: a p95 of 50 edits moves more than 10% between
    /// two runs of the same binary.
    pub relative: bool,
}

/// #15's numbers for the Scale Site.
pub const GATES: &[Gate] = &[
    // Excluding image encoding, which the next Gate covers.
    Gate {
        name: "cold build",
        limit: 10.0,
        stat: Stat::Median,
        relative: true,
    },
    Gate {
        name: "image encoding per output",
        limit: 0.075,
        stat: Stat::Median,
        relative: false,
    },
    Gate {
        name: "warm build",
        limit: 2.0,
        stat: Stat::Median,
        relative: true,
    },
    // Engine side: from the batch of changes to the build's last event.
    Gate {
        name: "incremental rebuild",
        limit: 0.1,
        stat: Stat::P95,
        relative: true,
    },
    // `yuanme dev` to the first Page served, with a warm disk cache.
    Gate {
        name: "dev cold start",
        limit: 2.0,
        stat: Stat::Median,
        relative: true,
    },
    Gate {
        name: "check",
        limit: 1.0,
        stat: Stat::Median,
        relative: true,
    },
    Gate {
        name: "peak RSS",
        limit: 1e9,
        stat: Stat::Max,
        relative: false,
    },
];

const SLOWER: f64 = 1.10;

fn median(xs: &[f64]) -> f64 {
    let mut xs = xs.to_vec();
    xs.sort_by(f64::total_cmp);
    let n = xs.len();
    match n % 2 {
        1 => xs[n / 2],
        _ => (xs[n / 2 - 1] + xs[n / 2]) / 2.0,
    }
}

/// By nearest rank.
fn p95(xs: &[f64]) -> f64 {
    let mut xs = xs.to_vec();
    xs.sort_by(f64::total_cmp);
    xs[(xs.len() * 95).div_ceil(100) - 1]
}

impl Gate {
    fn value(&self, xs: &[f64]) -> f64 {
        match self.stat {
            Stat::Median => median(xs),
            Stat::P95 => p95(xs),
            Stat::Max => xs.iter().copied().fold(f64::MIN, f64::max),
        }
    }

    fn show(&self, x: f64) -> String {
        match self.name {
            "peak RSS" => format!("{:.0} MB", x / 1e6),
            _ if x < 1.0 => format!("{:.1} ms", x * 1e3),
            _ => format!("{x:.2} s"),
        }
    }
}

/// How far `gate` is over its budget, in words: reported, not failed on,
/// until the Engine meets #15's numbers on the Scale Site (#73).
pub fn over(gate: &Gate, head: &[f64]) -> Option<String> {
    let value = gate.value(head);
    (value > gate.limit).then(|| {
        format!(
            "{}: {} is over its budget of {}",
            gate.name,
            gate.show(value),
            gate.show(gate.limit)
        )
    })
}

/// How much slower than main `gate` is, in words, when that fails it.
pub fn slower(gate: &Gate, head: &[f64], base: &[f64]) -> Option<String> {
    let (now, before) = (median(head), median(base));
    (gate.relative && now > before * SLOWER).then(|| {
        format!(
            "{}: {} is {:.0}% slower than main's {}",
            gate.name,
            gate.show(now),
            (now / before - 1.0) * 100.0,
            gate.show(before)
        )
    })
}

/// One binary and the Scale Site it builds.
struct Side {
    name: &'static str,
    bin: PathBuf,
    dir: PathBuf,
}

/// Runs every Gate on `head`, against `base` when there is one, and fails
/// if one breaches.
pub fn run(head: &Path, base: Option<&Path>) -> ExitCode {
    let work = crate::root().join("target/gates");
    let mut sides = Vec::new();
    for (name, bin) in [("main", base), ("head", Some(head))] {
        let Some(bin) = bin else { continue };
        let dir = work.join(name);
        let _ = fs::remove_dir_all(&dir);
        crate::scale::generate(&dir).expect("write the Scale Site");
        let bin = bin.canonicalize().expect("the yuanme binary");
        sides.push(Side { name, bin, dir });
    }
    let readings = measure(&sides, &work);

    let mut failed = Vec::new();
    let mut table = String::from(
        "| Gate | Budget | This commit | main | Change |\n|---|---:|---:|---:|---:|\n",
    );
    for gate in GATES {
        let (mut head, mut base) = readings[gate.name].clone();
        let mut found = base.as_ref().and_then(|b| slower(gate, &head, b));
        // A slowdown may be the runner: measured again, it fails only if
        // it is there again.
        if let Some(slowdown) = &found {
            println!("::warning::{slowdown}; measuring it again");
            (head, base) = measure_one(gate.name, &sides, &work);
            found = base.as_ref().and_then(|b| slower(gate, &head, b));
        }
        let budget = over(gate, &head);
        if let Some(over) = &budget {
            println!("::warning::{over}");
        }
        let change = match &base {
            Some(base) if gate.relative => {
                format!("{:+.1}%", (median(&head) / median(base) - 1.0) * 100.0)
            }
            _ => String::new(),
        };
        let _ = writeln!(
            table,
            "| {}{} | {}{} | {} | {} | {change} |",
            gate.name,
            if found.is_some() { " ❌" } else { "" },
            gate.show(gate.limit),
            if budget.is_some() { ", over" } else { "" },
            gate.show(gate.value(&head)),
            base.as_ref()
                .map(|b| gate.show(gate.value(b)))
                .unwrap_or_default(),
        );
        failed.extend(found);
    }
    println!("{table}");
    if let Ok(summary) = std::env::var("GITHUB_STEP_SUMMARY") {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(summary)
            .expect("open the step summary");
        let _ = writeln!(
            file,
            "## Build Gates on the Scale Site\n\n{table}\n\
             Only ❌, more than 10% slower than main, fails the job. A number \
             over #15's budget is reported until the Engine meets them (#73).\n"
        );
    }
    for breach in &failed {
        println!("::error::{breach}");
    }
    match failed.is_empty() {
        true => ExitCode::SUCCESS,
        false => ExitCode::FAILURE,
    }
}

/// Each Gate's samples, for this commit and for main.
type Readings = std::collections::BTreeMap<&'static str, (Vec<f64>, Option<Vec<f64>>)>;

fn measure(sides: &[Side], work: &Path) -> Readings {
    let mut readings = Readings::new();
    // The first build encodes every image, which the cold builds then find
    // in the cache. Before any other, since peak RSS counts every child.
    let mut full = Vec::new();
    for side in sides {
        let _ = fs::remove_dir_all(side.dir.join(".yuanme"));
        let started = Instant::now();
        let child = yuanme(side, &["build", "-q"])
            .spawn()
            .expect("run yuanme build");
        let rss = wait_for_rss(child, side);
        let took = started.elapsed().as_secs_f64();
        let outputs = image_outputs(&side.dir.join("dist/assets"));
        eprintln!(
            "{}: the first build took {took:.1} s, encoded {outputs} image files, and peaked at {:.0} MB",
            side.name,
            rss / 1e6
        );
        full.push((took, outputs, rss));
    }
    let pair = |f: &dyn Fn(&(f64, usize, f64)) -> f64| -> (Vec<f64>, Option<Vec<f64>>) {
        let mut all: Vec<Vec<f64>> = full.iter().map(|s| vec![f(s)]).collect();
        let head = all.pop().expect("head");
        (head, all.pop())
    };
    readings.insert("peak RSS", pair(&|s| s.2));
    for gate in GATES {
        if !matches!(gate.name, "peak RSS" | "image encoding per output") {
            readings.insert(gate.name, measure_one(gate.name, sides, work));
        }
    }
    // The first build less a cold build that finds every image encoded.
    let cold = &readings["cold build"];
    let mut colds = cold.1.iter().map(|b| median(b)).collect::<Vec<_>>();
    colds.push(median(&cold.0));
    let per: Vec<f64> = full
        .iter()
        .zip(colds)
        .map(|((took, outputs, _), cold)| (took - cold) / *outputs as f64)
        .collect();
    let (head, base) = match per.as_slice() {
        [base, head] => (vec![*head], Some(vec![*base])),
        [head] => (vec![*head], None),
        _ => unreachable!("one side or two"),
    };
    readings.insert("image encoding per output", (head, base));
    readings
}

/// Runs and warmups for each hyperfine Gate: enough that the median of a
/// run moves less than the 10% the relative Gate allows (#73).
fn measure_one(name: &str, sides: &[Side], work: &Path) -> (Vec<f64>, Option<Vec<f64>>) {
    let xtask = std::env::current_exe().expect("this xtask");
    let quote = |p: &Path| format!("'{}'", p.display());
    let samples: Vec<Vec<f64>> = match name {
        "incremental rebuild" => sides.iter().map(edits).collect(),
        _ => {
            let (runs, warmup, args) = match name {
                "cold build" => (5, 0, "build -q"),
                "warm build" => (10, 1, "build -q"),
                "check" => (10, 1, "check -q"),
                "dev cold start" => (10, 1, ""),
                _ => unreachable!("{name} isn't timed by hyperfine"),
            };
            let mut hyperfine = Command::new("hyperfine");
            hyperfine.args(["--style", "basic", "--runs", &runs.to_string()]);
            hyperfine.args(["--warmup", &warmup.to_string()]);
            let json = work.join("hyperfine.json");
            hyperfine.arg("--export-json").arg(&json);
            for side in sides {
                let command = match name {
                    "dev cold start" => format!(
                        "{} first-page {} {}",
                        quote(&xtask),
                        quote(&side.bin),
                        quote(&side.dir)
                    ),
                    _ => format!("{} -C {} {args}", quote(&side.bin), quote(&side.dir)),
                };
                hyperfine.args(["--command-name", &format!("{} {name}", side.name)]);
                if name == "cold build" {
                    // No cache but the images' (#15: excluding encoding).
                    let cache = side.dir.join(".yuanme/cache");
                    hyperfine.arg("--prepare").arg(format!(
                        "rm -rf {} {}/*.pack {}",
                        quote(&side.dir.join("dist")),
                        quote(&cache),
                        quote(&cache.join("output.manifest"))
                    ));
                }
                hyperfine.arg(command);
            }
            let status = hyperfine.status().expect("run hyperfine");
            assert!(status.success(), "hyperfine failed for {name}");
            let json: Value =
                serde_json::from_str(&fs::read_to_string(&json).expect("hyperfine's JSON"))
                    .expect("hyperfine's JSON");
            json["results"]
                .as_array()
                .expect("hyperfine's results")
                .iter()
                .map(|r| {
                    r["times"]
                        .as_array()
                        .expect("hyperfine's times")
                        .iter()
                        .map(|t| t.as_f64().expect("a time"))
                        .collect()
                })
                .collect()
        }
    };
    let mut samples = samples;
    let head = samples.pop().expect("head");
    (head, samples.pop())
}

fn yuanme(side: &Side, args: &[&str]) -> Command {
    let mut command = Command::new(&side.bin);
    command.arg("-C").arg(&side.dir).args(args);
    command
}

/// The child's peak resident set size in bytes, once it exits; it must
/// succeed.
fn wait_for_rss(child: Child, side: &Side) -> f64 {
    let mut status = 0;
    // SAFETY: rusage is plain data that wait4 fills in.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    let pid = child.id() as libc::pid_t;
    // SAFETY: the pid is our child's, which nothing else waits for.
    let waited = unsafe { libc::wait4(pid, &mut status, 0, &mut usage) };
    assert_eq!(waited, pid, "wait for yuanme");
    assert!(
        libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0,
        "{}: the Scale Site doesn't build: {status}",
        side.name
    );
    // Linux counts kilobytes, macOS bytes.
    let scale = if cfg!(target_os = "macos") {
        1.0
    } else {
        1024.0
    };
    usage.ru_maxrss as f64 * scale
}

fn image_outputs(dir: &Path) -> usize {
    fs::read_dir(dir)
        .expect("the Site's assets")
        .filter(|e| {
            let name = e.as_ref().expect("an asset").file_name();
            let name = name.to_string_lossy();
            [".avif", ".jpg", ".png", ".webp"]
                .iter()
                .any(|x| name.ends_with(x))
        })
        .count()
}

/// `yuanme dev --json` on a free port, with its events.
fn dev(bin: &Path, dir: &Path) -> (Child, mpsc::Receiver<Value>, String) {
    let mut child = Command::new(bin)
        .arg("-C")
        .arg(dir)
        .args(["dev", "--json", "--port", "0"])
        .stdout(Stdio::piped())
        .spawn()
        .expect("run yuanme dev");
    let stdout = child.stdout.take().expect("piped");
    let (send, events) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if send.send(event).is_err() {
                break;
            }
        }
    });
    let url = loop {
        let event = events
            .recv_timeout(Duration::from_secs(60))
            .expect("yuanme dev to serve");
        if event["type"] == "server-ready" {
            break event["url"].as_str().expect("its URL").to_string();
        }
    };
    (child, events, url)
}

/// `cargo xtask first-page YUANME DIR`, which hyperfine times: starts
/// `yuanme dev` and fetches the home page, then stops it.
pub fn first_page(bin: &Path, dir: &Path) -> ExitCode {
    let (mut child, _events, url) = dev(bin, dir);
    let address = url
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();
    let mut stream = TcpStream::connect(&address).expect("connect to yuanme dev");
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .expect("ask for the home page");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("the home page");
    // Killed, not stopped: a stop saves the cache, which isn't serving.
    let _ = child.kill();
    let _ = child.wait();
    match response.starts_with("HTTP/1.1 200") {
        true => ExitCode::SUCCESS,
        false => {
            eprintln!(
                "yuanme dev answered: {}",
                response.lines().next().unwrap_or("")
            );
            ExitCode::FAILURE
        }
    }
}

/// A prose edit to one Post, 60 times in `yuanme dev`: each edit's
/// rebuild, as `build-finished` times it, after the first 10, which warm
/// what the first build didn't.
fn edits(side: &Side) -> Vec<f64> {
    let post = side.dir.join("content/posts/calculus-0006.md");
    let url = "/blog/calculus-0006/";
    let text = fs::read_to_string(&post).expect("the Post");
    let (mut child, events, _) = dev(&side.bin, &side.dir);
    let mut times = Vec::new();
    for i in 0..60 {
        let edited = text.replacen("\n---\n\n", &format!("\n---\n\nEdit {i}.\n\n"), 1);
        fs::write(&post, edited).expect("edit the Post");
        let finished = loop {
            let event = events
                .recv_timeout(Duration::from_secs(30))
                .expect("yuanme dev to rebuild");
            if event["type"] == "build-finished" {
                break event;
            }
        };
        assert_eq!(
            finished["ok"], true,
            "{}: the edit failed the build",
            side.name
        );
        let changed = finished["changed_urls"].as_array().expect("its URLs");
        assert!(
            changed.iter().any(|u| u == url),
            "{}: the edit changed {changed:?}",
            side.name
        );
        if i >= 10 {
            times.push(finished["duration_ms"].as_f64().expect("its duration") / 1e3);
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    fs::write(&post, text).expect("restore the Post");
    times
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gate(name: &str) -> &'static Gate {
        GATES.iter().find(|g| g.name == name).unwrap()
    }

    #[test]
    fn a_number_over_its_budget_is_reported() {
        let cold = gate("cold build");
        assert_eq!(cold.limit, 10.0);
        // The median, which one slow run doesn't move.
        assert_eq!(over(cold, &[9.0, 10.0, 30.0]), None);
        assert!(over(cold, &[9.0, 10.1, 10.2]).is_some());

        let edit = gate("incremental rebuild");
        assert_eq!(edit.limit, 0.1);
        // p95 by nearest rank: the 19th of 20.
        let mut times = vec![0.05; 20];
        times[19] = 0.2;
        assert_eq!(over(edit, &times), None);
        times[18] = 0.2;
        assert!(over(edit, &times).is_some());
    }

    /// Only a slowdown against main fails the job, until the Engine meets
    /// #15's numbers on the Scale Site (#73).
    #[test]
    fn only_more_than_10_percent_slower_than_main_fails() {
        let warm = gate("warm build");
        let main = [1.0; 5];
        assert_eq!(slower(warm, &[1.09; 5], &main), None);
        assert!(slower(warm, &[1.11; 5], &main).is_some());
        // Medians: main's one slow run doesn't hide the slowdown.
        let noisy = [1.0, 1.0, 1.0, 1.0, 9.0];
        assert!(slower(warm, &[1.2; 5], &noisy).is_some());
        // Over the budget, and as fast as main, passes.
        assert_eq!(slower(warm, &[2.5; 5], &[2.5; 5]), None);
        assert!(over(warm, &[2.5; 5]).is_some());
    }

    #[test]
    fn peak_rss_is_never_compared_with_main() {
        let rss = gate("peak RSS");
        assert_eq!(slower(rss, &[900e6], &[500e6]), None);
        assert!(over(rss, &[1.1e9]).is_some());
    }
}
