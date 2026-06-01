# file-explorer

A translucent, fuzzy, content-searchable file explorer for macOS — built in Rust with [GPUI](https://www.gpui.rs/) (the same UI framework that powers [Zed](https://zed.dev/)).

Designed to feel like Spotlight, Raycast, and Telescope had a kid that does what Finder won't.

> macOS only. Linux/Windows aren't supported yet because the renderer uses Metal.

## Why

Finder is fine for browsing, but slow when you actually know what you're looking for. `fzf` is fast but command-line only. This is the in-between: native-feeling, GPU-rendered, with `fzf`-style filtering and `ripgrep`-style content search baked in.

## Features

- **Fuzzy filename search** powered by [nucleo](https://github.com/helix-editor/nucleo) (Helix's matcher). Streams results as the walker discovers files — `~/` indexes in under a second.
- **Content search** (`⌘F`) using the [grep crates](https://docs.rs/grep) — ripgrep's matcher and searcher, gitignore-aware, multi-threaded. Enter on a hit opens the file at the matching line in VS Code / Cursor / your `$EDITOR`.
- **Syntax-highlighted previews** for ~150 languages via [syntect](https://github.com/trishume/syntect) + [two-face](https://github.com/eth-p/two-face) (TypeScript, Astro, Vue included). Images, PDFs, Office docs, videos, and audio get inline Quick Look thumbnails (cached via `qlmanage`).
- **Translucent macOS-native UI** — full Metal vibrancy, Catppuccin Mocha palette, emoji-based file icons (no font install needed), bold yellow folder names.
- **Tabs** (`⌘T` / `⌘W` / `⌘⇧]` / `⌘⇧[`) with per-tab state saved across launches.
- **In-window tree pane** (`⌘\`) — VS Code–style disclosure triangles.
- **Multi-select** with `Shift+Click` / `⌘+Click`, batch trash and batch rename with a live find/replace preview.
- **Undo** (`⌘Z`) for renames and moves, persisted across sessions.
- **Right-click context menu** — Open / Reveal / Quick Look / Copy Path / Rename / Trash.
- **Self-update check** against GitHub Releases (set `FILE_EXPLORER_UPDATE_REPO=owner/repo` at compile time).
- **Settings overlay** (`⌘,`) — toggle hidden files, sort default, persisted to `~/.config/file-explorer/settings.txt`.

## Install

You'll need Rust (1.91+), Xcode (full app, not just Command Line Tools), and the Metal toolchain:

```bash
sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
sudo xcodebuild -runFirstLaunch
sudo xcodebuild -downloadComponent MetalToolchain
xcrun metal --version   # should print a version
```

Then:

```bash
git clone https://github.com/situmorang-com/file-explorer.git
cd file-explorer
cargo run --release -- ~/code   # or any starting directory
```

For a real `.app` bundle (drag into `/Applications`):

```bash
cargo install cargo-bundle
cargo bundle --release
open "target/release/bundle/osx/File Explorer.app"
```

Optionally drop a 1024×1024 PNG and run `./scripts/make-icon.sh icon.png` to bundle a custom icon.

## Keyboard map

### Navigation
| Key | Action |
|---|---|
| `↑` `↓` / `Ctrl+J` `Ctrl+K` / `Ctrl+N` `Ctrl+P` | Move selection |
| `Ctrl+D` `Ctrl+U` | Half-page down/up |
| `PageDown` `PageUp` | Page down/up |
| `Home` `End` / `⌘↑` `⌘↓` | Jump to first/last |
| `Tab` | Descend into selected folder |
| `Shift+Tab` / `Backspace` (empty query) | Go to parent |
| `Enter` | Open (or jump-to-line in content mode) |
| `⌘Enter` | Reveal in Finder |
| `Space` | Quick Look preview |

### Search & mode
| Key | Action |
|---|---|
| Any printable | Append to query |
| `Backspace` | Delete query char (or ascend if empty) |
| `⌘F` | Toggle content search (ripgrep mode) |
| `⌘L` | "Go to" path bar |
| `⌘J` | Cycle sort: relevance / name / size / modified |
| `⌘H` | Toggle hidden files |
| `⌘R` | Refresh / re-walk |

### File operations
| Key | Action |
|---|---|
| `⌘C` | Copy path(s) of selection / marked |
| `⌘⇧C` | Copy basename(s) |
| `⌘V` | Paste-move clipboard paths into selected folder |
| `⌘⌫` | Move selection / marked to Trash |
| `F2` | Rename |
| `⌘F2` | Batch rename marked (find/replace template, live preview) |
| `⌘⇧N` | New folder |
| `⌘⌥N` | New file |
| `⌘Z` | Undo |

### Selection
| Key | Action |
|---|---|
| `⌘+Click` | Toggle mark on row |
| `Shift+Click` | Range-mark from last anchor |
| `⌘A` | Mark all visible results |
| `⌘D` | Clear marks |

### Tabs & windows
| Key | Action |
|---|---|
| `⌘T` | New tab |
| `⌘W` | Close tab |
| `⌘⇧]` `⌘⇧[` | Next / previous tab |
| `⌘N` | New window |
| `⌘1`–`⌘6` | Jump to pinned location |
| `⌘B` | Bookmark current root |
| `⌘\` | Toggle file tree pane |
| `⌘,` | Settings overlay |
| `Esc` | Close overlay / quit |

### Text editing in modals
Path bar, Rename, Create, Batch rename all support:
- `←` `→` (chars), `⌥←` `⌥→` (words), `⌘←` `⌘→` / `Home` `End` (line)
- `Backspace`, `⌥Backspace`, `⌘Backspace`, `Delete`
- `Shift` + any motion above to extend selection
- `⌘A` select all, `⌘C` copy, `⌘X` cut, `⌘V` paste

## Configuration

Files live under `~/.config/file-explorer/`:

| File | What it stores |
|---|---|
| `settings.txt` | `show_hidden=true/false`, `sort=relevance/name/size/modified` |
| `session.txt` | Open tabs with per-tab root, query, sort, selected index |
| `bookmarks.txt` | One bookmark path per line |
| `recents.txt` | Last 8 visited roots |
| `undo.txt` | Persisted undo stack (capped at 50 actions) |

## Architecture

- [src/main.rs](src/main.rs) — UI and Explorer state machine
- [src/index.rs](src/index.rs) — parallel `ignore::WalkBuilder` streaming into Nucleo
- [src/content_search.rs](src/content_search.rs) — ripgrep-style file-content search
- [src/watcher.rs](src/watcher.rs) — `notify` filesystem watcher with debounce
- [src/highlight.rs](src/highlight.rs) — `syntect` + `two-face` syntax highlighting
- [src/settings.rs](src/settings.rs) — settings file parser
- [src/updater.rs](src/updater.rs) — GitHub Releases version check

## What's not in here yet

- External drag-out to other apps (gpui doesn't expose `NSDraggingSource`; needs a small `objc2` shim).
- Full Sparkle auto-update (currently it only checks for updates and opens the release page).
- Linux / Windows ports — would mean a different renderer.
- Tree-pane keyboard navigation.

## Acknowledgements

Built on [GPUI](https://www.gpui.rs/) by Zed Industries. Search powered by [nucleo](https://github.com/helix-editor/nucleo) (Helix) and the [grep crates](https://docs.rs/grep) (ripgrep). Syntax highlighting by [syntect](https://github.com/trishume/syntect) + [two-face](https://github.com/eth-p/two-face).
