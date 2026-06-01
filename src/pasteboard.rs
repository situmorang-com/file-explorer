//! Write real file URLs to the macOS general pasteboard so that ⌘V in
//! Finder / Mail / TextEdit pastes the actual *file*, not the path string.
//!
//! We shell out to `osascript` rather than touching AppKit directly — the
//! objc2 dance for `NSPasteboard.writeObjects` against `NSArray<NSURL>` is
//! fragile across crate versions, and the AppleScript is a one-liner that
//! Apple has maintained forever.

use std::path::Path;

pub fn write_files(paths: &[&Path]) {
    if paths.is_empty() {
        return;
    }
    let items: Vec<String> = paths
        .iter()
        .map(|p| {
            format!(
                "POSIX file \"{}\"",
                p.to_string_lossy().replace('"', "\\\"")
            )
        })
        .collect();
    let script = format!("set the clipboard to {{{}}}", items.join(", "));
    let _ = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
