//! `yuanme dev` (#11, #24): the project's Inputs, the caches and the last
//! good Build, all kept in memory. Each change rebuilds the whole Site
//! (ADR 0011), and the Pages whose files changed are what browsers reload.

mod serve;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::io;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

use tokio::sync::broadcast;
use yuanme_engine::{Build, Cache, Inputs, Mode};

use crate::events::Event;
use crate::watch::{self, Batch};
use crate::{ENGINE_VERSION, Error, Out, load, plural};

/// The port `yuanme dev` tries first, then the next ones up (#21).
const PORT: u16 = 4590;

/// `yuanme dev`: builds, serves on 127.0.0.1 and rebuilds on every change
/// until it is interrupted. `port` is tried alone when it is given.
pub fn run(root: &Path, port: Option<u16>, poll: bool, out: Out) -> Result<u8, Error> {
    let root = root.canonicalize()?;
    let started = Instant::now();
    // Watching first, so a change during the first build isn't missed.
    let (watcher, batches) = watch::watch(&root, poll)
        .map_err(|e| Error::Io(io::Error::other(format!("can't watch the project: {e}"))))?;
    out.event(Event::BuildStarted { command: "dev" });
    let cache = Cache::open(root.join(".yuanme/cache"), ENGINE_VERSION)?;
    let session = Session::start(root.clone(), root.join("dist"), cache)?;
    let snapshot = session.snapshot();
    crate::schemas::write(
        &root.join(".yuanme/schemas"),
        snapshot.build.config.as_ref(),
        snapshot.build.site.as_ref(),
    )?;
    report(&snapshot.build, started, Vec::new(), out);
    session.cache.lock().expect("no build is running").save()?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let ports = match port {
        Some(port) => port..=port,
        None => PORT..=PORT + 100,
    };
    let listener = runtime.block_on(async {
        let mut last = None;
        for port in ports {
            match tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await {
                Ok(listener) => return Ok(listener),
                Err(e) => last = Some(e),
            }
        }
        Err(last.expect("at least one port"))
    })?;
    let address: SocketAddr = listener.local_addr()?;
    let url = format!("http://{address}/");
    let shared = Arc::new(serve::Shared {
        snapshot: RwLock::new(snapshot),
        cache: session.cache.clone(),
        reloads: broadcast::channel(64).0,
        out,
    });
    out.say(&format!("serving {url}"));
    out.event(Event::ServerReady {
        url,
        pid: std::process::id(),
    });

    let builder = shared.clone();
    std::thread::spawn(move || rebuild(session, &batches, &builder));
    runtime.block_on(async {
        axum::serve(listener, serve::router(shared.clone()))
            .with_graceful_shutdown(stopped())
            .await
    })?;
    drop(watcher);
    // A build may be running; the cache is saved once it is done.
    shared.cache.lock().expect("a build doesn't panic").save()?;
    Ok(0)
}

/// Ctrl-C, or a SIGTERM such as `kill` sends.
async fn stopped() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let mut term = signal(SignalKind::terminate()).expect("a SIGTERM handler");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    let _ = tokio::signal::ctrl_c().await;
}

/// Builds once for each batch that changed something a build reads, and
/// serves each build that succeeds.
fn rebuild(mut session: Session, batches: &Receiver<Batch>, shared: &serve::Shared) {
    let out = shared.out;
    let mut rescan = false;
    for mut batch in batches {
        batch.rescan |= std::mem::take(&mut rescan);
        let started = Instant::now();
        match session.refresh(&batch) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(e) => {
                // Such as a file deleted while it was read: the next change
                // walks the whole project again.
                eprintln!(
                    "warning: can't read the project, so yuanme reads it all again on the next change: {e}"
                );
                rescan = true;
                continue;
            }
        }
        out.event(Event::BuildStarted { command: "dev" });
        match session.build() {
            Update::Built(urls) => {
                let snapshot = session.snapshot();
                *shared.snapshot.write().expect("a swap doesn't panic") = snapshot.clone();
                if !urls.is_empty() {
                    let message = serde_json::json!({ "type": "reload", "urls": urls });
                    // No browser connected is no error.
                    let _ = shared.reloads.send(message.to_string());
                }
                report(&snapshot.build, started, urls.clone(), out);
                if !urls.is_empty() {
                    out.event(Event::Reload { urls });
                }
                // After the reload, so it never holds one up.
                let saved = shared.cache.lock().expect("no build is running").save();
                if let Err(e) = saved {
                    eprintln!("warning: can't save the cache: {e}");
                }
            }
            Update::Failed(build) => report(&build, started, Vec::new(), out),
        }
    }
}

/// A build's Diagnostics, `build-finished` and a line for a person.
fn report(build: &Build, started: Instant, urls: Vec<String>, out: Out) {
    out.diagnostics(&build.diagnostics, &build.sources);
    let ms = started.elapsed().as_millis();
    let (errors, warnings) = crate::counts(build);
    match build.failed() {
        true => out.say(&format!(
            "build failed with {} and {}; serving the last build that succeeded",
            plural(errors, "error"),
            plural(warnings, "warning")
        )),
        false => out.say(&format!(
            "built in {ms} ms: {} changed, {}",
            plural(urls.len(), "URL"),
            plural(warnings, "warning")
        )),
    }
    crate::finish(&out, build, started, urls);
}

/// What browsers are served: a Build that succeeded, or the first one,
/// and the Inputs it read, which images are encoded from on request.
pub struct Snapshot {
    pub build: Build,
    pub inputs: Inputs,
}

pub struct Session {
    root: PathBuf,
    /// The output directory of `yuanme build`, which the walk leaves out.
    out: PathBuf,
    inputs: Inputs,
    /// Shared with the requests that encode images.
    pub cache: Arc<Mutex<Cache>>,
    snapshot: Arc<Snapshot>,
}

/// What one change did.
pub enum Update {
    /// The Site's URLs whose files changed, sorted, now served.
    Built(Vec<String>),
    /// The build failed: the last good snapshot is still served.
    Failed(Box<Build>),
}

impl Session {
    /// Loads and builds the project at `root`, which must be canonical,
    /// as the watcher's paths are.
    pub fn start(root: PathBuf, out: PathBuf, cache: Cache) -> io::Result<Session> {
        let inputs = load::load(&root, &out)?;
        let mut cache = cache;
        let build = yuanme_engine::build(&inputs, &mut cache, Mode::Dev);
        let snapshot = Arc::new(Snapshot {
            build,
            inputs: inputs.clone(),
        });
        Ok(Session {
            root,
            out,
            inputs,
            cache: Arc::new(Mutex::new(cache)),
            snapshot,
        })
    }

    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.snapshot.clone()
    }

    /// Reads the files `batch` names again, returning whether anything a
    /// build reads changed.
    pub fn refresh(&mut self, batch: &Batch) -> io::Result<bool> {
        load::refresh(
            &mut self.inputs,
            &self.root,
            &self.out,
            &batch.paths,
            batch.rescan,
        )
    }

    pub fn build(&mut self) -> Update {
        let mut cache = self.cache.lock().expect("a build doesn't panic");
        let build = yuanme_engine::build(&self.inputs, &mut cache, Mode::Dev);
        drop(cache);
        if build.failed() {
            return Update::Failed(Box::new(build));
        }
        let urls = changed_urls(&self.snapshot.build, &build);
        self.snapshot = Arc::new(Snapshot {
            build,
            inputs: self.inputs.clone(),
        });
        Update::Built(urls)
    }
}

/// The URLs of the files that `now` has and `before` hadn't, or had with
/// other bytes, or that `now` no longer has.
fn changed_urls(before: &Build, now: &Build) -> Vec<String> {
    let mut paths: BTreeSet<&str> = BTreeSet::new();
    for (path, output) in &now.files {
        if before.files.get(path).is_none_or(|o| o.hash != output.hash) {
            paths.insert(path);
        }
    }
    for path in before.files.keys() {
        if !now.files.contains_key(path) {
            paths.insert(path);
        }
    }
    let host = now.config.as_ref().map(|c| &c.host);
    let urls: BTreeSet<String> = paths
        .into_iter()
        .filter_map(|p| host.map_or(Some(p), |h| yuanme_engine::served(h, p)))
        .map(crate::url_of)
        .collect();
    urls.into_iter().collect()
}
