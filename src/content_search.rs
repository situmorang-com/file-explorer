use crossbeam_channel::{bounded, Receiver};
use grep_regex::RegexMatcherBuilder;
use grep_searcher::sinks::UTF8;
use grep_searcher::{BinaryDetection, SearcherBuilder};
use ignore::WalkBuilder;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct ContentMatch {
    pub path: PathBuf,
    pub line: u64,
    pub text: String,
}

pub struct ContentSearch {
    rx: Receiver<ContentMatch>,
    cancel: Arc<AtomicBool>,
    pub matches: Vec<ContentMatch>,
    pub done: bool,
    pub scanned: u64,
}

impl ContentSearch {
    pub fn spawn(root: PathBuf, query: String, show_hidden: bool, max: usize) -> Self {
        let (tx, rx) = bounded::<ContentMatch>(1024);
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_c = cancel.clone();

        std::thread::spawn(move || {
            let matcher = match RegexMatcherBuilder::new()
                .case_smart(true)
                .fixed_strings(true)
                .build(&query)
            {
                Ok(m) => m,
                Err(_) => return,
            };
            let walker = WalkBuilder::new(&root)
                .hidden(!show_hidden)
                .git_ignore(true)
                .git_global(true)
                .git_exclude(true)
                .threads(num_cpus())
                .build_parallel();

            let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

            walker.run(|| {
                let tx = tx.clone();
                let matcher = matcher.clone();
                let cancel = cancel_c.clone();
                let count = count.clone();
                Box::new(move |result| {
                    if cancel.load(Ordering::Relaxed) {
                        return ignore::WalkState::Quit;
                    }
                    let dent = match result {
                        Ok(d) => d,
                        Err(_) => return ignore::WalkState::Continue,
                    };
                    if !dent.file_type().map(|t| t.is_file()).unwrap_or(false) {
                        return ignore::WalkState::Continue;
                    }
                    // Skip giant files.
                    if let Ok(meta) = dent.metadata() {
                        if meta.len() > 4 * 1024 * 1024 {
                            return ignore::WalkState::Continue;
                        }
                    }

                    let path = dent.path().to_path_buf();
                    let mut searcher = SearcherBuilder::new()
                        .binary_detection(BinaryDetection::quit(0))
                        .line_number(true)
                        .build();
                    let mut local = 0u32;
                    let _ = searcher.search_path(
                        &matcher,
                        &path,
                        UTF8(|lineno, line| {
                            let trimmed = line.trim_end_matches('\n');
                            let cut = if trimmed.len() > 240 {
                                &trimmed[..240]
                            } else {
                                trimmed
                            };
                            let _ = tx.send(ContentMatch {
                                path: path.clone(),
                                line: lineno,
                                text: cut.to_string(),
                            });
                            local += 1;
                            if count.fetch_add(1, Ordering::Relaxed) > max {
                                cancel.store(true, Ordering::Relaxed);
                                return Ok(false);
                            }
                            Ok(local < 5) // up to 5 hits per file
                        }),
                    );

                    ignore::WalkState::Continue
                })
            });
            // tx drops when walker finishes; receiver iter ends.
        });

        ContentSearch {
            rx,
            cancel,
            matches: Vec::new(),
            done: false,
            scanned: 0,
        }
    }

    /// Drain channel into `matches`. Returns true if anything was added.
    pub fn drain(&mut self) -> bool {
        let mut changed = false;
        loop {
            match self.rx.try_recv() {
                Ok(m) => {
                    self.matches.push(m);
                    self.scanned += 1;
                    changed = true;
                }
                Err(crossbeam_channel::TryRecvError::Empty) => break,
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    self.done = true;
                    break;
                }
            }
        }
        changed
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
