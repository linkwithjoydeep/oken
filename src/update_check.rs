use std::io::IsTerminal;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const CHECK_INTERVAL_SECS: u64 = 86_400; // 24 hours
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASES_API: &str =
    "https://api.github.com/repos/linkwithjoydeep/oken/releases/latest";

/// Show an update notice if a newer version was found by a previous check,
/// then kick off a background refresh if 24 h have elapsed.
/// Returns immediately — never blocks the SSH connection.
pub fn maybe_notify() {
    // Only print to interactive terminals; skip when piped or scripted.
    if !std::io::stderr().is_terminal() {
        return;
    }

    let Ok(state_path) = crate::config::data_dir().map(|d| d.join("update_state")) else {
        return;
    };

    // Show a notice if the cached state already knows about a newer version.
    if let Some(latest_tag) = read_cached_tag(&state_path) {
        let latest_ver = latest_tag.trim_start_matches('v');
        if is_newer(latest_ver, CURRENT_VERSION) {
            let install_cmd = if cfg!(windows) {
                "powershell -c \"irm https://github.com/linkwithjoydeep/oken/releases/latest/download/oken-installer.ps1 | iex\""
            } else {
                "curl -LsSf https://github.com/linkwithjoydeep/oken/releases/latest/download/oken-installer.sh | sh"
            };
            eprintln!(
                "\x1b[33moken {latest_tag} is available\x1b[0m \x1b[2m(you have v{CURRENT_VERSION})\x1b[0m"
            );
            eprintln!("\x1b[2mUpdate: {install_cmd}\x1b[0m");
        }
    }

    // Spawn a background thread to refresh the cache if 24 h have elapsed.
    // The result is written to disk and shown on the *next* invocation.
    if should_check(&state_path) {
        std::thread::spawn(move || {
            if let Ok(tag) = fetch_latest_tag() {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                // Format: "<unix_timestamp>\t<tag>"
                let _ = std::fs::write(&state_path, format!("{now}\t{tag}"));
            }
        });
    }
}

/// Immediately check for updates, print the result, and refresh the cache.
/// Used by `oken update`.
pub fn force_check() -> anyhow::Result<()> {
    print!("Checking for updates… ");
    std::io::Write::flush(&mut std::io::stdout())?;

    let tag = fetch_latest_tag()?;
    let latest_ver = tag.trim_start_matches('v');

    // Refresh the cache so the background check timer resets
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if let Ok(state_path) = crate::config::data_dir().map(|d| d.join("update_state")) {
        let _ = std::fs::write(state_path, format!("{now}\t{tag}"));
    }

    if is_newer(latest_ver, CURRENT_VERSION) {
        let install_cmd = if cfg!(windows) {
            "powershell -c \"irm https://github.com/linkwithjoydeep/oken/releases/latest/download/oken-installer.ps1 | iex\""
        } else {
            "curl -LsSf https://github.com/linkwithjoydeep/oken/releases/latest/download/oken-installer.sh | sh"
        };
        println!("{tag} is available (you have v{CURRENT_VERSION})");
        println!("Run: {install_cmd}");
    } else {
        println!("already up to date (v{CURRENT_VERSION})");
    }

    Ok(())
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn read_cached_tag(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    // Second whitespace-separated token is the tag
    content.split_whitespace().nth(1).map(str::to_string)
}

fn should_check(path: &std::path::Path) -> bool {
    let last_ts: u64 = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.split_whitespace().next().and_then(|t| t.parse().ok()))
        .unwrap_or(0);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    now.saturating_sub(last_ts) >= CHECK_INTERVAL_SECS
}

fn fetch_latest_tag() -> anyhow::Result<String> {
    let response = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .build()
        .get(RELEASES_API)
        .set("User-Agent", &format!("oken/{CURRENT_VERSION}"))
        .set("Accept", "application/vnd.github.v3+json")
        .call()?
        .into_string()?;

    extract_tag_name(&response)
}

fn extract_tag_name(json: &str) -> anyhow::Result<String> {
    let v: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| anyhow::anyhow!("invalid JSON from GitHub API: {e}"))?;
    v["tag_name"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("tag_name not found in GitHub API response"))
}

/// Returns true if `latest` is a higher semver than `current`.
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Option<(u32, u32, u32)> {
        let mut it = s.splitn(4, '.');
        Some((
            it.next()?.parse().ok()?,
            it.next()?.parse().ok()?,
            // strip pre-release suffix (e.g. "1-beta.1" → 1)
            it.next()?
                .split(['-', '+'])
                .next()?
                .parse()
                .ok()?,
        ))
    };
    matches!((parse(latest), parse(current)), (Some(l), Some(c)) if l > c)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_tag_name() {
        let json = r#"{"tag_name":"v0.2.0","name":"oken v0.2.0"}"#;
        assert_eq!(extract_tag_name(json).unwrap(), "v0.2.0");
    }

    #[test]
    fn detects_newer_version() {
        assert!(is_newer("0.2.0", "0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("0.1.1", "0.1.0"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }
}
