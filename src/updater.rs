//! Minimal "check for updates" against a GitHub Releases JSON API.
//!
//! Shells out to `curl` (always present on macOS) so we don't bring in a
//! TLS/HTTP stack. Parses just enough of the JSON to find `tag_name` and
//! `html_url`. Compares to the running binary's CARGO_PKG_VERSION.
//!
//! Set FILE_EXPLORER_UPDATE_REPO at compile time (or override at runtime via
//! the same env var) to point at a `owner/repo` on GitHub. If unset, the
//! checker no-ops silently.

use std::process::{Command, Stdio};

const DEFAULT_REPO: Option<&str> = option_env!("FILE_EXPLORER_UPDATE_REPO");

pub struct UpdateInfo {
    pub latest: String,
    pub current: String,
    pub url: String,
}

pub fn check_for_updates() -> Option<UpdateInfo> {
    let repo = std::env::var("FILE_EXPLORER_UPDATE_REPO")
        .ok()
        .or_else(|| DEFAULT_REPO.map(String::from))?;
    let current = env!("CARGO_PKG_VERSION").to_string();

    let url = format!("https://api.github.com/repos/{}/releases/latest", repo);
    let out = Command::new("curl")
        .args([
            "-fsSL",
            "-H",
            "Accept: application/vnd.github+json",
            "-H",
            "User-Agent: file-explorer",
            "--max-time",
            "8",
            &url,
        ])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let body = String::from_utf8(out.stdout).ok()?;
    let tag = json_string_field(&body, "tag_name")?;
    let html = json_string_field(&body, "html_url").unwrap_or_default();
    let latest = tag.trim_start_matches('v').to_string();
    if is_newer(&latest, &current) {
        Some(UpdateInfo {
            latest,
            current,
            url: html,
        })
    } else {
        None
    }
}

/// Tiny string-field extractor: finds `"name":"…"` ignoring escapes inside.
fn json_string_field(body: &str, key: &str) -> Option<String> {
    let needle = format!("\"{}\"", key);
    let i = body.find(&needle)?;
    let after = &body[i + needle.len()..];
    let colon = after.find(':')?;
    let after = &after[colon + 1..];
    let quote = after.find('"')?;
    let after = &after[quote + 1..];
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

fn is_newer(a: &str, b: &str) -> bool {
    fn parts(s: &str) -> Vec<u32> {
        s.split('.')
            .map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>())
            .map(|s| s.parse().unwrap_or(0))
            .collect()
    }
    let pa = parts(a);
    let pb = parts(b);
    for i in 0..pa.len().max(pb.len()) {
        let av = pa.get(i).copied().unwrap_or(0);
        let bv = pb.get(i).copied().unwrap_or(0);
        if av > bv {
            return true;
        }
        if av < bv {
            return false;
        }
    }
    false
}
