use ignore::WalkBuilder;
use nucleo::{Config, Nucleo, Utf32String};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub struct PathEntry {
    pub path: PathBuf,
    pub display: String,
    pub is_dir: bool,
}

pub struct Index {
    pub nucleo: Nucleo<Arc<PathEntry>>,
    pub done: Arc<AtomicBool>,
}

impl Index {
    pub fn new(notify: Arc<dyn Fn() + Send + Sync>) -> Self {
        let nucleo = Nucleo::new(
            Config::DEFAULT.match_paths(),
            notify,
            None,
            1, // one column: the path
        );
        Self {
            nucleo,
            done: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::Relaxed)
    }

    /// Spawn a background walker that streams entries into the matcher.
    /// `show_hidden = true` includes dot-files; `false` hides them (default Finder behavior).
    pub fn spawn_walk(&self, root: PathBuf, show_hidden: bool) {
        let injector = self.nucleo.injector();
        let done = self.done.clone();
        std::thread::spawn(move || {
            let walker = WalkBuilder::new(&root)
                .hidden(!show_hidden)
                .git_ignore(true)
                .git_global(true)
                .git_exclude(true)
                .threads(num_cpus())
                .build_parallel();

            walker.run(|| {
                let injector = injector.clone();
                let root = root.clone();
                Box::new(move |result| {
                    if let Ok(dent) = result {
                        let path = dent.path().to_path_buf();
                        let is_dir = dent.file_type().map(|t| t.is_dir()).unwrap_or(false);
                        let display = path
                            .strip_prefix(&root)
                            .unwrap_or(&path)
                            .to_string_lossy()
                            .into_owned();
                        let entry = Arc::new(PathEntry {
                            path,
                            display,
                            is_dir,
                        });
                        injector.push(entry, |e, cols| {
                            cols[0] = Utf32String::from(e.display.as_str());
                        });
                    }
                    ignore::WalkState::Continue
                })
            });
            done.store(true, Ordering::Relaxed);
        });
    }

    pub fn set_query(&mut self, query: &str) {
        self.nucleo.pattern.reparse(
            0,
            query,
            nucleo::pattern::CaseMatching::Smart,
            nucleo::pattern::Normalization::Smart,
            false,
        );
    }

    pub fn tick(&mut self) -> bool {
        let status = self.nucleo.tick(10);
        status.changed
    }

    pub fn results(&self, max: u32) -> Vec<Arc<PathEntry>> {
        let snap = self.nucleo.snapshot();
        let n = snap.matched_item_count().min(max);
        snap.matched_items(0..n)
            .map(|m| m.data.clone())
            .collect()
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
