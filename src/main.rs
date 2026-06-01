mod content_search;
mod highlight;
mod index;
mod settings;
mod updater;
mod watcher;

use content_search::{ContentMatch, ContentSearch};
use watcher::FsWatcher;
use gpui::{
    div, prelude::*, px, rgb, rgba, uniform_list, App, Bounds, ClipboardItem, Context, FocusHandle,
    Focusable, KeyBinding, MouseButton, Rgba, ScrollStrategy, SharedString, UniformListScrollHandle,
    Window, WindowBackgroundAppearance, WindowBounds, WindowOptions,
};
use gpui_platform::application;
use index::{Index, PathEntry};
use nucleo::Matcher;
use std::path::PathBuf;
use std::sync::Arc;

// Catppuccin Mocha — modern dark palette used across Linear/Raycast-inspired UIs.
// Colors prefixed with `T_` are translucent (alpha-channel) for vibrancy layers.
mod theme {
    use gpui::{rgb, rgba, Rgba};
    pub fn c(v: u32) -> Rgba {
        rgb(v)
    }
    pub fn t(v: u32) -> Rgba {
        rgba(v)
    }
    // Solid (panels that need contrast)
    pub const TEXT: u32 = 0xcdd6f4;
    pub const SUBTEXT: u32 = 0xa6adc8;
    pub const MUTED: u32 = 0x7f849c;
    pub const ACCENT: u32 = 0x89b4fa;
    pub const DIR: u32 = 0xf9e2af;
    pub const ACCENT_BAR: u32 = 0xcba6f7;
    pub const MATCH: u32 = 0xf5c2e7; // pink for matched chars
    pub const BORDER: u32 = 0x313244;
    pub const SEL: u32 = 0x313244;
    // Translucent (window vibrancy layers; lower 8 bits = alpha 0-255)
    pub const WINDOW_BG: u32 = 0x11111b40; // ~0.25 alpha — let the desktop through
    pub const SURFACE: u32 = 0x18182526; // ~0.15
    pub const SURFACE_ALT: u32 = 0x1e1e2e40; // ~0.25
    pub const SIDEBAR: u32 = 0x18182559; // ~0.35
}

fn file_icon(is_dir: bool, name: &str) -> &'static str {
    if is_dir {
        // Special-cased common dotfile / project dirs
        return match name {
            ".git" => "🌳",
            ".github" => "🐙",
            "node_modules" => "📦",
            "target" | "build" | "dist" | "out" => "🛠️",
            "src" | "lib" => "📂",
            "assets" | "static" | "public" | "images" | "img" => "🖼️",
            "docs" | "doc" => "📚",
            "tests" | "test" | "__tests__" => "🧪",
            "scripts" => "📜",
            "Desktop" => "🖥️",
            "Downloads" => "⬇️",
            "Documents" => "📑",
            "Applications" => "🚀",
            "Pictures" => "🖼️",
            "Movies" => "🎬",
            "Music" => "🎵",
            _ => "📁",
        };
    }
    let ext = name.rsplit('.').next().unwrap_or("");
    match ext.to_ascii_lowercase().as_str() {
        // Code
        "rs" => "🦀",
        "py" => "🐍",
        "go" => "🐹",
        "rb" => "💎",
        "swift" => "🦅",
        "ts" | "tsx" => "🟦",
        "js" | "jsx" | "mjs" | "cjs" => "🟨",
        "html" | "htm" => "🌐",
        "css" | "scss" | "sass" | "less" => "🎨",
        "java" | "kt" | "scala" => "☕",
        "c" | "h" | "cpp" | "hpp" | "cc" | "cxx" => "🔧",
        "lua" => "🌙",
        "vim" => "📝",
        "el" => "🧠",
        "sh" | "zsh" | "bash" | "fish" => "🐚",
        // Config / data
        "toml" | "ini" | "cfg" | "conf" => "⚙️",
        "yaml" | "yml" => "📋",
        "json" | "json5" | "jsonc" => "🧾",
        "xml" => "📰",
        "csv" | "tsv" => "📊",
        "sql" => "🗄️",
        "env" => "🔐",
        "lock" => "🔒",
        "gitignore" | "gitattributes" => "🌳",
        // Docs
        "md" | "mdx" | "rst" => "📝",
        "txt" | "log" => "📃",
        "pdf" => "📕",
        // Office
        "docx" | "doc" | "pages" => "📄",
        "xlsx" | "xls" | "numbers" => "📊",
        "pptx" | "ppt" | "key" => "📽️",
        // Media — images
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tiff" | "tif" | "heic" | "avif" => {
            "🖼️"
        }
        "svg" => "🎨",
        "ico" | "icns" => "🪟",
        "psd" | "ai" | "sketch" | "fig" => "🎨",
        // Media — av
        "mp4" | "mov" | "m4v" | "mkv" | "webm" | "avi" | "wmv" => "🎬",
        "mp3" | "m4a" | "wav" | "aiff" | "flac" | "ogg" => "🎵",
        // Archives / binaries
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => "📦",
        "dmg" | "iso" | "img" => "💿",
        "app" | "exe" | "msi" | "appimage" => "🚀",
        "deb" | "rpm" | "pkg" => "📦",
        _ => "📄",
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", v, UNITS[u])
    }
}

/// Empty entity used as a drag payload — gpui requires one. The real "payload"
/// is the path we copy to the clipboard so the user can paste it with ⌘V in
/// Finder, since external NSDrag isn't reachable through gpui's window API yet.
struct DragGhost;

impl Render for DragGhost {
    fn render(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .px_3()
            .py_1p5()
            .rounded_md()
            .bg(rgba(0xcba6f7cc))
            .text_size(px(11.0))
            .text_color(rgb(0x11111b))
            .child("Path copied — ⌘V in Finder")
    }
}

struct Pin {
    label: &'static str,
    icon: &'static str,
    path: PathBuf,
}

fn pins() -> Vec<Pin> {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    vec![
        Pin {
            label: "Home",
            icon: "",
            path: home.clone(),
        },
        Pin {
            label: "Documents",
            icon: "",
            path: home.join("Documents"),
        },
        Pin {
            label: "Downloads",
            icon: "",
            path: home.join("Downloads"),
        },
        Pin {
            label: "Desktop",
            icon: "",
            path: home.join("Desktop"),
        },
        Pin {
            label: "Applications",
            icon: "",
            path: PathBuf::from("/Applications"),
        },
        Pin {
            label: "Projects",
            icon: "",
            path: home.join("code"),
        },
    ]
}

const VIEWPORT_ROWS: usize = 80;
const RECENTS_MAX: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Search,
    PathBar,
    Settings,
    Rename,
    Create,
    BatchRename,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CreateKind {
    Folder,
    File,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SearchMode {
    Filename,
    Content,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SortMode {
    Relevance,
    Name,
    Size,
    Modified,
}

impl SortMode {
    fn label(self) -> &'static str {
        match self {
            SortMode::Relevance => "relevance",
            SortMode::Name => "name",
            SortMode::Size => "size",
            SortMode::Modified => "modified",
        }
    }
    fn next(self) -> Self {
        match self {
            SortMode::Relevance => SortMode::Name,
            SortMode::Name => SortMode::Size,
            SortMode::Size => SortMode::Modified,
            SortMode::Modified => SortMode::Relevance,
        }
    }
}

#[derive(Clone)]
struct TabSnapshot {
    root: PathBuf,
    query: String,
    search_mode: SearchMode,
    sort: SortMode,
    selected: usize,
    viewport_start: usize,
    show_hidden: bool,
}

#[derive(Clone)]
enum UndoAction {
    Rename { from: PathBuf, to: PathBuf },
    Move { pairs: Vec<(PathBuf, PathBuf)> },
    Trash { count: usize },
}

#[derive(Clone)]
struct MenuState {
    x: f32,
    y: f32,
    path: PathBuf,
    is_dir: bool,
}

#[derive(Clone)]
struct TreeNode {
    path: PathBuf,
    expanded: bool,
    children: Option<Vec<TreeNode>>,
}

impl TreeNode {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            expanded: false,
            children: None,
        }
    }

    fn load_children(&mut self) {
        if self.children.is_some() {
            return;
        }
        let mut kids: Vec<TreeNode> = std::fs::read_dir(&self.path)
            .ok()
            .into_iter()
            .flat_map(|rd| rd.filter_map(|r| r.ok()))
            .filter(|de| {
                de.file_name()
                    .to_str()
                    .map(|s| !s.starts_with('.'))
                    .unwrap_or(true)
            })
            .map(|de| TreeNode::new(de.path()))
            .collect();
        kids.sort_by(|a, b| {
            let ad = a.path.is_dir();
            let bd = b.path.is_dir();
            match (ad, bd) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.path.file_name().cmp(&b.path.file_name()),
            }
        });
        self.children = Some(kids);
    }

    fn toggle_at(&mut self, target: &std::path::Path) -> bool {
        if self.path == target {
            if self.path.is_dir() {
                if !self.expanded {
                    self.load_children();
                }
                self.expanded = !self.expanded;
            }
            return true;
        }
        if let Some(kids) = self.children.as_mut() {
            for k in kids {
                if k.toggle_at(target) {
                    return true;
                }
            }
        }
        false
    }
}

/// Pre-flatten the expanded tree into render rows: (depth, path, is_dir, expanded).
fn flatten_tree(root: &TreeNode, depth: usize, out: &mut Vec<(usize, PathBuf, bool, bool)>) {
    let is_dir = root.path.is_dir();
    out.push((depth, root.path.clone(), is_dir, root.expanded));
    if root.expanded {
        if let Some(kids) = root.children.as_ref() {
            for k in kids {
                flatten_tree(k, depth + 1, out);
            }
        }
    }
}

struct Explorer {
    tabs: Vec<TabSnapshot>,
    active: usize,
    rename_input: String,
    rename_cursor: usize,
    rename_anchor: Option<usize>,
    batch_input: String,
    batch_cursor: usize,
    batch_anchor: Option<usize>,
    create_input: String,
    create_cursor: usize,
    create_anchor: Option<usize>,
    path_cursor: usize,
    path_anchor: Option<usize>,
    create_kind: CreateKind,
    menu: Option<MenuState>,
    tree: Option<TreeNode>,
    tree_visible: bool,
    undo_stack: Vec<UndoAction>,
    caret_on: bool,
    scroll_handle: UniformListScrollHandle,
    root: PathBuf,
    index: Index,
    matcher: Matcher,
    query: String,
    path_input: String,
    mode: Mode,
    search_mode: SearchMode,
    sort: SortMode,
    results: Vec<Arc<PathEntry>>,
    content: Option<ContentSearch>,
    selected: usize,
    marked: std::collections::BTreeSet<usize>,
    last_anchor: usize,
    viewport_start: usize,
    show_hidden: bool,
    recents: Vec<PathBuf>,
    bookmarks: Vec<PathBuf>,
    toast: Option<String>,
    watcher: Option<FsWatcher>,
    focus: FocusHandle,
}

impl Explorer {
    fn new(root: PathBuf, cx: &mut Context<Self>) -> Self {
        let settings = settings::load();
        let notifier = Arc::new(|| {}) as Arc<dyn Fn() + Send + Sync>;
        let index = Index::new(notifier);
        let show_hidden = settings.show_hidden;
        index.spawn_walk(root.clone(), show_hidden);

        let focus = cx.focus_handle();

        // Caret blink — 500ms toggle, only repaints when an input mode is active.
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(500))
                .await;
            let _ = this.update(cx, |this, cx| {
                if matches!(
                    this.mode,
                    Mode::PathBar | Mode::Rename | Mode::Create | Mode::BatchRename
                ) {
                    this.caret_on = !this.caret_on;
                    cx.notify();
                } else if !this.caret_on {
                    this.caret_on = true;
                }
            });
        })
        .detach();

        // Background update check (no-op unless FILE_EXPLORER_UPDATE_REPO is set).
        cx.spawn(async move |this, cx| {
            let info = cx
                .background_executor()
                .spawn(async move { updater::check_for_updates() })
                .await;
            if let Some(info) = info {
                let _ = this.update(cx, |this, cx| {
                    this.flash(
                        format!("Update available: {} → {}", info.current, info.latest),
                        cx,
                    );
                    // Open the release page so the user can grab it.
                    let _ = std::process::Command::new("open").arg(info.url).spawn();
                });
            }
        })
        .detach();

        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(33))
                .await;
            let _ = this.update(cx, |this, cx| {
                let mut changed = false;
                if this.index.tick() {
                    this.pull_results();
                    changed = true;
                }
                if let Some(c) = this.content.as_mut() {
                    if c.drain() {
                        changed = true;
                    }
                }
                if let Some(w) = this.watcher.as_ref() {
                    if w.dirty_rx.try_recv().is_ok() {
                        let root = this.root.clone();
                        this.rewalk(root, cx);
                    }
                }
                if changed {
                    cx.notify();
                }
            });
        })
        .detach();

        let recents = load_recents();
        let bookmarks = load_bookmarks();
        let watcher = FsWatcher::spawn(root.clone(), std::time::Duration::from_millis(400));

        let initial_tab = TabSnapshot {
            root: root.clone(),
            query: String::new(),
            search_mode: SearchMode::Filename,
            sort: match settings.sort.as_str() {
                "name" => SortMode::Name,
                "size" => SortMode::Size,
                "modified" => SortMode::Modified,
                _ => SortMode::Relevance,
            },
            selected: 0,
            viewport_start: 0,
            show_hidden,
        };

        Self {
            tabs: vec![initial_tab],
            active: 0,
            rename_input: String::new(),
            rename_cursor: 0,
            rename_anchor: None,
            batch_input: String::new(),
            batch_cursor: 0,
            batch_anchor: None,
            create_input: String::new(),
            create_cursor: 0,
            create_anchor: None,
            path_cursor: 0,
            path_anchor: None,
            create_kind: CreateKind::Folder,
            menu: None,
            tree: {
                let mut t = TreeNode::new(root.clone());
                t.load_children();
                t.expanded = true;
                Some(t)
            },
            tree_visible: false,
            undo_stack: load_undo(),
            caret_on: true,
            scroll_handle: UniformListScrollHandle::new(),
            root: root.clone(),
            index,
            matcher: Matcher::new(nucleo::Config::DEFAULT.match_paths()),
            query: String::new(),
            path_input: String::new(),
            mode: Mode::Search,
            search_mode: SearchMode::Filename,
            sort: match settings.sort.as_str() {
                "name" => SortMode::Name,
                "size" => SortMode::Size,
                "modified" => SortMode::Modified,
                _ => SortMode::Relevance,
            },
            results: Vec::new(),
            content: None,
            selected: 0,
            marked: std::collections::BTreeSet::new(),
            last_anchor: 0,
            viewport_start: 0,
            show_hidden,
            recents,
            bookmarks,
            toast: None,
            watcher,
            focus,
        }
    }

    fn snapshot_active(&self) -> TabSnapshot {
        TabSnapshot {
            root: self.root.clone(),
            query: self.query.clone(),
            search_mode: self.search_mode,
            sort: self.sort,
            selected: self.selected,
            viewport_start: self.viewport_start,
            show_hidden: self.show_hidden,
        }
    }

    fn sync_active_snapshot(&mut self) {
        if let Some(t) = self.tabs.get_mut(self.active) {
            *t = TabSnapshot {
                root: self.root.clone(),
                query: self.query.clone(),
                search_mode: self.search_mode,
                sort: self.sort,
                selected: self.selected,
                viewport_start: self.viewport_start,
                show_hidden: self.show_hidden,
            };
        }
    }

    fn persist_session(&mut self) {
        self.sync_active_snapshot();
        save_session(&self.tabs);
    }

    fn restore_snapshot(&mut self, s: TabSnapshot, cx: &mut Context<Self>) {
        if let Some(c) = self.content.as_ref() {
            c.cancel();
        }
        let notifier = Arc::new(|| {}) as Arc<dyn Fn() + Send + Sync>;
        self.show_hidden = s.show_hidden;
        self.index = Index::new(notifier);
        self.index.spawn_walk(s.root.clone(), s.show_hidden);
        self.watcher = FsWatcher::spawn(s.root.clone(), std::time::Duration::from_millis(400));
        self.root = s.root.clone();
        self.query = s.query.clone();
        self.index.set_query(&self.query);
        self.search_mode = s.search_mode;
        self.sort = s.sort;
        self.results.clear();
        self.content = None;
        self.selected = s.selected;
        self.viewport_start = s.viewport_start;
        self.marked.clear();
        self.last_anchor = 0;
        if self.search_mode == SearchMode::Content {
            self.restart_content_search();
        }
        cx.notify();
    }

    fn new_tab(&mut self, cx: &mut Context<Self>) {
        self.tabs[self.active] = self.snapshot_active();
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let snap = TabSnapshot {
            root: home,
            query: String::new(),
            search_mode: SearchMode::Filename,
            sort: self.sort,
            selected: 0,
            viewport_start: 0,
            show_hidden: self.show_hidden,
        };
        self.tabs.push(snap.clone());
        self.active = self.tabs.len() - 1;
        self.restore_snapshot(snap, cx);
        self.persist_session();
    }

    fn close_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.remove(self.active);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
        let snap = self.tabs[self.active].clone();
        self.restore_snapshot(snap, cx);
        self.persist_session();
    }

    fn switch_tab(&mut self, idx: usize, cx: &mut Context<Self>) {
        if idx == self.active || idx >= self.tabs.len() {
            return;
        }
        self.tabs[self.active] = self.snapshot_active();
        self.active = idx;
        let snap = self.tabs[idx].clone();
        self.restore_snapshot(snap, cx);
        self.persist_session();
    }

    fn next_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            return;
        }
        let next = (self.active + 1) % self.tabs.len();
        self.switch_tab(next, cx);
    }

    fn prev_tab(&mut self, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            return;
        }
        let prev = (self.active + self.tabs.len() - 1) % self.tabs.len();
        self.switch_tab(prev, cx);
    }

    fn toggle_bookmark(&mut self, cx: &mut Context<Self>) {
        let p = self.root.clone();
        if let Some(idx) = self.bookmarks.iter().position(|b| b == &p) {
            self.bookmarks.remove(idx);
            self.flash("Removed bookmark", cx);
        } else {
            self.bookmarks.insert(0, p);
            self.flash("Bookmarked", cx);
        }
        save_bookmarks(&self.bookmarks);
        cx.notify();
    }

    fn marked_paths(&self) -> Vec<PathBuf> {
        match self.search_mode {
            SearchMode::Filename => self
                .marked
                .iter()
                .filter_map(|&i| self.results.get(i).map(|e| e.path.clone()))
                .collect(),
            SearchMode::Content => self
                .marked
                .iter()
                .filter_map(|&i| {
                    self.content
                        .as_ref()
                        .and_then(|c| c.matches.get(i).map(|m| m.path.clone()))
                })
                .collect(),
        }
    }

    fn toggle_mark(&mut self, i: usize) {
        if !self.marked.remove(&i) {
            self.marked.insert(i);
        }
        self.last_anchor = i;
    }

    fn mark_range(&mut self, to: usize) {
        let (a, b) = if self.last_anchor <= to {
            (self.last_anchor, to)
        } else {
            (to, self.last_anchor)
        };
        for i in a..=b {
            self.marked.insert(i);
        }
    }

    fn clear_marks(&mut self, cx: &mut Context<Self>) {
        self.marked.clear();
        cx.notify();
    }

    fn select_all(&mut self, cx: &mut Context<Self>) {
        self.marked = (0..self.current_count()).collect();
        cx.notify();
    }

    fn push_recent(&mut self) {
        let p = self.root.clone();
        self.recents.retain(|r| r != &p);
        self.recents.insert(0, p);
        self.recents.truncate(RECENTS_MAX);
        save_recents(&self.recents);
    }

    fn flash(&mut self, msg: impl Into<String>, cx: &mut Context<Self>) {
        self.toast = Some(msg.into());
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1500))
                .await;
            let _ = this.update(cx, |this, cx| {
                this.toast = None;
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn rewalk(&mut self, new_root: PathBuf, cx: &mut Context<Self>) {
        if let Some(c) = self.content.as_ref() {
            c.cancel();
        }
        let notifier = Arc::new(|| {}) as Arc<dyn Fn() + Send + Sync>;
        self.index = Index::new(notifier);
        self.index.spawn_walk(new_root.clone(), self.show_hidden);
        self.watcher = FsWatcher::spawn(new_root.clone(), std::time::Duration::from_millis(400));
        self.root = new_root;
        self.query.clear();
        self.results.clear();
        self.content = None;
        self.search_mode = SearchMode::Filename;
        self.selected = 0;
        self.marked.clear();
        self.last_anchor = 0;
        self.viewport_start = 0;
        self.push_recent();
        // Keep the active tab snapshot's root in sync so the tab title matches.
        if let Some(t) = self.tabs.get_mut(self.active) {
            t.root = self.root.clone();
        }
        self.persist_session();
        cx.notify();
    }

    fn toggle_content_search(&mut self, cx: &mut Context<Self>) {
        self.search_mode = match self.search_mode {
            SearchMode::Filename => SearchMode::Content,
            SearchMode::Content => SearchMode::Filename,
        };
        if self.search_mode == SearchMode::Filename {
            if let Some(c) = self.content.as_ref() {
                c.cancel();
            }
            self.content = None;
        } else {
            self.restart_content_search();
        }
        self.selected = 0;
        self.viewport_start = 0;
        cx.notify();
    }

    fn restart_content_search(&mut self) {
        if let Some(c) = self.content.as_ref() {
            c.cancel();
        }
        if self.query.trim().len() < 2 {
            self.content = None;
            return;
        }
        self.content = Some(ContentSearch::spawn(
            self.root.clone(),
            self.query.clone(),
            self.show_hidden,
            5000,
        ));
    }

    fn cycle_sort(&mut self, cx: &mut Context<Self>) {
        self.sort = self.sort.next();
        self.apply_sort();
        self.persist_settings();
        cx.notify();
    }

    fn set_sort(&mut self, s: SortMode, cx: &mut Context<Self>) {
        self.sort = s;
        self.apply_sort();
        self.persist_settings();
        cx.notify();
    }

    fn apply_sort(&mut self) {
        match self.sort {
            SortMode::Relevance => {}
            SortMode::Name => {
                self.results
                    .sort_by(|a, b| a.display.to_lowercase().cmp(&b.display.to_lowercase()));
            }
            SortMode::Size => {
                self.results.sort_by_key(|e| {
                    std::cmp::Reverse(std::fs::metadata(&e.path).map(|m| m.len()).unwrap_or(0))
                });
            }
            SortMode::Modified => {
                self.results.sort_by_key(|e| {
                    std::cmp::Reverse(
                        std::fs::metadata(&e.path)
                            .and_then(|m| m.modified())
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_secs())
                            .unwrap_or(0),
                    )
                });
            }
        }
    }

    fn copy_basename(&mut self, cx: &mut Context<Self>) {
        let mut paths = self.marked_paths();
        if paths.is_empty() {
            if let Some(p) = self.current_path() {
                paths.push(p);
            }
        }
        if paths.is_empty() {
            return;
        }
        let payload = paths
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .collect::<Vec<_>>()
            .join("\n");
        cx.write_to_clipboard(ClipboardItem::new_string(payload));
        let n = paths.len();
        self.flash(format!("Copied {} name{}", n, if n == 1 { "" } else { "s" }), cx);
    }

    fn copy_path(&mut self, cx: &mut Context<Self>) {
        let paths = self.marked_paths();
        let payload = if !paths.is_empty() {
            paths
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("\n")
        } else if let Some(p) = self.current_path() {
            p.to_string_lossy().into_owned()
        } else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(payload));
        let n = paths.len().max(1);
        self.flash(format!("Copied {} path{}", n, if n == 1 { "" } else { "s" }), cx);
    }

    fn trash_selected(&mut self, cx: &mut Context<Self>) {
        let mut targets = self.marked_paths();
        if targets.is_empty() {
            if let Some(p) = self.current_path() {
                targets.push(p);
            }
        }
        if targets.is_empty() {
            return;
        }
        let n = targets.len();
        let posix_list = targets
            .iter()
            .map(|p| {
                format!(
                    "POSIX file \"{}\"",
                    p.to_string_lossy().replace('"', "\\\"")
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let script = format!(
            r#"tell application "Finder" to delete {{ {} }}"#,
            posix_list
        );
        let out = std::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output();
        match out {
            Ok(o) if o.status.success() => {
                self.push_undo(UndoAction::Trash { count: n });
                self.flash(format!("Trashed {}", n), cx);
                self.marked.clear();
                self.refresh(cx);
            }
            _ => self.flash("Trash failed", cx),
        }
    }

    fn enter_path_bar(&mut self, cx: &mut Context<Self>) {
        self.mode = Mode::PathBar;
        self.path_input = self.root.to_string_lossy().into_owned();
        self.path_cursor = self.path_input.len();
        cx.notify();
    }

    fn open_settings(&mut self, cx: &mut Context<Self>) {
        self.mode = Mode::Settings;
        cx.notify();
    }

    fn start_create(&mut self, kind: CreateKind, cx: &mut Context<Self>) {
        self.create_kind = kind;
        self.create_input = match kind {
            CreateKind::Folder => "Untitled folder".into(),
            CreateKind::File => "untitled.txt".into(),
        };
        self.create_cursor = self.create_input.len();
        self.mode = Mode::Create;
        cx.notify();
    }

    fn submit_create(&mut self, cx: &mut Context<Self>) {
        let name = self.create_input.trim().to_string();
        if name.is_empty() || name.contains('/') {
            self.flash("Invalid name", cx);
            return;
        }
        let parent = if self.current_is_dir() {
            self.current_path().unwrap_or_else(|| self.root.clone())
        } else {
            self.root.clone()
        };
        let target = parent.join(&name);
        let result = match self.create_kind {
            CreateKind::Folder => std::fs::create_dir(&target),
            CreateKind::File => std::fs::File::create(&target).map(|_| ()),
        };
        match result {
            Ok(_) => {
                self.flash(
                    match self.create_kind {
                        CreateKind::Folder => "Created folder",
                        CreateKind::File => "Created file",
                    },
                    cx,
                );
                self.mode = Mode::Search;
                self.create_input.clear();
                self.refresh(cx);
            }
            Err(e) => self.flash(format!("Create failed: {}", e), cx),
        }
    }

    fn cancel_create(&mut self, cx: &mut Context<Self>) {
        self.mode = Mode::Search;
        self.create_input.clear();
        cx.notify();
    }

    fn start_batch_rename(&mut self, cx: &mut Context<Self>) {
        if self.marked.is_empty() {
            self.flash("No items marked (use ⌘-click)", cx);
            return;
        }
        self.batch_input = "find/replace".into();
        self.batch_cursor = self.batch_input.len();
        self.mode = Mode::BatchRename;
        cx.notify();
    }

    fn submit_batch_rename(&mut self, cx: &mut Context<Self>) {
        let raw = self.batch_input.trim().to_string();
        let Some((find, replace)) = raw.split_once('/') else {
            self.flash("Use `find/replace` format", cx);
            return;
        };
        if find.is_empty() {
            self.flash("`find` is empty", cx);
            return;
        }
        let targets = self.marked_paths();
        let mut ok = 0usize;
        let mut fail = 0usize;
        let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::new();
        for p in &targets {
            let Some(name) = p.file_name().and_then(|s| s.to_str()) else {
                fail += 1;
                continue;
            };
            let new_name = name.replacen(find, replace, usize::MAX);
            if new_name == name {
                continue;
            }
            let target = p.with_file_name(&new_name);
            match std::fs::rename(p, &target) {
                Ok(_) => {
                    ok += 1;
                    pairs.push((p.clone(), target));
                }
                Err(_) => fail += 1,
            }
        }
        if !pairs.is_empty() {
            self.push_undo(UndoAction::Move { pairs });
        }
        self.flash(
            if fail == 0 {
                format!("Renamed {}", ok)
            } else {
                format!("Renamed {} · {} failed", ok, fail)
            },
            cx,
        );
        self.mode = Mode::Search;
        self.batch_input.clear();
        self.marked.clear();
        self.refresh(cx);
    }

    fn cancel_batch_rename(&mut self, cx: &mut Context<Self>) {
        self.mode = Mode::Search;
        self.batch_input.clear();
        cx.notify();
    }

    fn paste_move(&mut self, cx: &mut Context<Self>) {
        let payload = match cx.read_from_clipboard() {
            Some(item) => item
                .entries()
                .iter()
                .filter_map(|e| match e {
                    gpui::ClipboardEntry::String(s) => Some(s.text().to_string()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
            None => return,
        };
        let sources: Vec<PathBuf> = payload
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .collect();
        if sources.is_empty() {
            self.flash("Clipboard has no paths", cx);
            return;
        }
        let dest_dir = if self.current_is_dir() {
            self.current_path().unwrap_or_else(|| self.root.clone())
        } else {
            self.root.clone()
        };
        let mut moved = 0usize;
        let mut failed = 0usize;
        let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::new();
        for src in &sources {
            let Some(name) = src.file_name() else { failed += 1; continue };
            let target = dest_dir.join(name);
            match std::fs::rename(src, &target) {
                Ok(_) => {
                    moved += 1;
                    pairs.push((src.clone(), target));
                }
                Err(_) => failed += 1,
            }
        }
        if !pairs.is_empty() {
            self.push_undo(UndoAction::Move { pairs });
        }
        let msg = if failed == 0 {
            format!("Moved {}", moved)
        } else {
            format!("Moved {} · {} failed", moved, failed)
        };
        self.flash(msg, cx);
        self.refresh(cx);
    }

    fn toggle_tree(&mut self, cx: &mut Context<Self>) {
        self.tree_visible = !self.tree_visible;
        if self.tree_visible && self.tree.is_none() {
            let mut t = TreeNode::new(self.root.clone());
            t.load_children();
            t.expanded = true;
            self.tree = Some(t);
        }
        cx.notify();
    }

    fn tree_toggle_at(&mut self, p: &std::path::Path, cx: &mut Context<Self>) {
        if let Some(t) = self.tree.as_mut() {
            t.toggle_at(p);
            cx.notify();
        }
    }

    fn start_rename(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.current_path() else { return };
        self.rename_input = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        // Default cursor before the extension dot so the stem is easy to retype.
        self.rename_cursor = self
            .rename_input
            .rfind('.')
            .unwrap_or(self.rename_input.len());
        self.mode = Mode::Rename;
        cx.notify();
    }

    fn submit_rename(&mut self, cx: &mut Context<Self>) {
        let Some(p) = self.current_path() else {
            self.mode = Mode::Search;
            return;
        };
        let new_name = self.rename_input.trim().to_string();
        if new_name.is_empty() || new_name.contains('/') {
            self.flash("Invalid name", cx);
            return;
        }
        let target = p.with_file_name(&new_name);
        match std::fs::rename(&p, &target) {
            Ok(_) => {
                self.push_undo(UndoAction::Rename {
                    from: p.clone(),
                    to: target.clone(),
                });
                self.flash("Renamed", cx);
                self.mode = Mode::Search;
                self.rename_input.clear();
                self.refresh(cx);
            }
            Err(e) => self.flash(format!("Rename failed: {}", e), cx),
        }
    }

    fn push_undo(&mut self, action: UndoAction) {
        self.undo_stack.push(action);
        if self.undo_stack.len() > 50 {
            self.undo_stack.remove(0);
        }
        save_undo(&self.undo_stack);
    }

    fn undo(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.undo_stack.pop() else {
            self.flash("Nothing to undo", cx);
            return;
        };
        save_undo(&self.undo_stack);
        match action {
            UndoAction::Rename { from, to } => match std::fs::rename(&to, &from) {
                Ok(_) => {
                    self.flash("Undid rename", cx);
                    self.refresh(cx);
                }
                Err(e) => {
                    self.flash(format!("Undo rename failed: {}", e), cx);
                    self.undo_stack.push(UndoAction::Rename { from, to });
                }
            },
            UndoAction::Move { pairs } => {
                let mut ok = 0usize;
                let mut fail = 0usize;
                for (src, dst) in &pairs {
                    match std::fs::rename(dst, src) {
                        Ok(_) => ok += 1,
                        Err(_) => fail += 1,
                    }
                }
                self.flash(
                    if fail == 0 {
                        format!("Undid move ({})", ok)
                    } else {
                        format!("Undo move: {} ok · {} failed", ok, fail)
                    },
                    cx,
                );
                self.refresh(cx);
            }
            UndoAction::Trash { count } => {
                self.flash(
                    format!(
                        "Trash can't be undone here — press ⌘⇧Z in Finder ({} item{})",
                        count,
                        if count == 1 { "" } else { "s" }
                    ),
                    cx,
                );
            }
        }
    }

    fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.mode = Mode::Search;
        self.rename_input.clear();
        cx.notify();
    }

    fn open_menu_for(&mut self, idx: usize, x: f32, y: f32, cx: &mut Context<Self>) {
        let (path, is_dir) = match self.search_mode {
            SearchMode::Filename => match self.results.get(idx) {
                Some(e) => (e.path.clone(), e.is_dir),
                None => return,
            },
            SearchMode::Content => match self
                .content
                .as_ref()
                .and_then(|c| c.matches.get(idx))
            {
                Some(m) => (m.path.clone(), false),
                None => return,
            },
        };
        self.selected = idx;
        self.menu = Some(MenuState { x, y, path, is_dir });
        cx.notify();
    }

    fn close_menu(&mut self, cx: &mut Context<Self>) {
        self.menu = None;
        cx.notify();
    }

    fn close_overlay(&mut self, cx: &mut Context<Self>) {
        self.mode = Mode::Search;
        self.path_input.clear();
        cx.notify();
    }

    fn persist_settings(&self) {
        let s = settings::Settings {
            show_hidden: self.show_hidden,
            sort: self.sort.label().to_string(),
        };
        settings::save(&s);
    }

    fn submit_path_bar(&mut self, cx: &mut Context<Self>) {
        let raw = self.path_input.trim().to_string();
        let expanded = expand_tilde(&raw);
        if let Some(p) = expanded {
            if p.is_dir() {
                self.mode = Mode::Search;
                self.rewalk(p, cx);
                return;
            }
        }
        self.flash("Path not found", cx);
    }

    fn cancel_path_bar(&mut self, cx: &mut Context<Self>) {
        self.mode = Mode::Search;
        self.path_input.clear();
        cx.notify();
    }

    fn refresh(&mut self, cx: &mut Context<Self>) {
        let root = self.root.clone();
        self.rewalk(root, cx);
    }

    fn toggle_hidden(&mut self, cx: &mut Context<Self>) {
        self.show_hidden = !self.show_hidden;
        self.persist_settings();
        self.refresh(cx);
    }

    fn update_query(&mut self, q: String, cx: &mut Context<Self>) {
        self.query = q;
        self.index.set_query(&self.query);
        if self.search_mode == SearchMode::Content {
            self.restart_content_search();
        }
        self.selected = 0;
        self.viewport_start = 0;
        self.marked.clear();
        cx.notify();
    }

    fn pull_results(&mut self) {
        self.results = self.index.results(2000);
        self.apply_sort();
        if self.selected >= self.results.len() {
            self.selected = self.results.len().saturating_sub(1);
        }
        self.ensure_visible();
    }

    fn ensure_visible(&mut self) {
        self.scroll_handle
            .scroll_to_item(self.selected, ScrollStrategy::Nearest);
    }

    fn move_selection(&mut self, delta: i32, cx: &mut Context<Self>) {
        let n = self.current_count();
        if n == 0 {
            return;
        }
        let new = (self.selected as i32 + delta).clamp(0, (n - 1) as i32) as usize;
        self.selected = new;
        self.ensure_visible();
        cx.notify();
    }

    fn current(&self) -> Option<&Arc<PathEntry>> {
        self.results.get(self.selected)
    }

    fn current_count(&self) -> usize {
        match self.search_mode {
            SearchMode::Filename => self.results.len(),
            SearchMode::Content => self.content.as_ref().map(|c| c.matches.len()).unwrap_or(0),
        }
    }

    fn current_path(&self) -> Option<PathBuf> {
        match self.search_mode {
            SearchMode::Filename => self.current().map(|e| e.path.clone()),
            SearchMode::Content => self
                .content
                .as_ref()
                .and_then(|c| c.matches.get(self.selected).map(|m| m.path.clone())),
        }
    }

    fn current_is_dir(&self) -> bool {
        match self.search_mode {
            SearchMode::Filename => self.current().map(|e| e.is_dir).unwrap_or(false),
            SearchMode::Content => false,
        }
    }

    fn open_selected(&self) {
        match self.search_mode {
            SearchMode::Content => {
                if let Some(c) = self.content.as_ref() {
                    if let Some(m) = c.matches.get(self.selected) {
                        open_at_line(&m.path, m.line);
                        return;
                    }
                }
            }
            SearchMode::Filename => {}
        }
        if let Some(p) = self.current_path() {
            let _ = std::process::Command::new("open").arg(p).spawn();
        }
    }

    fn reveal_selected(&self) {
        if let Some(p) = self.current_path() {
            let _ = std::process::Command::new("open").arg("-R").arg(p).spawn();
        }
    }

    fn quicklook_selected(&self) {
        if let Some(p) = self.current_path() {
            let _ = std::process::Command::new("qlmanage").arg("-p").arg(p).spawn();
        }
    }

    fn descend(&mut self, cx: &mut Context<Self>) {
        if self.current_is_dir() {
            if let Some(p) = self.current_path() {
                self.rewalk(p, cx);
            }
        }
    }

    fn ascend(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.root.parent() {
            self.rewalk(parent.to_path_buf(), cx);
        }
    }

    fn jump_pin(&mut self, n: usize, cx: &mut Context<Self>) {
        let pins = pins();
        if let Some(p) = pins.get(n) {
            if p.path.exists() {
                self.rewalk(p.path.clone(), cx);
            }
        }
    }

    fn build_results_list(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let mode = self.search_mode;
        let total = self.current_count();
        let selected = self.selected;
        let marked = std::sync::Arc::new(self.marked.clone());
        let weak = cx.entity().downgrade();
        let query = self.query.clone();

        // Pre-compute fuzzy match indices for the current snapshot — closure
        // can't borrow &mut self.matcher.
        let (results_snapshot, content_snapshot, name_indices, content_indices) = match mode {
            SearchMode::Filename => {
                let results = self.results.clone();
                let mut indices: Vec<Vec<u32>> = Vec::with_capacity(results.len());
                if !query.is_empty() {
                    for e in &results {
                        indices.push(self.match_indices(&e.display));
                    }
                }
                (Some(std::sync::Arc::new(results)), None, std::sync::Arc::new(indices), std::sync::Arc::new(Vec::<Vec<u32>>::new()))
            }
            SearchMode::Content => {
                let matches: Vec<ContentMatch> = self
                    .content
                    .as_ref()
                    .map(|c| c.matches.clone())
                    .unwrap_or_default();
                (None, Some(std::sync::Arc::new(matches)), std::sync::Arc::new(Vec::<Vec<u32>>::new()), std::sync::Arc::new(Vec::<Vec<u32>>::new()))
            }
        };

        let scroll = self.scroll_handle.clone();

        uniform_list("results-list", total, move |range, _window, _app| {
            range
                .into_iter()
                .map(|i| {
                    if mode == SearchMode::Filename {
                        if let Some(results) = results_snapshot.as_ref() {
                            render_filename_row(
                                i,
                                &results[i],
                                selected == i,
                                marked.contains(&i),
                                name_indices.get(i).map(|v| v.as_slice()).unwrap_or(&[]),
                                weak.clone(),
                            )
                            .into_any_element()
                        } else {
                            div().into_any_element()
                        }
                    } else {
                        if let Some(matches) = content_snapshot.as_ref() {
                            render_content_row(
                                i,
                                &matches[i],
                                selected == i,
                                marked.contains(&i),
                                &query,
                                content_indices.get(i).map(|v| v.as_slice()).unwrap_or(&[]),
                                weak.clone(),
                            )
                            .into_any_element()
                        } else {
                            div().into_any_element()
                        }
                    }
                })
                .collect()
        })
        .size_full()
        .track_scroll(&scroll)
    }

    #[allow(dead_code)]
    fn render_filename_rows(
        &mut self,
        start: usize,
        end: usize,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let entries: Vec<(usize, Arc<PathEntry>)> = self.results[start..end]
            .iter()
            .cloned()
            .enumerate()
            .map(|(local, e)| (start + local, e))
            .collect();
        entries
            .into_iter()
            .map(|(i, e)| {
                let selected = i == self.selected;
                let row_bg = if selected {
                    theme::c(theme::SEL)
                } else {
                    rgba(0x00000000)
                };
                let name = e
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| e.display.clone());
                let icon = file_icon(e.is_dir, &name);
                let icon_color = if e.is_dir {
                    theme::c(theme::DIR)
                } else {
                    theme::c(theme::ACCENT)
                };
                let parent_dir = e
                    .display
                    .rsplit_once('/')
                    .map(|(p, _)| p.to_string())
                    .unwrap_or_default();
                let name_indices = self.match_indices(&e.display);
                let prefix_len = e.display.len().saturating_sub(name.len()) as u32;
                let name_hi: Vec<u32> = name_indices
                    .iter()
                    .filter_map(|&ix| ix.checked_sub(prefix_len))
                    .collect();
                let is_dir = e.is_dir;
                let path_for_click = e.path.clone();
                let is_marked = self.marked.contains(&i);
                let path_for_drag = e.path.clone();

                div()
                    .id(SharedString::from(format!("row-{}", i)))
                    .on_drag(
                        (),
                        {
                            let p = path_for_drag.clone();
                            move |_, _offset, _window, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    p.to_string_lossy().into_owned(),
                                ));
                                cx.new(|_| DragGhost)
                            }
                        },
                    )
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .pl_3()
                    .pr_4()
                    .py_1p5()
                    .bg(if is_marked && !selected {
                        rgba(0xcba6f733)
                    } else {
                        row_bg
                    })
                    .border_l_2()
                    .border_color(if selected {
                        theme::c(theme::ACCENT_BAR)
                    } else if is_marked {
                        theme::c(theme::MATCH)
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(|s| s.bg(rgba(0x31324466)))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &gpui::MouseDownEvent, _, cx| {
                            let m = &ev.modifiers;
                            if m.shift {
                                this.mark_range(i);
                            } else if m.platform {
                                this.toggle_mark(i);
                            } else {
                                this.marked.clear();
                                this.last_anchor = i;
                                this.selected = i;
                                if ev.click_count >= 2 {
                                    if is_dir {
                                        this.rewalk(path_for_click.clone(), cx);
                                    } else {
                                        let _ = std::process::Command::new("open")
                                            .arg(&path_for_click)
                                            .spawn();
                                    }
                                }
                            }
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, ev: &gpui::MouseDownEvent, _, cx| {
                            this.open_menu_for(
                                i,
                                ev.position.x.as_f32(),
                                ev.position.y.as_f32(),
                                cx,
                            );
                        }),
                    )
                    .child({
                        let ext = e
                            .path
                            .extension()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_ascii_lowercase();
                        let size_ok = std::fs::metadata(&e.path)
                            .map(|m| m.len() <= 2 * 1024 * 1024)
                            .unwrap_or(false);
                        if !is_dir && size_ok && is_image(&ext) {
                            div()
                                .w(px(22.0))
                                .h(px(22.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    gpui::img(e.path.clone())
                                        .max_w(px(22.0))
                                        .max_h(px(22.0))
                                        .rounded_sm(),
                                )
                                .into_any_element()
                        } else {
                            div()
                                .w(px(22.0))
                                .text_color(icon_color)
                                .text_size(px(14.0))
                                .child(SharedString::from(icon.to_string()))
                                .into_any_element()
                        }
                    })
                    .child(
                        div()
                            .flex_grow()
                            .flex()
                            .flex_row()
                            .text_size(px(13.0))
                            .when(is_dir, |s| s.font_weight(gpui::FontWeight::SEMIBOLD))
                            .children(highlight(
                                &name,
                                &name_hi,
                                if is_dir {
                                    theme::c(theme::DIR)
                                } else {
                                    theme::c(theme::TEXT)
                                },
                                theme::c(theme::MATCH),
                            )),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::c(theme::MUTED))
                            .child(SharedString::from(parent_dir)),
                    )
                    .into_any_element()
            })
            .collect()
    }

    #[allow(dead_code)]
    fn render_content_rows(
        &mut self,
        start: usize,
        end: usize,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let q = self.query.clone();
        let root = self.root.clone();
        let matches: Vec<(usize, ContentMatch)> = self
            .content
            .as_ref()
            .map(|c| {
                c.matches[start..end]
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(local, m)| (start + local, m))
                    .collect()
            })
            .unwrap_or_default();

        matches
            .into_iter()
            .map(|(i, m)| {
                let selected = i == self.selected;
                let row_bg = if selected {
                    theme::c(theme::SEL)
                } else {
                    rgba(0x00000000)
                };
                let name = m
                    .path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let rel = m
                    .path
                    .strip_prefix(&root)
                    .ok()
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_else(|| m.path.to_string_lossy().into_owned());
                let icon = file_icon(false, &name);
                let path_for_click = m.path.clone();

                // Highlight the literal needle inside the snippet
                let snippet_lo = m.text.to_lowercase();
                let q_lo = q.to_lowercase();
                let hi_indices: Vec<u32> = if !q.is_empty() {
                    let mut ix = Vec::new();
                    let mut search_from = 0usize;
                    while let Some(pos) = snippet_lo[search_from..].find(&q_lo) {
                        let abs = search_from + pos;
                        for k in 0..q_lo.len() {
                            ix.push((abs + k) as u32);
                        }
                        search_from = abs + q_lo.len().max(1);
                    }
                    ix
                } else {
                    Vec::new()
                };

                let is_marked = self.marked.contains(&i);
                div()
                    .id(SharedString::from(format!("crow-{}", i)))
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .pl_3()
                    .pr_4()
                    .py_1p5()
                    .bg(if is_marked && !selected {
                        rgba(0xcba6f733)
                    } else {
                        row_bg
                    })
                    .border_l_2()
                    .border_color(if selected {
                        theme::c(theme::ACCENT_BAR)
                    } else if is_marked {
                        theme::c(theme::MATCH)
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(|s| s.bg(rgba(0x31324466)))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &gpui::MouseDownEvent, _, cx| {
                            let m = &ev.modifiers;
                            if m.shift {
                                this.mark_range(i);
                            } else if m.platform {
                                this.toggle_mark(i);
                            } else {
                                this.marked.clear();
                                this.last_anchor = i;
                                this.selected = i;
                                if ev.click_count >= 2 {
                                    let _ = std::process::Command::new("open")
                                        .arg(&path_for_click)
                                        .spawn();
                                }
                            }
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, ev: &gpui::MouseDownEvent, _, cx| {
                            this.open_menu_for(
                                i,
                                ev.position.x.as_f32(),
                                ev.position.y.as_f32(),
                                cx,
                            );
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .w(px(18.0))
                                    .text_color(theme::c(theme::ACCENT))
                                    .text_size(px(12.0))
                                    .child(SharedString::from(icon.to_string())),
                            )
                            .child(
                                div()
                                    .flex_grow()
                                    .text_size(px(12.0))
                                    .text_color(theme::c(theme::TEXT))
                                    .child(SharedString::from(rel)),
                            )
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme::c(theme::MUTED))
                                    .child(SharedString::from(format!(":{}", m.line))),
                            ),
                    )
                    .child({
                        let ext = m
                            .path
                            .extension()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_ascii_lowercase();
                        // Syntax-colour the snippet, then overlay match-pink on the
                        // exact substring positions reported by `hi_indices`.
                        let spans = highlight::highlight_one(&m.text, &ext);
                        let mut rendered: Vec<gpui::AnyElement> = Vec::new();
                        let mut cursor = 0usize;
                        for s in spans {
                            let span_len = s.text.chars().count();
                            let (r, g, b) = s.rgb;
                            let base = gpui::Rgba {
                                r: r as f32 / 255.0,
                                g: g as f32 / 255.0,
                                b: b as f32 / 255.0,
                                a: 1.0,
                            };
                            for (ci, ch) in s.text.chars().enumerate() {
                                let abs = (cursor + ci) as u32;
                                let hit = hi_indices.binary_search(&abs).is_ok();
                                let color = if hit {
                                    theme::c(theme::MATCH)
                                } else {
                                    base
                                };
                                rendered.push(
                                    div()
                                        .text_color(color)
                                        .child(SharedString::from(ch.to_string()))
                                        .into_any_element(),
                                );
                            }
                            cursor += span_len;
                        }
                        div()
                            .pl_6()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .font_family("Menlo")
                            .text_size(px(11.0))
                            .children(rendered)
                    })
                    .into_any_element()
            })
            .collect()
    }

    fn match_indices(&mut self, haystack: &str) -> Vec<u32> {
        if self.query.is_empty() {
            return Vec::new();
        }
        let mut buf1 = Vec::new();
        let mut buf2 = Vec::new();
        let h = nucleo::Utf32Str::new(haystack, &mut buf1);
        let n = nucleo::Utf32Str::new(&self.query, &mut buf2);
        let mut indices = Vec::new();
        self.matcher.fuzzy_indices(h, n, &mut indices);
        indices
    }
}

impl Focusable for Explorer {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// Returns a flat list of (label, Some(target_path)) clickable segments interleaved with
/// (separator_glyph, None) separators.
fn clickable_breadcrumb(root: &std::path::Path) -> Vec<(String, Option<PathBuf>)> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let abs = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut out: Vec<(String, Option<PathBuf>)> = Vec::new();

    // Prefix with ~ if under HOME.
    let (start_label, start_path, remainder) = if let Some(h) = home.as_ref() {
        if let Ok(rest) = abs.strip_prefix(h) {
            ("~".to_string(), h.clone(), Some(rest.to_path_buf()))
        } else {
            ("/".to_string(), PathBuf::from("/"), None)
        }
    } else {
        ("/".to_string(), PathBuf::from("/"), None)
    };

    out.push((start_label, Some(start_path.clone())));

    let segments: Vec<PathBuf> = if let Some(rest) = remainder {
        rest.components()
            .map(|c| std::path::PathBuf::from(c.as_os_str()))
            .collect()
    } else {
        abs.components()
            .skip(1)
            .map(|c| std::path::PathBuf::from(c.as_os_str()))
            .collect()
    };

    let mut acc = start_path;
    for seg in segments {
        out.push((" › ".to_string(), None));
        acc.push(&seg);
        out.push((seg.to_string_lossy().into_owned(), Some(acc.clone())));
    }
    out
}

fn highlight(name: &str, indices: &[u32], base: Rgba, hi: Rgba) -> Vec<gpui::AnyElement> {
    // Build runs: each run is (text, highlighted?)
    let mut runs: Vec<(String, bool)> = Vec::new();
    let mut cur = String::new();
    let mut cur_hi = false;
    for (ci, ch) in name.chars().enumerate() {
        let is_hi = indices.binary_search(&(ci as u32)).is_ok();
        if is_hi != cur_hi && !cur.is_empty() {
            runs.push((std::mem::take(&mut cur), cur_hi));
            cur_hi = is_hi;
        } else if cur.is_empty() {
            cur_hi = is_hi;
        }
        cur.push(ch);
    }
    if !cur.is_empty() {
        runs.push((cur, cur_hi));
    }
    runs.into_iter()
        .map(|(text, h)| {
            div()
                .text_color(if h { hi } else { base })
                .child(SharedString::from(text))
                .into_any_element()
        })
        .collect()
}

impl Render for Explorer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ─── Sidebar (left) ─────────────────────────────────────────────
        let current = self.root.clone();
        let sidebar = div()
            .w(px(180.0))
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme::t(theme::SIDEBAR))
            .border_r_1()
            .border_color(theme::c(theme::BORDER))
            .pt_8()
            .px_2()
            .gap_0p5()
            .child(
                div()
                    .px_2()
                    .pb_2()
                    .text_size(px(10.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("LOCATIONS"),
            )
            .children(pins().into_iter().enumerate().map(|(i, p)| {
                let active = p.path == current;
                let path_for_click = p.path.clone();
                div()
                    .id(SharedString::from(format!("pin-{}", i)))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1p5()
                    .rounded_md()
                    .bg(if active {
                        theme::c(theme::SEL)
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(|s| s.bg(theme::c(theme::SEL)))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            if path_for_click.exists() {
                                this.rewalk(path_for_click.clone(), cx);
                            }
                        }),
                    )
                    .child(
                        div()
                            .w(px(18.0))
                            .text_size(px(13.0))
                            .text_color(theme::c(theme::ACCENT))
                            .child(SharedString::from(p.icon.to_string())),
                    )
                    .child(
                        div()
                            .flex_grow()
                            .text_size(px(12.0))
                            .text_color(theme::c(theme::TEXT))
                            .child(SharedString::from(p.label.to_string())),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(theme::c(theme::MUTED))
                            .child(SharedString::from(format!("⌘{}", i + 1))),
                    )
            }))
            .child(
                div()
                    .mt_4()
                    .px_2()
                    .pb_2()
                    .text_size(px(10.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("BOOKMARKS"),
            )
            .children(self.bookmarks.clone().into_iter().enumerate().map(|(i, p)| {
                let active = p == current;
                let target = p.clone();
                let display = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.to_string_lossy().into_owned());
                div()
                    .id(SharedString::from(format!("bm-{}", i)))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(if active {
                        theme::c(theme::SEL)
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(|s| s.bg(theme::c(theme::SEL)))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            if target.exists() {
                                this.rewalk(target.clone(), cx);
                            }
                        }),
                    )
                    .child(
                        div()
                            .w(px(18.0))
                            .text_size(px(11.0))
                            .text_color(theme::c(theme::ACCENT_BAR))
                            .child(""),
                    )
                    .child(
                        div()
                            .flex_grow()
                            .text_size(px(12.0))
                            .text_color(theme::c(theme::TEXT))
                            .child(SharedString::from(display)),
                    )
            }))
            .child(
                div()
                    .mt_4()
                    .px_2()
                    .pb_2()
                    .text_size(px(10.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("RECENT"),
            )
            .children(self.recents.clone().into_iter().enumerate().map(|(i, p)| {
                let active = p == current;
                let target = p.clone();
                let display = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.to_string_lossy().into_owned());
                let sub = short_path(&p.to_string_lossy());
                div()
                    .id(SharedString::from(format!("recent-{}", i)))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(if active {
                        theme::c(theme::SEL)
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(|s| s.bg(theme::c(theme::SEL)))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            if target.exists() {
                                this.rewalk(target.clone(), cx);
                            }
                        }),
                    )
                    .child(
                        div()
                            .w(px(18.0))
                            .text_size(px(11.0))
                            .text_color(theme::c(theme::MUTED))
                            .child(""),
                    )
                    .child(
                        div()
                            .flex_grow()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_size(px(11.0))
                                    .text_color(theme::c(theme::SUBTEXT))
                                    .child(SharedString::from(display)),
                            )
                            .child(
                                div()
                                    .text_size(px(9.0))
                                    .text_color(theme::c(theme::MUTED))
                                    .child(SharedString::from(sub)),
                            ),
                    )
            }));

        // ─── Top breadcrumb bar (or path-bar input) ─────────────────────
        let crumbs = if self.mode == Mode::PathBar {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .px_4()
                .py_2()
                .bg(theme::t(theme::SURFACE_ALT))
                .border_b_1()
                .border_color(theme::c(theme::ACCENT_BAR))
                .text_size(px(12.0))
                .child(
                    div()
                        .text_color(theme::c(theme::ACCENT_BAR))
                        .child("Go to:"),
                )
                .child(
                    div()
                        .flex_grow()
                        .font_family("Menlo")
                        .child(render_text_with_caret(
                            &self.path_input,
                            self.path_cursor,
                            self.path_anchor,
                            theme::c(theme::TEXT),
                            theme::c(theme::ACCENT_BAR),
                            self.caret_on,
                        )),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(theme::c(theme::MUTED))
                        .child("↵ go · esc cancel"),
                )
        } else {
            let segs = clickable_breadcrumb(&self.root);
            let branch = git_branch_for(&self.root);
            let branch_chip: Option<gpui::AnyElement> = branch.map(|b| {
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .py_0p5()
                    .mr_2()
                    .rounded_sm()
                    .bg(theme::c(theme::SEL))
                    .text_size(px(11.0))
                    .text_color(theme::c(theme::DIR))
                    .child("🌿")
                    .child(SharedString::from(b))
                    .into_any_element()
            });
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .px_4()
                .py_2()
                .bg(theme::t(theme::SURFACE_ALT))
                .border_b_1()
                .border_color(theme::c(theme::BORDER))
                .text_size(px(12.0))
                .children(branch_chip)
                .children(segs.into_iter().enumerate().map(|(i, (label, path))| {
                    if let Some(path) = path {
                        div()
                            .id(SharedString::from(format!("crumb-{}", i)))
                            .px_1()
                            .rounded_sm()
                            .text_color(theme::c(theme::SUBTEXT))
                            .hover(|s| s.bg(rgba(0x31324466)).text_color(theme::c(theme::TEXT)))
                            .cursor_pointer()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.rewalk(path.clone(), cx);
                                }),
                            )
                            .child(SharedString::from(label))
                            .into_any_element()
                    } else {
                        // separator
                        div()
                            .px_0p5()
                            .text_color(theme::c(theme::MUTED))
                            .child(SharedString::from(label))
                            .into_any_element()
                    }
                }))
        };

        // ─── Search input ───────────────────────────────────────────────
        let (icon_glyph, placeholder, counter) = match self.search_mode {
            SearchMode::Filename => {
                let total_seen = self.index.nucleo.snapshot().item_count();
                let suffix = if self.index.is_done() { "" } else { " · scanning…" };
                (
                    "",
                    "Search filenames…",
                    format!("{} / {}{}", self.results.len(), total_seen, suffix),
                )
            }
            SearchMode::Content => {
                let n = self.content.as_ref().map(|c| c.matches.len()).unwrap_or(0);
                let done = self.content.as_ref().map(|c| c.done).unwrap_or(false);
                (
                    "",
                    "Search inside files…",
                    format!("{} match{}", n, if done { "" } else { " · scanning…" }),
                )
            }
        };

        let mode_badge = div()
            .px_2()
            .py_0p5()
            .rounded_sm()
            .bg(theme::c(theme::SEL))
            .text_size(px(10.0))
            .text_color(if self.search_mode == SearchMode::Content {
                theme::c(theme::ACCENT_BAR)
            } else {
                theme::c(theme::SUBTEXT)
            })
            .child(SharedString::from(match self.search_mode {
                SearchMode::Filename => "FILES",
                SearchMode::Content => "CONTENT",
            }));

        let input = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_4()
            .py_3()
            .bg(theme::t(theme::SURFACE_ALT))
            .border_b_1()
            .border_color(if self.search_mode == SearchMode::Content {
                theme::c(theme::ACCENT_BAR)
            } else {
                theme::c(theme::BORDER)
            })
            .child(mode_badge)
            .child(
                div()
                    .text_color(theme::c(theme::ACCENT))
                    .text_size(px(16.0))
                    .child(SharedString::from(icon_glyph.to_string())),
            )
            .child(
                div()
                    .flex_grow()
                    .text_size(px(16.0))
                    .text_color(if self.query.is_empty() {
                        theme::c(theme::MUTED)
                    } else {
                        theme::c(theme::TEXT)
                    })
                    .child(SharedString::from(if self.query.is_empty() {
                        placeholder.to_string()
                    } else {
                        format!("{}▏", self.query)
                    })),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(theme::c(theme::MUTED))
                    .child(SharedString::from(counter)),
            );

        // ─── Results list (virtualized via uniform_list) ───────────────
        let total = self.current_count();
        let empty = total == 0;
        let results_pane = div()
            .flex()
            .flex_col()
            .flex_grow()
            .overflow_hidden()
            .bg(theme::t(theme::SURFACE))
            .child(if empty {
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .size_full()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(40.0))
                            .text_color(theme::c(theme::MUTED))
                            .child(""),
                    )
                    .child(
                        div()
                            .text_size(px(13.0))
                            .text_color(theme::c(theme::SUBTEXT))
                            .child(if self.query.is_empty() {
                                "Indexing files…".to_string()
                            } else {
                                format!("No matches for “{}”", self.query)
                            }),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(theme::c(theme::MUTED))
                            .child(if self.query.is_empty() {
                                "If this stays empty, try ⌘H to show hidden files."
                            } else {
                                "Try a shorter query or ⌘H to include hidden files."
                            }),
                    )
                    .into_any_element()
            } else {
                self.build_results_list(cx).into_any_element()
            });

        // ─── Preview pane ──────────────────────────────────────────────
        let preview = self.current().map(|e| {
            let name = e
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| e.display.clone());
            let meta = std::fs::metadata(&e.path).ok();
            let (size_s, modified_s, kind_s) = match meta {
                Some(m) => {
                    let size = if m.is_file() {
                        human_size(m.len())
                    } else {
                        "—".into()
                    };
                    let modified = m
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| format_time(d.as_secs()))
                        .unwrap_or_else(|| "—".into());
                    let kind = if m.is_dir() {
                        "Folder"
                    } else if m.is_symlink() {
                        "Symlink"
                    } else {
                        "File"
                    };
                    (size, modified, kind.to_string())
                }
                None => ("—".into(), "—".into(), "—".into()),
            };
            let icon = file_icon(e.is_dir, &name);
            let icon_color = if e.is_dir {
                theme::c(theme::DIR)
            } else {
                theme::c(theme::ACCENT)
            };
            (name, size_s, modified_s, kind_s, icon, icon_color, e.path.clone())
        });

        let is_dir = self.current().map(|e| e.is_dir).unwrap_or(false);
        let preview_pane = div()
            .w(px(320.0))
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme::t(theme::SURFACE_ALT))
            .border_l_1()
            .border_color(theme::c(theme::BORDER))
            .p_5()
            .overflow_hidden()
            .child(if let Some((name, size, modified, kind, icon, icon_color, path)) = preview {
                let body = preview_body(&path, is_dir);
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .text_size(px(40.0))
                                    .text_color(icon_color)
                                    .child(SharedString::from(icon.to_string())),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(px(14.0))
                                            .text_color(theme::c(theme::TEXT))
                                            .child(SharedString::from(name)),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.0))
                                            .text_color(theme::c(theme::MUTED))
                                            .child(SharedString::from(kind.clone())),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .pt_2()
                            .border_t_1()
                            .border_color(theme::c(theme::BORDER))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(meta_row("Size", &size))
                            .child(meta_row("Modified", &modified))
                            .child(meta_row(
                                "Path",
                                &short_path(&path.to_string_lossy()),
                            )),
                    )
                    .child(
                        div()
                            .mt_2()
                            .pt_3()
                            .border_t_1()
                            .border_color(theme::c(theme::BORDER))
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(10.0))
                                    .text_color(theme::c(theme::MUTED))
                                    .child("PREVIEW"),
                            )
                            .child(body),
                    )
                    .into_any_element()
            } else {
                div()
                    .text_size(px(12.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("No selection")
                    .into_any_element()
            });

        // ─── Body ──────────────────────────────────────────────────────
        let tree_pane: Option<gpui::AnyElement> = if self.tree_visible {
            Some(self.render_tree_pane(cx).into_any_element())
        } else {
            None
        };
        let body = div()
            .flex()
            .flex_row()
            .flex_grow()
            .overflow_hidden()
            .children(tree_pane)
            .child(results_pane)
            .child(preview_pane);

        // ─── Right column (everything except sidebar) ──────────────────
        let tab_strip = self.render_tab_strip(cx);
        let main_col = div()
            .flex()
            .flex_col()
            .flex_grow()
            .overflow_hidden()
            .child(tab_strip)
            .child(crumbs)
            .child(input)
            .child(body)
            .child(footer(self.sort, self.toast.as_deref()));

        let base_layer = div()
            .flex()
            .flex_row()
            .size_full()
            .child(sidebar)
            .child(main_col)
            .into_any_element();

        let mut layers: Vec<gpui::AnyElement> = vec![base_layer];
        if self.mode == Mode::Settings {
            layers.push(self.render_settings_overlay(cx).into_any_element());
        }
        if self.menu.is_some() {
            layers.push(self.render_context_menu(cx).into_any_element());
        }
        if self.mode == Mode::Rename {
            layers.push(self.render_rename_overlay(cx).into_any_element());
        }
        if self.mode == Mode::Create {
            layers.push(self.render_create_overlay(cx).into_any_element());
        }
        if self.mode == Mode::BatchRename {
            layers.push(self.render_batch_rename_overlay(cx).into_any_element());
        }

        // ─── Root ──────────────────────────────────────────────────────
        div()
            .track_focus(&self.focus)
            .key_context("Explorer")
            .relative()
            .size_full()
            .bg(theme::t(theme::WINDOW_BG))
            .text_color(theme::c(theme::TEXT))
            .font_family(".SystemUIFont")
            .on_key_down(cx.listener(|this, ev: &gpui::KeyDownEvent, _, cx| {
                let m = &ev.keystroke.modifiers;
                let key = ev.keystroke.key.as_str();

                // Path bar mode owns most keys.
                if this.mode == Mode::PathBar {
                    match key {
                        "escape" => { this.cancel_path_bar(cx); return; }
                        "enter" => { this.submit_path_bar(cx); return; }
                        _ => {
                            let mut text = std::mem::take(&mut this.path_input);
                            let mut cur = this.path_cursor;
                            let mut anc = this.path_anchor;
                            let changed = handle_text_edit(&mut text, &mut cur, &mut anc, ev, cx);
                            this.path_input = text;
                            this.path_cursor = cur;
                            this.path_anchor = anc;
                            if changed {
                                this.caret_on = true;
                                cx.notify();
                            }
                            return;
                        }
                    }
                }

                // Settings overlay: Esc closes, nothing else passes through.
                if this.mode == Mode::Settings {
                    if key == "escape" || (m.platform && key == ",") {
                        this.close_overlay(cx);
                    }
                    return;
                }

                // Create mode owns most keys.
                if this.mode == Mode::Create {
                    match key {
                        "escape" => { this.cancel_create(cx); return; }
                        "enter" => { this.submit_create(cx); return; }
                        _ => {
                            let mut text = std::mem::take(&mut this.create_input);
                            let mut cur = this.create_cursor;
                            let mut anc = this.create_anchor;
                            let changed = handle_text_edit(&mut text, &mut cur, &mut anc, ev, cx);
                            this.create_input = text;
                            this.create_cursor = cur;
                            this.create_anchor = anc;
                            if changed { this.caret_on = true; cx.notify(); }
                            return;
                        }
                    }
                }

                // Batch-rename mode owns most keys.
                if this.mode == Mode::BatchRename {
                    match key {
                        "escape" => { this.cancel_batch_rename(cx); return; }
                        "enter" => { this.submit_batch_rename(cx); return; }
                        _ => {
                            let mut text = std::mem::take(&mut this.batch_input);
                            let mut cur = this.batch_cursor;
                            let mut anc = this.batch_anchor;
                            let changed = handle_text_edit(&mut text, &mut cur, &mut anc, ev, cx);
                            this.batch_input = text;
                            this.batch_cursor = cur;
                            this.batch_anchor = anc;
                            if changed { this.caret_on = true; cx.notify(); }
                            return;
                        }
                    }
                }

                // Rename mode owns most keys.
                if this.mode == Mode::Rename {
                    match key {
                        "escape" => { this.cancel_rename(cx); return; }
                        "enter" => { this.submit_rename(cx); return; }
                        _ => {
                            let mut text = std::mem::take(&mut this.rename_input);
                            let mut cur = this.rename_cursor;
                            let mut anc = this.rename_anchor;
                            let changed = handle_text_edit(&mut text, &mut cur, &mut anc, ev, cx);
                            this.rename_input = text;
                            this.rename_cursor = cur;
                            this.rename_anchor = anc;
                            if changed { this.caret_on = true; cx.notify(); }
                            return;
                        }
                    }
                }

                // Context menu: Esc closes.
                if this.menu.is_some() && key == "escape" {
                    this.close_menu(cx);
                    return;
                }

                // F2 starts inline rename. ⌘F2 = batch rename of marked.
                if key == "f2" {
                    if m.platform {
                        this.start_batch_rename(cx);
                    } else {
                        this.start_rename(cx);
                    }
                    return;
                }

                if m.platform {
                    match key {
                        "enter" => { this.reveal_selected(); return; }
                        "h" => { this.toggle_hidden(cx); return; }
                        "r" => { this.refresh(cx); return; }
                        "l" => { this.enter_path_bar(cx); return; }
                        "c" if m.shift => { this.copy_basename(cx); return; }
                        "c" => { this.copy_path(cx); return; }
                        "f" => { this.toggle_content_search(cx); return; }
                        "j" => { this.cycle_sort(cx); return; }
                        "b" => { this.toggle_bookmark(cx); return; }
                        "a" => { this.select_all(cx); return; }
                        "d" => { this.clear_marks(cx); return; }
                        "n" if m.shift => { this.start_create(CreateKind::Folder, cx); return; }
                        "n" if m.alt => { this.start_create(CreateKind::File, cx); return; }
                        "n" => { open_new_window(cx); return; }
                        "v" => { this.paste_move(cx); return; }
                        "z" => { this.undo(cx); return; }
                        "\\" => { this.toggle_tree(cx); return; }
                        "," => { this.open_settings(cx); return; }
                        "t" => { this.new_tab(cx); return; }
                        "w" => { this.close_tab(cx); return; }
                        "]" if m.shift => { this.next_tab(cx); return; }
                        "[" if m.shift => { this.prev_tab(cx); return; }
                        "backspace" => { this.trash_selected(cx); return; }
                        "1" => { this.jump_pin(0, cx); return; }
                        "2" => { this.jump_pin(1, cx); return; }
                        "3" => { this.jump_pin(2, cx); return; }
                        "4" => { this.jump_pin(3, cx); return; }
                        "5" => { this.jump_pin(4, cx); return; }
                        "6" => { this.jump_pin(5, cx); return; }
                        "up" => {
                            this.selected = 0;
                            this.ensure_visible();
                            cx.notify();
                            return;
                        }
                        "down" => {
                            this.selected = this.current_count().saturating_sub(1);
                            this.ensure_visible();
                            cx.notify();
                            return;
                        }
                        _ => {}
                    }
                }

                if m.control {
                    match key {
                        "j" | "n" => { this.move_selection(1, cx); return; }
                        "k" | "p" => { this.move_selection(-1, cx); return; }
                        "d" => { this.move_selection(VIEWPORT_ROWS as i32 / 2, cx); return; }
                        "u" => { this.move_selection(-(VIEWPORT_ROWS as i32) / 2, cx); return; }
                        _ => {}
                    }
                }

                match key {
                    "enter" => this.open_selected(),
                    "space" => this.quicklook_selected(),
                    "tab" if !m.shift => this.descend(cx),
                    "tab" if m.shift => this.ascend(cx),
                    "backspace" => {
                        if this.query.is_empty() {
                            this.ascend(cx);
                        } else {
                            let mut q = this.query.clone();
                            q.pop();
                            this.update_query(q, cx);
                        }
                    }
                    "down" => this.move_selection(1, cx),
                    "up" => this.move_selection(-1, cx),
                    "pagedown" => this.move_selection(VIEWPORT_ROWS as i32, cx),
                    "pageup" => this.move_selection(-(VIEWPORT_ROWS as i32), cx),
                    "home" => {
                        this.selected = 0;
                        this.ensure_visible();
                        cx.notify();
                    }
                    "end" => {
                        this.selected = this.current_count().saturating_sub(1);
                        this.ensure_visible();
                        cx.notify();
                    }
                    _ => {
                        if !m.platform && !m.control && !m.alt {
                            if let Some(ch) = ev.keystroke.key_char.as_ref() {
                                if ch.chars().all(|c| !c.is_control()) && ch != " " {
                                    let mut q = this.query.clone();
                                    q.push_str(ch);
                                    this.update_query(q, cx);
                                }
                            }
                        }
                    }
                }
            }))
            .children(layers)
    }
}

impl Explorer {
    fn render_context_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let menu = self.menu.clone().unwrap();
        let path = menu.path.clone();
        let is_dir = menu.is_dir;

        let item = |label: &'static str, icon: &'static str, accel: &'static str,
                    on_click: gpui::AnyElement|
         -> gpui::AnyElement {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .px_3()
                .py_1p5()
                .text_size(px(12.0))
                .text_color(theme::c(theme::TEXT))
                .hover(|s| s.bg(theme::c(theme::SEL)))
                .cursor_pointer()
                .child(
                    div()
                        .w(px(14.0))
                        .text_color(theme::c(theme::ACCENT))
                        .child(SharedString::from(icon.to_string())),
                )
                .child(
                    div()
                        .flex_grow()
                        .child(SharedString::from(label.to_string())),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(theme::c(theme::MUTED))
                        .child(SharedString::from(accel.to_string())),
                )
                .child(on_click)
                .into_any_element()
        };

        // Build menu item rows with their listeners.
        let p1 = path.clone();
        let row_open = div()
            .id("ctx-open")
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py_1p5()
            .text_size(px(12.0))
            .text_color(theme::c(theme::TEXT))
            .hover(|s| s.bg(theme::c(theme::SEL)))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if is_dir {
                        this.rewalk(p1.clone(), cx);
                    } else {
                        let _ = std::process::Command::new("open").arg(&p1).spawn();
                    }
                    this.close_menu(cx);
                }),
            )
            .child(
                div()
                    .w(px(14.0))
                    .text_color(theme::c(theme::ACCENT))
                    .child(""),
            )
            .child(div().flex_grow().child(if is_dir { "Open Folder" } else { "Open" }))
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("↵"),
            );

        let p2 = path.clone();
        let row_reveal = div()
            .id("ctx-reveal")
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py_1p5()
            .text_size(px(12.0))
            .text_color(theme::c(theme::TEXT))
            .hover(|s| s.bg(theme::c(theme::SEL)))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let _ = std::process::Command::new("open")
                        .arg("-R")
                        .arg(&p2)
                        .spawn();
                    this.close_menu(cx);
                }),
            )
            .child(
                div()
                    .w(px(14.0))
                    .text_color(theme::c(theme::ACCENT))
                    .child(""),
            )
            .child(div().flex_grow().child("Reveal in Finder"))
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("⌘↵"),
            );

        let p3 = path.clone();
        let row_quick = div()
            .id("ctx-quick")
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py_1p5()
            .text_size(px(12.0))
            .text_color(theme::c(theme::TEXT))
            .hover(|s| s.bg(theme::c(theme::SEL)))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    let _ = std::process::Command::new("qlmanage")
                        .arg("-p")
                        .arg(&p3)
                        .spawn();
                    this.close_menu(cx);
                }),
            )
            .child(
                div()
                    .w(px(14.0))
                    .text_color(theme::c(theme::ACCENT))
                    .child(""),
            )
            .child(div().flex_grow().child("Quick Look"))
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("␣"),
            );

        let p4 = path.clone();
        let row_copy = div()
            .id("ctx-copy")
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py_1p5()
            .text_size(px(12.0))
            .text_color(theme::c(theme::TEXT))
            .hover(|s| s.bg(theme::c(theme::SEL)))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(
                        p4.to_string_lossy().into_owned(),
                    ));
                    this.flash("Copied path", cx);
                    this.close_menu(cx);
                }),
            )
            .child(
                div()
                    .w(px(14.0))
                    .text_color(theme::c(theme::ACCENT))
                    .child(""),
            )
            .child(div().flex_grow().child("Copy Path"))
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("⌘C"),
            );

        let row_rename = div()
            .id("ctx-rename")
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py_1p5()
            .text_size(px(12.0))
            .text_color(theme::c(theme::TEXT))
            .hover(|s| s.bg(theme::c(theme::SEL)))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.close_menu(cx);
                    this.start_rename(cx);
                }),
            )
            .child(
                div()
                    .w(px(14.0))
                    .text_color(theme::c(theme::ACCENT))
                    .child(""),
            )
            .child(div().flex_grow().child("Rename"))
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("F2"),
            );

        let row_trash = div()
            .id("ctx-trash")
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .px_3()
            .py_1p5()
            .text_size(px(12.0))
            .text_color(theme::c(0xf38ba8))
            .hover(|s| s.bg(theme::c(theme::SEL)))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.close_menu(cx);
                    this.trash_selected(cx);
                }),
            )
            .child(
                div()
                    .w(px(14.0))
                    .text_color(theme::c(0xf38ba8))
                    .child(""),
            )
            .child(div().flex_grow().child("Move to Trash"))
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("⌘⌫"),
            );

        let _ = item; // unused helper kept for reference

        let panel = div()
            .w(px(240.0))
            .flex()
            .flex_col()
            .bg(theme::c(0x1e1e2e))
            .border_1()
            .border_color(theme::c(theme::BORDER))
            .rounded_md()
            .py_1()
            .child(row_open)
            .child(row_reveal)
            .child(row_quick)
            .child(
                div()
                    .my_1()
                    .h(px(1.0))
                    .bg(theme::c(theme::BORDER)),
            )
            .child(row_copy)
            .child(row_rename)
            .child(
                div()
                    .my_1()
                    .h(px(1.0))
                    .bg(theme::c(theme::BORDER)),
            )
            .child(row_trash);

        div()
            .absolute()
            .inset_0()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.close_menu(cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, _, _, cx| this.close_menu(cx)),
            )
            .child(
                div()
                    .id("ctx-menu")
                    .absolute()
                    .left(px(menu.x))
                    .top(px(menu.y))
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .child(panel),
            )
    }

    fn render_create_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let title = match self.create_kind {
            CreateKind::Folder => "NEW FOLDER",
            CreateKind::File => "NEW FILE",
        };
        let parent = if self.current_is_dir() {
            self.current_path().unwrap_or_else(|| self.root.clone())
        } else {
            self.root.clone()
        };
        let panel = div()
            .w(px(440.0))
            .flex()
            .flex_col()
            .gap_2()
            .bg(theme::c(0x1e1e2e))
            .border_1()
            .border_color(theme::c(theme::ACCENT_BAR))
            .rounded_lg()
            .p_5()
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::c(theme::MUTED))
                    .child(SharedString::from(title.to_string())),
            )
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(theme::c(theme::SUBTEXT))
                    .child(SharedString::from(format!(
                        "in {}",
                        short_path(&parent.to_string_lossy())
                    ))),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .font_family("Menlo")
                    .child(render_text_with_caret(
                        &self.create_input,
                        self.create_cursor,
                        self.create_anchor,
                        theme::c(theme::TEXT),
                        theme::c(theme::ACCENT_BAR),
                        self.caret_on,
                    )),
            )
            .child(
                div()
                    .pt_2()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("Press")
                    .child(
                        div()
                            .px_1p5()
                            .rounded_sm()
                            .bg(theme::c(theme::SEL))
                            .text_color(theme::c(theme::SUBTEXT))
                            .child("↵"),
                    )
                    .child("to create,")
                    .child(
                        div()
                            .px_1p5()
                            .rounded_sm()
                            .bg(theme::c(theme::SEL))
                            .text_color(theme::c(theme::SUBTEXT))
                            .child("esc"),
                    )
                    .child("to cancel"),
            );

        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x00000099))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.cancel_create(cx)),
            )
            .child(
                div()
                    .id("create-panel")
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .child(panel),
            )
    }

    fn render_batch_rename_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let n = self.marked.len();
        // Live preview of the first 5 marked items' before→after
        let (find, replace) = self
            .batch_input
            .split_once('/')
            .map(|(f, r)| (f.to_string(), r.to_string()))
            .unwrap_or((String::new(), String::new()));

        let preview: Vec<gpui::AnyElement> = if !find.is_empty() {
            self.marked_paths()
                .into_iter()
                .take(5)
                .filter_map(|p| {
                    let name = p.file_name()?.to_string_lossy().into_owned();
                    let new = name.replacen(&find, &replace, usize::MAX);
                    Some(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .text_size(px(11.0))
                            .font_family("Menlo")
                            .child(
                                div()
                                    .flex_grow()
                                    .text_color(theme::c(theme::MUTED))
                                    .child(SharedString::from(name)),
                            )
                            .child(
                                div()
                                    .text_color(theme::c(theme::MUTED))
                                    .child("→"),
                            )
                            .child(
                                div()
                                    .flex_grow()
                                    .text_color(theme::c(theme::TEXT))
                                    .child(SharedString::from(new)),
                            )
                            .into_any_element(),
                    )
                })
                .collect()
        } else {
            Vec::new()
        };

        let panel = div()
            .w(px(520.0))
            .flex()
            .flex_col()
            .gap_3()
            .bg(theme::c(0x1e1e2e))
            .border_1()
            .border_color(theme::c(theme::ACCENT_BAR))
            .rounded_lg()
            .p_5()
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::c(theme::MUTED))
                    .child(SharedString::from(format!(
                        "BATCH RENAME · {} item{}",
                        n,
                        if n == 1 { "" } else { "s" }
                    ))),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::c(theme::SUBTEXT))
                    .child("Replace pattern as find/replace (e.g. img/photo):"),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .font_family("Menlo")
                    .child(render_text_with_caret(
                        &self.batch_input,
                        self.batch_cursor,
                        self.batch_anchor,
                        theme::c(theme::TEXT),
                        theme::c(theme::ACCENT_BAR),
                        self.caret_on,
                    )),
            )
            .child(
                div()
                    .pt_2()
                    .border_t_1()
                    .border_color(theme::c(theme::BORDER))
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(if preview.is_empty() {
                        vec![div()
                            .text_size(px(11.0))
                            .text_color(theme::c(theme::MUTED))
                            .child("(preview will appear here)")
                            .into_any_element()]
                    } else {
                        preview
                    }),
            )
            .child(
                div()
                    .pt_2()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("Press")
                    .child(
                        div()
                            .px_1p5()
                            .rounded_sm()
                            .bg(theme::c(theme::SEL))
                            .text_color(theme::c(theme::SUBTEXT))
                            .child("↵"),
                    )
                    .child("to apply,")
                    .child(
                        div()
                            .px_1p5()
                            .rounded_sm()
                            .bg(theme::c(theme::SEL))
                            .text_color(theme::c(theme::SUBTEXT))
                            .child("esc"),
                    )
                    .child("to cancel"),
            );

        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x00000099))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.cancel_batch_rename(cx)),
            )
            .child(
                div()
                    .id("batch-rename-panel")
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .child(panel),
            )
    }

    fn render_tree_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut rows: Vec<(usize, PathBuf, bool, bool)> = Vec::new();
        if let Some(t) = self.tree.as_ref() {
            flatten_tree(t, 0, &mut rows);
        }
        let cur = self.root.clone();
        let entries: Vec<gpui::AnyElement> = rows
            .into_iter()
            .take(500)
            .enumerate()
            .map(|(i, (depth, p, is_dir, expanded))| {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| p.to_string_lossy().into_owned());
                let icon = file_icon(is_dir, &name);
                let active = p == cur;
                let tri = if is_dir {
                    if expanded { "▾" } else { "▸" }
                } else {
                    " "
                };
                let p_for_toggle = p.clone();
                let p_for_click = p.clone();
                div()
                    .id(SharedString::from(format!("tree-{}", i)))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .pr_2()
                    .py_0p5()
                    .pl(px(8.0 + (depth as f32) * 14.0))
                    .bg(if active {
                        theme::c(theme::SEL)
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(|s| s.bg(rgba(0x31324466)))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, ev: &gpui::MouseDownEvent, _, cx| {
                            if is_dir && ev.click_count >= 2 {
                                this.rewalk(p_for_click.clone(), cx);
                            } else if is_dir {
                                this.tree_toggle_at(&p_for_toggle, cx);
                            } else if ev.click_count >= 2 {
                                let _ = std::process::Command::new("open")
                                    .arg(&p_for_click)
                                    .spawn();
                            }
                        }),
                    )
                    .child(
                        div()
                            .w(px(12.0))
                            .text_size(px(9.0))
                            .text_color(theme::c(theme::MUTED))
                            .child(SharedString::from(tri.to_string())),
                    )
                    .child(
                        div()
                            .w(px(14.0))
                            .text_size(px(11.0))
                            .text_color(if is_dir {
                                theme::c(theme::DIR)
                            } else {
                                theme::c(theme::ACCENT)
                            })
                            .child(SharedString::from(icon.to_string())),
                    )
                    .child(
                        div()
                            .flex_grow()
                            .text_size(px(11.0))
                            .text_color(if active {
                                theme::c(theme::TEXT)
                            } else {
                                theme::c(theme::SUBTEXT)
                            })
                            .child(SharedString::from(name)),
                    )
                    .into_any_element()
            })
            .collect();

        div()
            .w(px(240.0))
            .flex_none()
            .flex()
            .flex_col()
            .bg(theme::t(theme::SIDEBAR))
            .border_r_1()
            .border_color(theme::c(theme::BORDER))
            .overflow_hidden()
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_size(px(10.0))
                    .text_color(theme::c(theme::MUTED))
                    .border_b_1()
                    .border_color(theme::c(theme::BORDER))
                    .child("FILES"),
            )
            .child(div().flex().flex_col().children(entries))
    }

    fn render_rename_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let panel = div()
            .w(px(420.0))
            .flex()
            .flex_col()
            .gap_2()
            .bg(theme::c(0x1e1e2e))
            .border_1()
            .border_color(theme::c(theme::ACCENT_BAR))
            .rounded_lg()
            .p_5()
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("RENAME"),
            )
            .child(
                div()
                    .text_size(px(14.0))
                    .font_family("Menlo")
                    .child(render_text_with_caret(
                        &self.rename_input,
                        self.rename_cursor,
                        self.rename_anchor,
                        theme::c(theme::TEXT),
                        theme::c(theme::ACCENT_BAR),
                        self.caret_on,
                    )),
            )
            .child(
                div()
                    .pt_2()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("Press")
                    .child(
                        div()
                            .px_1p5()
                            .rounded_sm()
                            .bg(theme::c(theme::SEL))
                            .text_color(theme::c(theme::SUBTEXT))
                            .child("↵"),
                    )
                    .child("to rename,")
                    .child(
                        div()
                            .px_1p5()
                            .rounded_sm()
                            .bg(theme::c(theme::SEL))
                            .text_color(theme::c(theme::SUBTEXT))
                            .child("esc"),
                    )
                    .child("to cancel"),
            );

        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x00000099))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.cancel_rename(cx)),
            )
            .child(
                div()
                    .id("rename-panel")
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .child(panel),
            )
    }

    fn render_tab_strip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active;
        let tab_chips: Vec<gpui::AnyElement> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let is_active = i == active;
                let label = t
                    .root
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| short_path(&t.root.to_string_lossy()));
                let can_close = self.tabs.len() > 1;

                div()
                    .id(SharedString::from(format!("tab-{}", i)))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .pl_3()
                    .pr_1p5()
                    .py_1p5()
                    .rounded_md()
                    .bg(if is_active {
                        theme::c(theme::SEL)
                    } else {
                        rgba(0x00000000)
                    })
                    .border_b_2()
                    .border_color(if is_active {
                        theme::c(theme::ACCENT_BAR)
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(|s| s.bg(rgba(0x31324466)))
                    .cursor_pointer()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| this.switch_tab(i, cx)),
                    )
                    .child(
                        div()
                            .text_size(px(11.0))
                            .text_color(if is_active {
                                theme::c(theme::ACCENT_BAR)
                            } else {
                                theme::c(theme::MUTED)
                            })
                            .child(""),
                    )
                    .child(
                        div()
                            .text_size(px(12.0))
                            .text_color(if is_active {
                                theme::c(theme::TEXT)
                            } else {
                                theme::c(theme::SUBTEXT)
                            })
                            .child(SharedString::from(label)),
                    )
                    .child(if can_close {
                        div()
                            .id(SharedString::from(format!("tab-x-{}", i)))
                            .px_1()
                            .text_size(px(10.0))
                            .text_color(theme::c(theme::MUTED))
                            .hover(|s| {
                                s.bg(rgba(0x31324488)).text_color(theme::c(theme::TEXT))
                            })
                            .cursor_pointer()
                            .rounded_sm()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    if i != this.active {
                                        // Switch to the closing tab first so close logic works on it
                                        this.switch_tab(i, cx);
                                    }
                                    this.close_tab(cx);
                                }),
                            )
                            .child("✕")
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    })
                    .into_any_element()
            })
            .collect();

        let plus = div()
            .id("tab-new")
            .px_2()
            .py_1p5()
            .ml_1()
            .rounded_md()
            .text_size(px(14.0))
            .text_color(theme::c(theme::MUTED))
            .hover(|s| s.bg(rgba(0x31324488)).text_color(theme::c(theme::TEXT)))
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.new_tab(cx)),
            )
            .child("+");

        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_2()
            .pt_2()
            .bg(theme::t(theme::SURFACE_ALT))
            .border_b_1()
            .border_color(theme::c(theme::BORDER))
            .children(tab_chips)
            .child(plus)
    }

    fn render_settings_overlay(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let show_hidden = self.show_hidden;
        let cur_sort = self.sort;

        let sort_chip = |label: &'static str, value: SortMode| {
            let active = value == cur_sort;
            div()
                .id(SharedString::from(format!("sort-{}", label)))
                .px_3()
                .py_1p5()
                .rounded_md()
                .bg(if active {
                    theme::c(theme::ACCENT_BAR)
                } else {
                    theme::c(theme::SEL)
                })
                .text_color(if active {
                    theme::c(0x11111b)
                } else {
                    theme::c(theme::TEXT)
                })
                .text_size(px(12.0))
                .hover(|s| s.bg(theme::c(theme::ACCENT_BAR)).text_color(theme::c(0x11111b)))
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| this.set_sort(value, cx)),
                )
                .child(SharedString::from(label.to_string()))
        };

        let hidden_chip = |label: &'static str, value: bool| {
            let active = show_hidden == value;
            div()
                .id(SharedString::from(format!("hidden-{}", label)))
                .px_3()
                .py_1p5()
                .rounded_md()
                .bg(if active {
                    theme::c(theme::ACCENT_BAR)
                } else {
                    theme::c(theme::SEL)
                })
                .text_color(if active {
                    theme::c(0x11111b)
                } else {
                    theme::c(theme::TEXT)
                })
                .text_size(px(12.0))
                .hover(|s| s.bg(theme::c(theme::ACCENT_BAR)).text_color(theme::c(0x11111b)))
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        if this.show_hidden != value {
                            this.toggle_hidden(cx);
                        }
                    }),
                )
                .child(SharedString::from(label.to_string()))
        };

        let row = |label: &'static str, chips: Vec<gpui::AnyElement>| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_4()
                .py_3()
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(theme::c(theme::SUBTEXT))
                        .child(SharedString::from(label.to_string())),
                )
                .child(div().flex().flex_row().gap_2().children(chips))
        };

        let panel = div()
            .w(px(440.0))
            .flex()
            .flex_col()
            .bg(theme::c(0x1e1e2e))
            .border_1()
            .border_color(theme::c(theme::ACCENT_BAR))
            .rounded_lg()
            .p_6()
            .gap_2()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .pb_2()
                    .border_b_1()
                    .border_color(theme::c(theme::BORDER))
                    .child(
                        div()
                            .text_size(px(15.0))
                            .text_color(theme::c(theme::TEXT))
                            .child("Settings"),
                    )
                    .child(
                        div()
                            .text_size(px(10.0))
                            .text_color(theme::c(theme::MUTED))
                            .child(SharedString::from(
                                short_path(&settings_file_display()).to_string(),
                            )),
                    ),
            )
            .child(row(
                "Hidden files",
                vec![
                    hidden_chip("Show", true).into_any_element(),
                    hidden_chip("Hide", false).into_any_element(),
                ],
            ))
            .child(row(
                "Default sort",
                vec![
                    sort_chip("Relevance", SortMode::Relevance).into_any_element(),
                    sort_chip("Name", SortMode::Name).into_any_element(),
                    sort_chip("Size", SortMode::Size).into_any_element(),
                    sort_chip("Modified", SortMode::Modified).into_any_element(),
                ],
            ))
            .child(
                div()
                    .pt_3()
                    .border_t_1()
                    .border_color(theme::c(theme::BORDER))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("Press")
                    .child(
                        div()
                            .px_1p5()
                            .rounded_sm()
                            .bg(theme::c(theme::SEL))
                            .text_color(theme::c(theme::SUBTEXT))
                            .child("esc"),
                    )
                    .child("or")
                    .child(
                        div()
                            .px_1p5()
                            .rounded_sm()
                            .bg(theme::c(theme::SEL))
                            .text_color(theme::c(theme::SUBTEXT))
                            .child("⌘,"),
                    )
                    .child("to close"),
            );

        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x00000099))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.close_overlay(cx)),
            )
            .child(
                // Inner wrapper swallows clicks so they don't dismiss the panel
                div()
                    .id("settings-panel")
                    .on_mouse_down(MouseButton::Left, |_, _, _| {})
                    .child(panel),
            )
    }
}

fn prev_boundary(s: &str, idx: usize) -> usize {
    if idx == 0 || idx > s.len() {
        return 0;
    }
    let mut i = idx - 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn next_boundary(s: &str, idx: usize) -> usize {
    if idx >= s.len() {
        return s.len();
    }
    let mut i = idx + 1;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

fn prev_word(s: &str, idx: usize) -> usize {
    if idx == 0 {
        return 0;
    }
    // skip trailing whitespace, then skip word chars
    let bytes = s.as_bytes();
    let mut i = idx;
    while i > 0 && bytes[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    while i > 0 && !bytes[i - 1].is_ascii_whitespace() {
        i -= 1;
    }
    i
}

fn next_word(s: &str, idx: usize) -> usize {
    let bytes = s.as_bytes();
    let mut i = idx;
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn selection_range(cursor: usize, anchor: Option<usize>) -> Option<(usize, usize)> {
    let a = anchor?;
    if a == cursor {
        return None;
    }
    if a < cursor {
        Some((a, cursor))
    } else {
        Some((cursor, a))
    }
}

/// Generic key handler for any single-line text input with selection.
/// Returns true if state changed.
fn handle_text_edit(
    text: &mut String,
    cursor: &mut usize,
    anchor: &mut Option<usize>,
    ev: &gpui::KeyDownEvent,
    cx: &mut Context<Explorer>,
) -> bool {
    let m = &ev.keystroke.modifiers;
    let key = ev.keystroke.key.as_str();

    // Helper: start/extend selection if shift, else clear.
    let begin_motion = |cursor: usize, anchor: &mut Option<usize>, shift: bool| {
        if shift {
            if anchor.is_none() {
                *anchor = Some(cursor);
            }
        } else {
            *anchor = None;
        }
    };

    // Helper: replace selection with given string. Returns true if selection existed.
    fn replace_selection(
        text: &mut String,
        cursor: &mut usize,
        anchor: &mut Option<usize>,
        s: &str,
    ) -> bool {
        if let Some((lo, hi)) = selection_range(*cursor, *anchor) {
            text.replace_range(lo..hi, s);
            *cursor = lo + s.len();
            *anchor = None;
            true
        } else {
            false
        }
    }

    match key {
        "left" => {
            // If selection exists and no shift, collapse to left edge.
            if !m.shift {
                if let Some((lo, _)) = selection_range(*cursor, *anchor) {
                    *cursor = lo;
                    *anchor = None;
                    return true;
                }
            }
            begin_motion(*cursor, anchor, m.shift);
            *cursor = if m.platform {
                0
            } else if m.alt {
                prev_word(text, *cursor)
            } else {
                prev_boundary(text, *cursor)
            };
            true
        }
        "right" => {
            if !m.shift {
                if let Some((_, hi)) = selection_range(*cursor, *anchor) {
                    *cursor = hi;
                    *anchor = None;
                    return true;
                }
            }
            begin_motion(*cursor, anchor, m.shift);
            *cursor = if m.platform {
                text.len()
            } else if m.alt {
                next_word(text, *cursor)
            } else {
                next_boundary(text, *cursor)
            };
            true
        }
        "home" => {
            begin_motion(*cursor, anchor, m.shift);
            *cursor = 0;
            if !m.shift {
                *anchor = None;
            }
            true
        }
        "end" => {
            begin_motion(*cursor, anchor, m.shift);
            *cursor = text.len();
            if !m.shift {
                *anchor = None;
            }
            true
        }
        "backspace" => {
            if replace_selection(text, cursor, anchor, "") {
                return true;
            }
            if *cursor == 0 {
                return false;
            }
            let start = if m.platform {
                0
            } else if m.alt {
                prev_word(text, *cursor)
            } else {
                prev_boundary(text, *cursor)
            };
            text.replace_range(start..*cursor, "");
            *cursor = start;
            true
        }
        "delete" => {
            if replace_selection(text, cursor, anchor, "") {
                return true;
            }
            if *cursor >= text.len() {
                return false;
            }
            let end = if m.alt {
                next_word(text, *cursor)
            } else {
                next_boundary(text, *cursor)
            };
            text.replace_range(*cursor..end, "");
            true
        }
        "v" if m.platform => {
            let s: Option<String> = cx.read_from_clipboard().and_then(|c| {
                c.entries().iter().find_map(|e| match e {
                    gpui::ClipboardEntry::String(s) => Some(s.text().to_string()),
                    _ => None,
                })
            });
            if let Some(s) = s {
                let s = s.replace('\n', " ");
                if !replace_selection(text, cursor, anchor, &s) {
                    text.insert_str(*cursor, &s);
                    *cursor += s.len();
                }
                true
            } else {
                false
            }
        }
        "c" if m.platform => {
            if let Some((lo, hi)) = selection_range(*cursor, *anchor) {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text[lo..hi].to_string()));
            }
            false
        }
        "x" if m.platform => {
            if let Some((lo, hi)) = selection_range(*cursor, *anchor) {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text[lo..hi].to_string()));
                replace_selection(text, cursor, anchor, "");
                return true;
            }
            false
        }
        "a" if m.platform => {
            *anchor = Some(0);
            *cursor = text.len();
            true
        }
        _ => {
            if !m.platform && !m.control && !m.alt {
                if let Some(ch) = ev.keystroke.key_char.as_ref() {
                    if ch.chars().all(|c| !c.is_control()) {
                        if !replace_selection(text, cursor, anchor, ch) {
                            text.insert_str(*cursor, ch);
                            *cursor += ch.len();
                        }
                        return true;
                    }
                }
            }
            false
        }
    }
}

/// Render `text` with a blinking caret bar at `cursor`, and an optional
/// selection highlight if `anchor != cursor`.
fn render_text_with_caret(
    text: &str,
    cursor: usize,
    anchor: Option<usize>,
    color: Rgba,
    caret_color: Rgba,
    caret_on: bool,
) -> gpui::Div {
    let cursor = cursor.min(text.len());
    let caret_bg = if caret_on { caret_color } else { rgba(0x00000000) };
    let sel_bg = rgba(0xcba6f766); // mauve @ 40%

    if let Some((lo, hi)) = selection_range(cursor, anchor) {
        // Selection painted: [before][sel][caret?][after]
        // Caret sits at cursor edge (lo or hi).
        let caret_at_start = cursor == lo;
        div()
            .flex()
            .flex_row()
            .child(
                div()
                    .text_color(color)
                    .child(SharedString::from(text[..lo].to_string())),
            )
            .child(if caret_at_start {
                div().w(px(1.5)).h(px(20.0)).bg(caret_bg).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(
                div()
                    .bg(sel_bg)
                    .text_color(color)
                    .child(SharedString::from(text[lo..hi].to_string())),
            )
            .child(if !caret_at_start {
                div().w(px(1.5)).h(px(20.0)).bg(caret_bg).into_any_element()
            } else {
                div().into_any_element()
            })
            .child(
                div()
                    .text_color(color)
                    .child(SharedString::from(text[hi..].to_string())),
            )
    } else {
        let (before, after) = text.split_at(cursor);
        div()
            .flex()
            .flex_row()
            .child(
                div()
                    .text_color(color)
                    .child(SharedString::from(before.to_string())),
            )
            .child(div().w(px(1.5)).h(px(20.0)).bg(caret_bg))
            .child(
                div()
                    .text_color(color)
                    .child(SharedString::from(after.to_string())),
            )
    }
}

/// Best-effort: return the current branch by reading `.git/HEAD` if any
/// ancestor of `root` is a git repository.
fn git_branch_for(root: &std::path::Path) -> Option<String> {
    let mut cur = root.to_path_buf();
    loop {
        let head = cur.join(".git/HEAD");
        if let Ok(body) = std::fs::read_to_string(&head) {
            let body = body.trim();
            if let Some(ref_path) = body.strip_prefix("ref: refs/heads/") {
                return Some(ref_path.to_string());
            }
            // Detached HEAD: short SHA
            return Some(body.chars().take(7).collect());
        }
        if !cur.pop() {
            return None;
        }
    }
}

fn open_at_line(path: &std::path::Path, line: u64) {
    for editor in &["cursor", "code"] {
        let res = std::process::Command::new(editor)
            .arg("--goto")
            .arg(format!("{}:{}", path.display(), line))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        if res.is_ok() {
            return;
        }
    }
    let _ = std::process::Command::new("open").arg(path).spawn();
}

fn open_new_window(cx: &mut Context<Explorer>) {
    let root = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let bounds = Bounds::centered(None, gpui::size(px(1080.0), px(680.0)), cx);
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_background: WindowBackgroundAppearance::Blurred,
            titlebar: Some(gpui::TitlebarOptions {
                title: Some("File Explorer".into()),
                appears_transparent: true,
                ..Default::default()
            }),
            ..Default::default()
        },
        move |_, cx| cx.new(|cx| Explorer::new(root.clone(), cx)),
    );
}

fn undo_file_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let dir = home.join(".config/file-explorer");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("undo.txt"))
}

fn load_undo() -> Vec<UndoAction> {
    let Some(p) = undo_file_path() else { return Vec::new() };
    let Ok(body) = std::fs::read_to_string(p) else { return Vec::new() };
    let mut out: Vec<UndoAction> = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (kind, rest) = match line.split_once('\0') {
            Some(p) => p,
            None => continue,
        };
        match kind {
            "rename" => {
                let mut it = rest.split('\0');
                if let (Some(from), Some(to)) = (it.next(), it.next()) {
                    out.push(UndoAction::Rename {
                        from: PathBuf::from(from),
                        to: PathBuf::from(to),
                    });
                }
            }
            "move" => {
                let pairs: Vec<(PathBuf, PathBuf)> = rest
                    .split('\x1e')
                    .filter_map(|p| {
                        let mut it = p.split('\x1f');
                        Some((PathBuf::from(it.next()?), PathBuf::from(it.next()?)))
                    })
                    .collect();
                if !pairs.is_empty() {
                    out.push(UndoAction::Move { pairs });
                }
            }
            "trash" => {
                if let Ok(n) = rest.parse() {
                    out.push(UndoAction::Trash { count: n });
                }
            }
            _ => {}
        }
    }
    if out.len() > 50 {
        out.drain(..out.len() - 50);
    }
    out
}

fn save_undo(stack: &[UndoAction]) {
    let Some(p) = undo_file_path() else { return };
    let body: String = stack
        .iter()
        .map(|a| match a {
            UndoAction::Rename { from, to } => format!(
                "rename\0{}\0{}",
                from.to_string_lossy(),
                to.to_string_lossy()
            ),
            UndoAction::Move { pairs } => {
                let inner = pairs
                    .iter()
                    .map(|(s, d)| format!("{}\x1f{}", s.to_string_lossy(), d.to_string_lossy()))
                    .collect::<Vec<_>>()
                    .join("\x1e");
                format!("move\0{}", inner)
            }
            UndoAction::Trash { count } => format!("trash\0{}", count),
        })
        .collect::<Vec<_>>()
        .join("\n");
    let _ = std::fs::write(p, body);
}

fn session_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let dir = home.join(".config/file-explorer");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("session.txt"))
}

fn load_session() -> Vec<TabSnapshot> {
    session_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| {
            s.lines()
                .filter(|l| !l.trim().is_empty())
                .filter_map(parse_session_line)
                .filter(|t| t.root.exists())
                .collect()
        })
        .unwrap_or_default()
}

fn parse_session_line(line: &str) -> Option<TabSnapshot> {
    let parts: Vec<&str> = line.splitn(7, '\t').collect();
    if parts.is_empty() {
        return None;
    }
    let root = PathBuf::from(parts[0]);
    let query = parts.get(1).map(|s| s.to_string()).unwrap_or_default();
    let search_mode = match parts.get(2).copied().unwrap_or("files") {
        "content" => SearchMode::Content,
        _ => SearchMode::Filename,
    };
    let sort = match parts.get(3).copied().unwrap_or("relevance") {
        "name" => SortMode::Name,
        "size" => SortMode::Size,
        "modified" => SortMode::Modified,
        _ => SortMode::Relevance,
    };
    let selected = parts.get(4).and_then(|s| s.parse().ok()).unwrap_or(0);
    let viewport_start = parts.get(5).and_then(|s| s.parse().ok()).unwrap_or(0);
    let show_hidden = parts.get(6).map(|s| s == &"true").unwrap_or(false);
    Some(TabSnapshot {
        root,
        query,
        search_mode,
        sort,
        selected,
        viewport_start,
        show_hidden,
    })
}

fn save_session(tabs: &[TabSnapshot]) {
    if let Some(p) = session_path() {
        let body: String = tabs
            .iter()
            .map(|t| {
                format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    t.root.to_string_lossy(),
                    t.query.replace('\t', " "),
                    match t.search_mode {
                        SearchMode::Filename => "files",
                        SearchMode::Content => "content",
                    },
                    t.sort.label(),
                    t.selected,
                    t.viewport_start,
                    t.show_hidden,
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write(p, body);
    }
}

fn thumb_for(path: &std::path::Path) -> Option<PathBuf> {
    let cache = std::env::temp_dir().join("file-explorer-thumbs");
    let _ = std::fs::create_dir_all(&cache);
    // Hash the path to a stable filename without pulling in a hash crate.
    let key: u64 = path
        .to_string_lossy()
        .as_bytes()
        .iter()
        .fold(1469598103934665603u64, |h, &b| {
            (h ^ b as u64).wrapping_mul(1099511628211)
        });
    let out = cache.join(format!("{:016x}.png", key));
    if !out.exists() {
        let status = std::process::Command::new("qlmanage")
            .args(&["-t", "-s", "512", "-o"])
            .arg(&cache)
            .arg(path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok()?;
        if !status.success() {
            return None;
        }
        // qlmanage names the output "<basename>.png" inside the cache dir.
        let name = path.file_name()?.to_string_lossy().into_owned();
        let generated = cache.join(format!("{}.png", name));
        if generated.exists() {
            let _ = std::fs::rename(&generated, &out);
        }
    }
    if out.exists() {
        Some(out)
    } else {
        None
    }
}

fn bookmarks_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let dir = home.join(".config/file-explorer");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("bookmarks.txt"))
}

fn load_bookmarks() -> Vec<PathBuf> {
    bookmarks_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| {
            s.lines()
                .filter(|l| !l.trim().is_empty())
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .collect()
        })
        .unwrap_or_default()
}

fn save_bookmarks(list: &[PathBuf]) {
    if let Some(p) = bookmarks_path() {
        let body: String = list
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write(p, body);
    }
}

fn recents_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let dir = home.join(".config/file-explorer");
    let _ = std::fs::create_dir_all(&dir);
    Some(dir.join("recents.txt"))
}

fn load_recents() -> Vec<PathBuf> {
    recents_path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| {
            s.lines()
                .filter(|l| !l.trim().is_empty())
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .take(RECENTS_MAX)
                .collect()
        })
        .unwrap_or_default()
}

fn save_recents(list: &[PathBuf]) {
    if let Some(p) = recents_path() {
        let body: String = list
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("\n");
        let _ = std::fs::write(p, body);
    }
}

fn expand_tilde(s: &str) -> Option<PathBuf> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(rest) = s.strip_prefix("~") {
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        let rest = rest.trim_start_matches('/');
        Some(if rest.is_empty() {
            home
        } else {
            home.join(rest)
        })
    } else {
        Some(PathBuf::from(s))
    }
}

fn render_filename_row(
    i: usize,
    e: &Arc<PathEntry>,
    selected: bool,
    is_marked: bool,
    name_hi: &[u32],
    weak: gpui::WeakEntity<Explorer>,
) -> gpui::AnyElement {
    let row_bg = if selected {
        theme::c(theme::SEL)
    } else {
        rgba(0x00000000)
    };
    let name = e
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| e.display.clone());
    let icon = file_icon(e.is_dir, &name);
    let icon_color = if e.is_dir {
        theme::c(theme::DIR)
    } else {
        theme::c(theme::ACCENT)
    };
    let parent_dir = e
        .display
        .rsplit_once('/')
        .map(|(p, _)| p.to_string())
        .unwrap_or_default();
    let prefix_len = e.display.len().saturating_sub(name.len()) as u32;
    let name_hi_local: Vec<u32> = name_hi
        .iter()
        .filter_map(|&ix| ix.checked_sub(prefix_len))
        .collect();
    let is_dir = e.is_dir;
    let path_for_click = e.path.clone();

    let ext = e
        .path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let size_ok = std::fs::metadata(&e.path)
        .map(|m| m.len() <= 2 * 1024 * 1024)
        .unwrap_or(false);
    let icon_child: gpui::AnyElement = if !is_dir && size_ok && is_image(&ext) {
        div()
            .w(px(22.0))
            .h(px(22.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                gpui::img(e.path.clone())
                    .max_w(px(22.0))
                    .max_h(px(22.0))
                    .rounded_sm(),
            )
            .into_any_element()
    } else {
        div()
            .w(px(22.0))
            .text_color(icon_color)
            .text_size(px(14.0))
            .child(SharedString::from(icon.to_string()))
            .into_any_element()
    };

    let weak_left = weak.clone();
    let weak_right = weak.clone();
    let path_for_menu = e.path.clone();

    div()
        .id(SharedString::from(format!("row-{}", i)))
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .pl_3()
        .pr_4()
        .py_1p5()
        .bg(if is_marked && !selected {
            rgba(0xcba6f733)
        } else {
            row_bg
        })
        .border_l_2()
        .border_color(if selected {
            theme::c(theme::ACCENT_BAR)
        } else if is_marked {
            theme::c(theme::MATCH)
        } else {
            rgba(0x00000000)
        })
        .hover(|s| s.bg(rgba(0x31324466)))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            move |ev: &gpui::MouseDownEvent, _w, app| {
                let m = ev.modifiers;
                let click_count = ev.click_count;
                let path = path_for_click.clone();
                let _ = weak_left.update(app, move |this, cx| {
                    if m.shift {
                        this.mark_range(i);
                    } else if m.platform {
                        this.toggle_mark(i);
                    } else {
                        this.marked.clear();
                        this.last_anchor = i;
                        this.selected = i;
                        if click_count >= 2 {
                            if is_dir {
                                this.rewalk(path.clone(), cx);
                            } else {
                                let _ = std::process::Command::new("open")
                                    .arg(&path)
                                    .spawn();
                            }
                        }
                    }
                    cx.notify();
                });
            },
        )
        .on_mouse_down(
            MouseButton::Right,
            move |ev: &gpui::MouseDownEvent, _w, app| {
                let x = ev.position.x.as_f32();
                let y = ev.position.y.as_f32();
                let path = path_for_menu.clone();
                let _ = weak_right.update(app, move |this, cx| {
                    this.selected = i;
                    this.menu = Some(MenuState {
                        x,
                        y,
                        path,
                        is_dir,
                    });
                    cx.notify();
                });
            },
        )
        .child(icon_child)
        .child(
            div()
                .flex_grow()
                .flex()
                .flex_row()
                .text_size(px(13.0))
                .when(is_dir, |s| s.font_weight(gpui::FontWeight::SEMIBOLD))
                .children(highlight(
                    &name,
                    &name_hi_local,
                    if is_dir {
                        theme::c(theme::DIR)
                    } else {
                        theme::c(theme::TEXT)
                    },
                    theme::c(theme::MATCH),
                )),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::c(theme::MUTED))
                .child(SharedString::from(parent_dir)),
        )
        .child({
            let size_text = if is_dir {
                "—".to_string()
            } else {
                std::fs::metadata(&e.path)
                    .map(|m| human_size(m.len()))
                    .unwrap_or_else(|_| "—".into())
            };
            div()
                .min_w(px(56.0))
                .text_size(px(11.0))
                .text_color(theme::c(theme::MUTED))
                .child(SharedString::from(size_text))
        })
        .into_any_element()
}

fn render_content_row(
    i: usize,
    m: &ContentMatch,
    selected: bool,
    is_marked: bool,
    query: &str,
    _content_hi: &[u32],
    weak: gpui::WeakEntity<Explorer>,
) -> gpui::AnyElement {
    let row_bg = if selected {
        theme::c(theme::SEL)
    } else {
        rgba(0x00000000)
    };
    let name = m
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let rel = m
        .path
        .to_string_lossy()
        .into_owned();
    let icon = file_icon(false, &name);
    let path_for_click = m.path.clone();
    let line = m.line;

    // Highlight needle positions within the snippet
    let snippet_lo = m.text.to_lowercase();
    let q_lo = query.to_lowercase();
    let hi_indices: Vec<u32> = if !q_lo.is_empty() {
        let mut ix = Vec::new();
        let mut from = 0usize;
        while let Some(pos) = snippet_lo[from..].find(&q_lo) {
            let abs = from + pos;
            for k in 0..q_lo.len() {
                ix.push((abs + k) as u32);
            }
            from = abs + q_lo.len().max(1);
        }
        ix
    } else {
        Vec::new()
    };

    let weak_left = weak.clone();
    let weak_right = weak.clone();
    let path_for_menu = m.path.clone();

    div()
        .id(SharedString::from(format!("crow-{}", i)))
        .flex()
        .flex_col()
        .gap_0p5()
        .pl_3()
        .pr_4()
        .py_1p5()
        .bg(if is_marked && !selected {
            rgba(0xcba6f733)
        } else {
            row_bg
        })
        .border_l_2()
        .border_color(if selected {
            theme::c(theme::ACCENT_BAR)
        } else if is_marked {
            theme::c(theme::MATCH)
        } else {
            rgba(0x00000000)
        })
        .hover(|s| s.bg(rgba(0x31324466)))
        .cursor_pointer()
        .on_mouse_down(
            MouseButton::Left,
            move |ev: &gpui::MouseDownEvent, _w, app| {
                let mods = ev.modifiers;
                let click_count = ev.click_count;
                let path = path_for_click.clone();
                let _ = weak_left.update(app, move |this, cx| {
                    if mods.shift {
                        this.mark_range(i);
                    } else if mods.platform {
                        this.toggle_mark(i);
                    } else {
                        this.marked.clear();
                        this.last_anchor = i;
                        this.selected = i;
                        if click_count >= 2 {
                            open_at_line(&path, line);
                        }
                    }
                    cx.notify();
                });
            },
        )
        .on_mouse_down(
            MouseButton::Right,
            move |ev: &gpui::MouseDownEvent, _w, app| {
                let x = ev.position.x.as_f32();
                let y = ev.position.y.as_f32();
                let path = path_for_menu.clone();
                let _ = weak_right.update(app, move |this, cx| {
                    this.selected = i;
                    this.menu = Some(MenuState {
                        x,
                        y,
                        path,
                        is_dir: false,
                    });
                    cx.notify();
                });
            },
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(18.0))
                        .text_color(theme::c(theme::ACCENT))
                        .text_size(px(12.0))
                        .child(SharedString::from(icon.to_string())),
                )
                .child(
                    div()
                        .flex_grow()
                        .text_size(px(12.0))
                        .text_color(theme::c(theme::TEXT))
                        .child(SharedString::from(rel)),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(theme::c(theme::MUTED))
                        .child(SharedString::from(format!(":{}", line))),
                ),
        )
        .child(
            div()
                .pl_6()
                .flex()
                .flex_row()
                .flex_wrap()
                .font_family("Menlo")
                .text_size(px(11.0))
                .children(highlight(
                    &m.text,
                    &hi_indices,
                    theme::c(theme::SUBTEXT),
                    theme::c(theme::MATCH),
                )),
        )
        .into_any_element()
}

fn settings_file_display() -> String {
    std::env::var_os("HOME")
        .map(|h| {
            PathBuf::from(h)
                .join(".config/file-explorer/settings.txt")
                .to_string_lossy()
                .into_owned()
        })
        .unwrap_or_else(|| "settings.txt".into())
}

fn short_path(p: &str) -> String {
    let home = std::env::var_os("HOME")
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !home.is_empty() && p.starts_with(&home) {
        format!("~{}", &p[home.len()..])
    } else {
        p.to_string()
    }
}

fn is_image(ext: &str) -> bool {
    matches!(
        ext,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "tif" | "tiff" | "svg"
    )
}

fn is_quicklookable(ext: &str) -> bool {
    matches!(
        ext,
        "pdf"
            | "mp4"
            | "mov"
            | "m4v"
            | "mkv"
            | "webm"
            | "mp3"
            | "m4a"
            | "wav"
            | "aiff"
            | "docx"
            | "doc"
            | "xlsx"
            | "xls"
            | "pptx"
            | "ppt"
            | "key"
            | "numbers"
            | "pages"
            | "heic"
            | "psd"
    )
}

fn is_textual(ext: &str) -> bool {
    matches!(
        ext,
        "rs" | "toml" | "lock" | "md" | "txt" | "json" | "yaml" | "yml" | "js" | "ts" | "tsx"
            | "jsx" | "py" | "go" | "java" | "c" | "h" | "cpp" | "hpp" | "css" | "html" | "xml"
            | "sh" | "bash" | "zsh" | "fish" | "ini" | "cfg" | "conf" | "log" | "csv" | "sql"
            | "rb" | "swift" | "kt" | "scala" | "lua" | "vim" | "el" | ""
            | "gitignore" | "gitattributes" | "env" | "dockerfile" | "makefile"
    )
}

fn preview_body(path: &std::path::Path, is_dir: bool) -> gpui::AnyElement {
    use gpui::img;

    if is_dir {
        // If the folder mostly contains images, render a small thumbnail grid.
        let images: Vec<PathBuf> = std::fs::read_dir(path)
            .ok()
            .into_iter()
            .flat_map(|rd| rd.filter_map(|r| r.ok()))
            .filter(|de| de.file_type().map(|t| t.is_file()).unwrap_or(false))
            .map(|de| de.path())
            .filter(|p| {
                p.extension()
                    .and_then(|s| s.to_str())
                    .map(|e| is_image(&e.to_ascii_lowercase()))
                    .unwrap_or(false)
            })
            .take(12)
            .collect();
        if images.len() >= 4 {
            return div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(theme::c(theme::MUTED))
                        .child(SharedString::from(format!(
                            "{} images in this folder",
                            images.len()
                        ))),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap_1()
                        .children(images.into_iter().map(|p| {
                            div()
                                .w(px(82.0))
                                .h(px(82.0))
                                .rounded_md()
                                .overflow_hidden()
                                .child(
                                    img(p)
                                        .max_w(px(82.0))
                                        .max_h(px(82.0)),
                                )
                                .into_any_element()
                        })),
                )
                .into_any_element();
        }

        let mut names: Vec<String> = std::fs::read_dir(path)
            .ok()
            .into_iter()
            .flat_map(|rd| rd.filter_map(|r| r.ok()))
            .map(|de| {
                let n = de.file_name().to_string_lossy().into_owned();
                if de.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    format!("{}/", n)
                } else {
                    n
                }
            })
            .collect();
        names.sort();
        let total = names.len();
        let shown: Vec<gpui::AnyElement> = names
            .into_iter()
            .take(40)
            .map(|n| {
                div()
                    .text_size(px(11.0))
                    .text_color(if n.ends_with('/') {
                        theme::c(theme::DIR)
                    } else {
                        theme::c(theme::SUBTEXT)
                    })
                    .child(SharedString::from(n))
                    .into_any_element()
            })
            .collect();
        return div()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(theme::c(theme::MUTED))
                    .child(SharedString::from(format!("{} entries", total))),
            )
            .children(shown)
            .into_any_element();
    }

    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if is_image(&ext) {
        return div()
            .flex()
            .items_center()
            .justify_center()
            .py_2()
            .child(
                img(path.to_path_buf())
                    .max_w(px(260.0))
                    .max_h(px(260.0))
                    .rounded_md(),
            )
            .into_any_element();
    }

    if is_quicklookable(&ext) {
        if let Some(thumb) = thumb_for(path) {
            return div()
                .flex()
                .flex_col()
                .items_center()
                .gap_2()
                .py_2()
                .child(
                    img(thumb)
                        .max_w(px(260.0))
                        .max_h(px(260.0))
                        .rounded_md(),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .text_color(theme::c(theme::MUTED))
                        .child("Quick Look preview"),
                )
                .into_any_element();
        }
    }

    if is_textual(&ext)
        || path
            .file_name()
            .and_then(|s| s.to_str())
            .map(|n| is_textual(&n.to_ascii_lowercase()))
            .unwrap_or(false)
    {
        let meta = std::fs::metadata(path).ok();
        let size = meta.map(|m| m.len()).unwrap_or(0);
        if size == 0 {
            return div()
                .text_size(px(11.0))
                .text_color(theme::c(theme::MUTED))
                .child("(empty file)")
                .into_any_element();
        }
        if size > 256 * 1024 {
            return div()
                .text_size(px(11.0))
                .text_color(theme::c(theme::MUTED))
                .child("(file too large to preview)")
                .into_any_element();
        }
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let rows = highlight::highlight_lines(&content, &ext, 60);
                let lines: Vec<gpui::AnyElement> = rows
                    .into_iter()
                    .map(|row| {
                        let spans: Vec<gpui::AnyElement> = row
                            .into_iter()
                            .map(|s| {
                                let (r, g, b) = s.rgb;
                                let color = gpui::Rgba {
                                    r: r as f32 / 255.0,
                                    g: g as f32 / 255.0,
                                    b: b as f32 / 255.0,
                                    a: 1.0,
                                };
                                div()
                                    .text_color(color)
                                    .child(SharedString::from(s.text))
                                    .into_any_element()
                            })
                            .collect();
                        div()
                            .flex()
                            .flex_row()
                            .font_family("Menlo")
                            .text_size(px(11.0))
                            .children(spans)
                            .into_any_element()
                    })
                    .collect();
                return div().flex().flex_col().children(lines).into_any_element();
            }
            Err(_) => {
                return div()
                    .text_size(px(11.0))
                    .text_color(theme::c(theme::MUTED))
                    .child("(could not read)")
                    .into_any_element();
            }
        }
    }

    div()
        .text_size(px(11.0))
        .text_color(theme::c(theme::MUTED))
        .child("(binary file — press Space to Quick Look)")
        .into_any_element()
}

fn footer(sort: SortMode, toast: Option<&str>) -> gpui::AnyElement {
    let left = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .flex_grow()
        .child(hint("↵", "open"))
        .child(hint("⌘↵", "reveal"))
        .child(hint("⇥", "descend"))
        .child(hint("⇧⇥", "up"))
        .child(hint("␣", "quicklook"))
        .child(hint("⌘L", "go to"))
        .child(hint("⌘F", "find"))
        .child(hint("⌘C", "copy"))
        .child(hint("⌘⇧C", "copy name"))
        .child(hint("⌘⌫", "trash"))
        .child(hint("⌘F2", "batch rename"))
        .child(hint("⌘Z", "undo"))
        .child(hint("⌘B", "bookmark"))
        .child(hint("⌘A", "select all"))
        .child(hint("⌘J", "sort"))
        .child(hint("⌘T", "new tab"))
        .child(hint("⌘W", "close tab"))
        .child(hint("⌘⇧N", "new folder"))
        .child(hint("⌘V", "paste here"))
        .child(hint("⌘\\", "tree"))
        .child(hint("⌘N", "new win"))
        .child(hint("⌘,", "settings"))
        .child(hint("⌘H", "hidden"))
        .child(hint("⌘R", "refresh"));

    let right = div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .child(
            div()
                .px_2()
                .py_0p5()
                .rounded_sm()
                .bg(theme::c(theme::SEL))
                .text_color(theme::c(theme::ACCENT_BAR))
                .child(SharedString::from(format!("⇅ {}", sort.label()))),
        )
        .child(if let Some(msg) = toast {
            div()
                .px_2()
                .py_0p5()
                .rounded_sm()
                .bg(theme::c(theme::ACCENT_BAR))
                .text_color(theme::c(0x11111b))
                .child(SharedString::from(msg.to_string()))
                .into_any_element()
        } else {
            div().into_any_element()
        });

    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_4()
        .px_4()
        .py_2()
        .bg(theme::t(theme::SURFACE_ALT))
        .border_t_1()
        .border_color(theme::c(theme::BORDER))
        .text_size(px(11.0))
        .text_color(theme::c(theme::MUTED))
        .child(left)
        .child(right)
        .into_any_element()
}

fn meta_row(label: &str, value: &str) -> gpui::AnyElement {
    div()
        .flex()
        .flex_row()
        .justify_between()
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::c(theme::MUTED))
                .child(SharedString::from(label.to_string())),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(theme::c(theme::SUBTEXT))
                .child(SharedString::from(value.to_string())),
        )
        .into_any_element()
}

fn hint(key: &str, label: &str) -> gpui::AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_1()
        .child(
            div()
                .px_1p5()
                .rounded_sm()
                .bg(theme::c(theme::SEL))
                .text_color(theme::c(theme::SUBTEXT))
                .child(SharedString::from(key.to_string())),
        )
        .child(
            div()
                .text_color(theme::c(theme::MUTED))
                .child(SharedString::from(label.to_string())),
        )
        .into_any_element()
}

fn format_time(secs: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let diff = now.saturating_sub(secs) as i64;
    if diff < 60 {
        format!("{}s ago", diff)
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86_400 {
        format!("{}h ago", diff / 3600)
    } else if diff < 86_400 * 30 {
        format!("{}d ago", diff / 86_400)
    } else {
        format!("{}mo ago", diff / (86_400 * 30))
    }
}

fn main() {
    let cli = std::env::args().nth(1).map(PathBuf::from);
    let session = if cli.is_none() {
        load_session()
    } else {
        Vec::new()
    };
    let root = cli.clone().unwrap_or_else(|| {
        session
            .first()
            .map(|t| t.root.clone())
            .unwrap_or_else(|| dirs_home().unwrap_or_else(|| PathBuf::from(".")))
    });
    let restore_tabs: Vec<TabSnapshot> = if cli.is_none() { session } else { Vec::new() };

    application().run(move |cx: &mut App| {
        cx.bind_keys([KeyBinding::new("escape", Quit, None)]);
        cx.on_action(|_: &Quit, cx: &mut App| cx.quit());

        let bounds = Bounds::centered(None, gpui::size(px(1080.0), px(680.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_background: WindowBackgroundAppearance::Blurred,
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("File Explorer".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|cx| {
                    let mut exp = Explorer::new(root.clone(), cx);
                    if let Some(first) = restore_tabs.first().cloned() {
                        exp.tabs[0] = first.clone();
                        exp.restore_snapshot(first, cx);
                    }
                    for t in restore_tabs.iter().skip(1) {
                        if t.root.exists() {
                            exp.tabs.push(t.clone());
                        }
                    }
                    exp
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

gpui::actions!(app, [Quit]);

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}
