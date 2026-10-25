//! The watcher for `yuanme dev` (#11): notify's events, coalesced into
//! batches that each start one build.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use notify::{RecursiveMode, Watcher};

/// A batch ends once no event has come for this long. Editors save in
/// bursts: a temporary file, a rename, a chmod.
const QUIET: Duration = Duration::from_millis(15);
/// …or this long after its first event, so that a stream of events, such
/// as a `git checkout`, still builds.
const CAP: Duration = Duration::from_millis(50);
/// How often `--poll` looks at every file.
const POLL: Duration = Duration::from_millis(100);

/// What changed since the last batch.
#[derive(Debug, Default)]
pub struct Batch {
    /// Absolute, as the watcher reports them: files and folders created,
    /// changed, removed or renamed, without saying which.
    pub paths: BTreeSet<PathBuf>,
    /// The watcher lost events, so only walking the whole project again
    /// is sure to see them.
    pub rescan: bool,
}

/// One event, as notify reports it.
type Raw = notify::Result<notify::Event>;

/// Watches the project at `root`, which must be canonical, as the paths
/// the operating system reports are. `poll` looks at every file every
/// 100 ms instead, for file systems that report no events, such as some
/// network and container mounts. Batches arrive while the watcher lives.
pub fn watch(
    root: &Path,
    poll: bool,
) -> notify::Result<(Box<dyn Watcher + Send>, Receiver<Batch>)> {
    let (send, events) = mpsc::channel::<Raw>();
    let mut watcher: Box<dyn Watcher + Send> = if poll {
        let config = notify::Config::default().with_poll_interval(POLL);
        Box::new(notify::PollWatcher::new(send, config)?)
    } else {
        Box::new(notify::recommended_watcher(send)?)
    };
    watcher.watch(root, RecursiveMode::Recursive)?;
    let (batches, receiver) = mpsc::channel();
    std::thread::spawn(move || coalesce(events, batches));
    Ok((watcher, receiver))
}

/// Groups `events` into batches, each ended by [`QUIET`] or [`CAP`].
fn coalesce(events: Receiver<Raw>, batches: Sender<Batch>) {
    while let Ok(first) = events.recv() {
        if !changes(&first) {
            continue;
        }
        let started = Instant::now();
        let mut batch = Batch::default();
        add(&mut batch, first);
        let mut last = started;
        loop {
            let deadline = (last + QUIET).min(started + CAP);
            match events.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(event) if !changes(&event) => {}
                Ok(event) => {
                    add(&mut batch, event);
                    last = Instant::now();
                }
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => {
                    let _ = batches.send(batch);
                    return;
                }
            }
        }
        if batches.send(batch).is_err() {
            return;
        }
    }
}

/// Whether an event can mean a change. inotify reports every file and
/// folder opened, and reading one to see what changed opens it again.
fn changes(event: &Raw) -> bool {
    use notify::EventKind;
    use notify::event::{AccessKind, AccessMode};
    match event {
        Ok(event) => match event.kind {
            EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
            EventKind::Access(_) => false,
            _ => true,
        },
        Err(_) => true,
    }
}

fn add(batch: &mut Batch, event: Raw) {
    match event {
        Ok(event) => {
            batch.rescan |= event.need_rescan();
            batch.paths.extend(event.paths);
        }
        Err(e) => {
            // An event lost to an error is found by walking again.
            eprintln!(
                "warning: the file watcher failed, so yuanme reads the whole project again: {e}"
            );
            batch.rescan = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(path: &str) -> Raw {
        Ok(notify::Event::new(notify::EventKind::Any).add_path(PathBuf::from(path)))
    }

    #[test]
    fn a_burst_is_one_batch_and_a_stream_is_cut_at_the_cap() {
        let (send, events) = mpsc::channel();
        let (batches, received) = mpsc::channel();
        std::thread::spawn(move || coalesce(events, batches));

        for path in ["/p/a.md", "/p/.a.md.swp", "/p/a.md"] {
            send.send(event(path)).unwrap();
        }
        // An open, as reading a file to hash it makes, is no change.
        let open = notify::EventKind::Access(notify::event::AccessKind::Open(
            notify::event::AccessMode::Read,
        ));
        send.send(Ok(notify::Event::new(open).add_path("/p/b.md".into())))
            .unwrap();
        let batch = received.recv().unwrap();
        assert_eq!(
            batch.paths,
            BTreeSet::from(["/p/a.md".into(), "/p/.a.md.swp".into()])
        );
        assert!(!batch.rescan);

        // An event every 5 ms for 400 ms never leaves 15 ms quiet, but
        // builds at least every 50 ms.
        let started = Instant::now();
        while started.elapsed() < Duration::from_millis(400) {
            send.send(event("/p/b.md")).unwrap();
            std::thread::sleep(Duration::from_millis(5));
        }
        drop(send);
        let cut = received.iter().count();
        assert!(cut >= 3, "{cut} batches");

        let flag =
            notify::Event::new(notify::EventKind::Other).set_flag(notify::event::Flag::Rescan);
        let mut batch = Batch::default();
        add(&mut batch, Ok(flag));
        assert!(batch.rescan);
    }

    /// `--poll` sees a change no event announces.
    #[test]
    fn polling_finds_a_changed_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let file = root.join("a.md");
        std::fs::write(&file, "a").unwrap();
        let (_watcher, batches) = watch(&root, true).unwrap();
        // Polling compares modification times, which some file systems
        // keep only to the second.
        std::thread::sleep(Duration::from_millis(1100));
        std::fs::write(&file, "b").unwrap();
        let batch = batches.recv_timeout(Duration::from_secs(5)).unwrap();
        assert!(batch.paths.contains(&file), "{batch:?}");
    }
}
