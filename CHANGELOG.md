# Changelog

## Unreleased

- `⌘C` (Copy Path) and drag-start now write **real file references** to the
  macOS pasteboard via `osascript`, so `⌘V` in Finder pastes the actual
  file rather than the path text. Path text still goes to gpui's clipboard
  for anything that wants strings.
- Double-click inside any modal input (Rename / Create / Batch rename) now
  selects the word at the cursor — same affordance as native Mac text fields.
- Catppuccin Latte light theme that mirrors macOS system appearance every
  frame. Drag the window between displays with different appearances and the
  palette flips live.
- `SPARKLE.md` — full integration roadmap for replacing the GitHub-Releases
  toast check with real Sparkle silent auto-update. Code shim is ~30 lines;
  the blockers are env (Ed25519 keypair, hosted appcast, Sparkle.framework).
- AVIF and HEIC images now preview inline and in result rows.
- Extended supported image set with TGA, DDS, HDR, EXR, QOI (gpui already decodes these).
- File tree pane now uses `uniform_list` — true virtualization, native trackpad scroll.
- Tree-pane keyboard navigation: `⌥↑/⌥↓` move, `⌥→` expand/descend, `⌥←` collapse/parent, `⌥↵` activate. The header bar inside the tree shows the bindings as a one-line reminder.
- `scripts/screenshot.sh` — `screencapture -o -w` helper for grabbing a clean PNG of the live window into `docs/screenshot.png`.

## v0.1.0 — 2026-06-01

Initial public release.

### Features
- Fuzzy filename search via nucleo with streaming results
- Content search (`⌘F`) via the `grep` crates (ripgrep's matcher + searcher)
- Jump-to-line on content hits via `cursor` / `code` / fallback to `open`
- Syntax-highlighted previews (syntect + two-face) — TypeScript, Astro, Vue included
- Inline Quick Look thumbnails for PDFs, videos, Office docs (cached via `qlmanage`)
- Inline image thumbnails directly in result rows (for files ≤ 2 MB)
- Translucent macOS-native UI with Metal blur backdrop, Catppuccin Mocha palette
- Emoji-based file icons (no font install needed)
- Tabs with per-tab state, persisted across launches
- VS Code–style file tree pane (`⌘\`)
- Right-click context menu (Open / Reveal / Quick Look / Copy Path / Rename / Trash)
- Multi-select with `Shift+Click` / `⌘+Click`, batch trash, batch rename with live preview
- Undo (`⌘Z`) for renames and moves, persisted to disk
- Settings overlay (`⌘,`) — hidden files, default sort
- Path bar (`⌘L`) for type-to-jump navigation
- Bookmarks (`⌘B`), Recents, pinned Locations (Home/Docs/Downloads/Desktop/Apps)
- Self-update check against GitHub Releases
- `cargo bundle` integration for `.app` packaging
- Real text-input cursor with selection, copy/cut/paste, word/line navigation
- Virtualized result list (uniform_list) — scales to 100k+ matches with native trackpad scroll
- Animated caret blink in modal inputs
- Live filesystem watcher (notify) with debounced refresh
