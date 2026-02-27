use std::path::{Path, PathBuf};

use anyhow::Result;

/// Parse `~/.ssh/config` and return concrete host aliases (no wildcards).
pub fn parse_ssh_config() -> Result<Vec<String>> {
    let home = dirs::home_dir().unwrap_or_default();
    let config_path = home.join(".ssh/config");
    if !config_path.exists() {
        return Ok(Vec::new());
    }
    let mut hosts = Vec::new();
    parse_file(&config_path, &home, &mut hosts)?;
    hosts.sort();
    hosts.dedup();
    Ok(hosts)
}

fn parse_file(path: &Path, home: &Path, hosts: &mut Vec<String>) -> Result<()> {
    let contents = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Ok(()), // silently skip unreadable files
    };

    let mut in_match_block = false;

    for line in contents.lines() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let (keyword, value) = match split_keyword(trimmed) {
            Some(kv) => kv,
            None => continue,
        };

        let kw_lower = keyword.to_ascii_lowercase();

        match kw_lower.as_str() {
            "host" => {
                in_match_block = false;
                for alias in value.split_whitespace() {
                    // Skip wildcard patterns
                    if !alias.contains('*') && !alias.contains('?') {
                        hosts.push(alias.to_string());
                    }
                }
            }
            "match" => {
                in_match_block = true;
            }
            "include" if !in_match_block => {
                process_include(value, home, path, hosts)?;
            }
            _ => {}
        }
    }

    Ok(())
}

/// Split a line into keyword and value, handling both `Key Value` and `Key=Value`.
fn split_keyword(line: &str) -> Option<(&str, &str)> {
    // Handle `Key=Value`
    if let Some(eq_pos) = line.find('=') {
        let key = line[..eq_pos].trim();
        let val = line[eq_pos + 1..].trim();
        if !key.is_empty() && !val.is_empty() {
            return Some((key, val));
        }
    }
    // Handle `Key Value`
    let mut parts = line.splitn(2, char::is_whitespace);
    let key = parts.next()?;
    let val = parts.next()?.trim();
    if val.is_empty() {
        return None;
    }
    Some((key, val))
}

fn process_include(pattern: &str, home: &Path, config_path: &Path, hosts: &mut Vec<String>) -> Result<()> {
    let expanded = expand_tilde(pattern, home);

    // If not absolute, resolve relative to the directory containing the config file
    let base = if expanded.is_absolute() {
        expanded.to_string_lossy().to_string()
    } else {
        let parent = config_path.parent().unwrap_or(home);
        parent.join(&expanded).to_string_lossy().to_string()
    };

    // Use glob to expand wildcards
    let paths = match glob::glob(&base) {
        Ok(p) => p,
        Err(_) => return Ok(()),
    };

    for entry in paths {
        if let Ok(path) = entry {
            if path.is_file() {
                parse_file(&path, home, hosts)?;
            }
        }
    }

    Ok(())
}

fn expand_tilde(path: &str, home: &Path) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        home.join(rest)
    } else if path == "~" {
        home.to_path_buf()
    } else {
        PathBuf::from(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_host_lines() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        std::fs::write(
            &config,
            "Host foo bar\n  HostName example.com\n\nHost *.wild\n  HostName w.com\n\nHost baz\n  HostName b.com\n",
        )
        .unwrap();

        let home = dir.path();
        let mut hosts = Vec::new();
        parse_file(&config, home, &mut hosts).unwrap();
        assert!(hosts.contains(&"foo".to_string()));
        assert!(hosts.contains(&"bar".to_string()));
        assert!(hosts.contains(&"baz".to_string()));
        // wildcard should be skipped
        assert!(!hosts.iter().any(|h| h.contains('*')));
    }

    #[test]
    fn include_directive() {
        let dir = tempfile::tempdir().unwrap();
        let ssh_dir = dir.path().join(".ssh");
        std::fs::create_dir_all(&ssh_dir).unwrap();

        let included = ssh_dir.join("extra");
        std::fs::write(&included, "Host included-host\n  HostName i.com\n").unwrap();

        let config = ssh_dir.join("config");
        let mut f = std::fs::File::create(&config).unwrap();
        writeln!(f, "Include {}", included.display()).unwrap();
        writeln!(f, "Host main-host").unwrap();

        let home = dir.path();
        let mut hosts = Vec::new();
        parse_file(&config, home, &mut hosts).unwrap();
        assert!(hosts.contains(&"main-host".to_string()));
        assert!(hosts.contains(&"included-host".to_string()));
    }
}
