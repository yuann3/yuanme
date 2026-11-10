//! The project on disk: reading its Inputs, building them in memory as
//! every command does, and writing a Build's files to an output directory.

pub(crate) mod load;
pub(crate) mod sync;

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use yuanme_content::{Config, Site, url_path};
use yuanme_diag::Level;
use yuanme_engine::{Build, Cache, Mode};

use crate::ENGINE_VERSION;
use crate::out::Out;

pub(crate) struct Run {
    pub(crate) build: Build,
    /// The output directory: `--out`, or else the adapter's.
    pub(crate) out: PathBuf,
    pub(crate) load: Duration,
    pub(crate) took: Duration,
}

/// One build in memory. `out` is `--out`, which the walk leaves out; the
/// adapter's output directory is `dist` or hidden. Only `yuanme build`
/// encodes images and measures the Site budgets: every other command builds
/// as `check` does.
pub(crate) fn build(
    root: &Path,
    out: Option<&Path>,
    mode: Mode,
    strict: bool,
    strict_budgets: bool,
) -> io::Result<Run> {
    let started = Instant::now();
    let dist = root.join("dist");
    let inputs = load::load(root, &[out.unwrap_or(&dist).to_path_buf()])?;
    let load = started.elapsed();
    let mut cache = Cache::open(root.join(".yuanme/cache"), ENGINE_VERSION)?;
    let mut build = yuanme_engine::build(&inputs, &mut cache, mode);
    cache.save()?;
    let out = match (out, &build.config) {
        (Some(out), _) => out.to_path_buf(),
        (None, Some(config)) => root.join(config.host.adapter.out_dir()),
        (None, None) => dist,
    };
    // The Worker's configuration, which isn't an input.
    if let Ok(text) = fs::read_to_string(root.join("wrangler.jsonc")) {
        let dir = out.strip_prefix(root).unwrap_or(&out);
        let dir = dir.to_string_lossy().replace('\\', "/");
        yuanme_engine::wrangler(&mut build, &text, &dir);
    }
    let budgets_strict = build
        .config
        .as_ref()
        .and_then(|c| c.budgets.as_ref())
        .is_some_and(|b| b.strict);
    for d in &mut build.diagnostics {
        let budget = d.code.name.starts_with("budget/");
        let why = match d.level {
            Level::Warning if strict => "`--strict` makes every warning an error",
            Level::Warning if budget && strict_budgets => {
                "`--strict-budgets` makes every warning about a Site budget an error"
            }
            Level::Warning if budget && budgets_strict => {
                "`budgets.strict` makes every warning about a Site budget an error"
            }
            _ => continue,
        };
        *d = d.clone().note(why);
        d.level = Level::Error;
    }
    Ok(Run {
        build,
        out,
        load,
        took: started.elapsed() - load,
    })
}

/// `path` from `root`, with its `.` and `..` resolved, since
/// `Path::starts_with` takes `..` as a name. `root` is canonical, so a
/// `..` leaves the folder the OS would.
pub(crate) fn resolve(root: &Path, path: &Path) -> PathBuf {
    let mut dir = PathBuf::new();
    for part in root.join(path).components() {
        match part {
            Component::ParentDir => drop(dir.pop()),
            Component::CurDir => {}
            part => dir.push(part),
        }
    }
    dir
}

/// An output path's URL: `blog/pew/index.html` is `/blog/pew/`.
pub(crate) fn url_of(output: &str) -> String {
    let path = match output.strip_suffix("index.html") {
        Some(dir) if dir.is_empty() || dir.ends_with('/') => dir,
        _ => output,
    };
    url_path(&format!("/{path}"))
}

/// Site config, from a build so that its Diagnostics are the build's.
pub(crate) fn config_or_report<'b>(run: &'b Run, out: &Out) -> Option<&'b Config> {
    if run.build.config.is_none() {
        out.diagnostics(&run.build.diagnostics, &run.build.sources);
    }
    run.build.config.as_ref()
}

/// The Site, or its errors reported. Template and asset errors don't stop
/// it: they are `check`'s to report.
pub(crate) fn site_or_report<'b>(run: &'b Run, out: &Out) -> Option<(&'b Config, &'b Site)> {
    match (&run.build.config, &run.build.site) {
        (Some(config), Some(site)) => Some((config, site)),
        _ => {
            out.diagnostics(&run.build.diagnostics, &run.build.sources);
            None
        }
    }
}
