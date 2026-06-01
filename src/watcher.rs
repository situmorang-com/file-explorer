use crossbeam_channel::{unbounded, Receiver};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Watch a directory tree and emit a single coalesced "dirty" tick at most once
/// per `debounce` window after activity. Returns the receiver and the live
/// watcher (kept alive by the caller).
pub struct FsWatcher {
    pub dirty_rx: Receiver<()>,
    _watcher: RecommendedWatcher,
}

impl FsWatcher {
    pub fn spawn(root: PathBuf, debounce: Duration) -> Option<Self> {
        let (raw_tx, raw_rx) = unbounded::<()>();
        let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res| {
            if let Ok(_event) = res {
                let _ = raw_tx.send(());
            }
        })
        .ok()?;
        if watcher.watch(&root, RecursiveMode::Recursive).is_err() {
            return None;
        }

        let (dirty_tx, dirty_rx) = unbounded::<()>();
        std::thread::spawn(move || {
            let mut last: Option<Instant> = None;
            loop {
                let wait = match last {
                    Some(t) => {
                        let elapsed = t.elapsed();
                        if elapsed >= debounce {
                            let _ = dirty_tx.send(());
                            last = None;
                            Duration::from_millis(500)
                        } else {
                            debounce - elapsed
                        }
                    }
                    None => Duration::from_secs(60),
                };
                match raw_rx.recv_timeout(wait) {
                    Ok(_) => {
                        last = Some(Instant::now());
                        // drain a burst
                        while raw_rx.try_recv().is_ok() {}
                    }
                    Err(_) => {
                        if let Some(t) = last {
                            if t.elapsed() >= debounce {
                                let _ = dirty_tx.send(());
                                last = None;
                            }
                        }
                    }
                }
            }
        });

        Some(FsWatcher {
            dirty_rx,
            _watcher: watcher,
        })
    }
}
