# Assets

## App icon

Drop a 1024×1024 PNG anywhere on disk, then run from the repo root:

```
./scripts/make-icon.sh ~/Desktop/my-icon.png
```

That uses the macOS-native `sips` + `iconutil` tools to write
`assets/icon.icns` with all the required slot sizes. Uncomment the
`icon = ["assets/icon.icns"]` line in `Cargo.toml`, then:

```
cargo install cargo-bundle  # one-time
cargo bundle --release
open "target/release/bundle/osx/File Explorer.app"
```

## Self-update

The app checks GitHub Releases for a newer version on every launch (no telemetry,
runs in a background thread, fails silently if offline). To enable it for your
fork, set the env var at compile time so the binary embeds the repo:

```
FILE_EXPLORER_UPDATE_REPO=youruser/file-explorer cargo bundle --release
```

When a newer `tag_name` (e.g. `v0.2.0`) is found, the app toasts
`Update available: 0.1.0 → 0.2.0` and opens the release page in the browser.
No auto-install (yet); that needs Sparkle.framework or `axoupdater` integration.
