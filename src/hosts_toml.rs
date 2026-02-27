use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HostEntry {
    pub hostname: String,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct HostsFile {
    #[serde(default)]
    hosts: HashMap<String, HostEntry>,
}

/// Parse `~/.config/oken/hosts.toml` and return the hosts map.
/// Returns an empty map if the file doesn't exist.
pub fn load_hosts_toml(path: &Path) -> Result<HashMap<String, HostEntry>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let contents = std::fs::read_to_string(path)?;
    let file: HostsFile = toml::from_str(&contents)?;
    Ok(file.hosts)
}

/// Serialize and write hosts map back to the TOML file.
fn save_hosts_toml(path: &Path, hosts: &HashMap<String, HostEntry>) -> Result<()> {
    let file = HostsFile {
        hosts: hosts.clone(),
    };
    let contents = toml::to_string_pretty(&file)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)?;
    Ok(())
}

/// Add a host entry. Errors if the name already exists.
pub fn add_host(path: &Path, name: &str, entry: HostEntry) -> Result<()> {
    let mut hosts = load_hosts_toml(path)?;
    if hosts.contains_key(name) {
        bail!("host '{}' already exists", name);
    }
    hosts.insert(name.to_string(), entry);
    save_hosts_toml(path, &hosts)
}

/// Remove a host entry. Errors if the name doesn't exist.
pub fn remove_host(path: &Path, name: &str) -> Result<()> {
    let mut hosts = load_hosts_toml(path)?;
    if hosts.remove(name).is_none() {
        bail!("host '{}' not found", name);
    }
    save_hosts_toml(path, &hosts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_valid_hosts_toml() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        write!(
            tmp,
            r#"
[hosts.prod-web]
hostname = "10.0.1.50"
user = "deploy"
port = 22
identity_file = "~/.ssh/key"
tags = ["prod", "web"]

[hosts.staging]
hostname = "10.0.2.10"
"#
        )
        .unwrap();

        let hosts = load_hosts_toml(tmp.path()).unwrap();
        assert_eq!(hosts.len(), 2);
        assert_eq!(hosts["prod-web"].hostname, "10.0.1.50");
        assert_eq!(hosts["prod-web"].user.as_deref(), Some("deploy"));
        assert_eq!(hosts["prod-web"].port, Some(22));
        assert_eq!(hosts["prod-web"].tags, vec!["prod", "web"]);
        assert_eq!(hosts["staging"].hostname, "10.0.2.10");
        assert!(hosts["staging"].user.is_none());
        assert!(hosts["staging"].tags.is_empty());
    }

    #[test]
    fn missing_file_returns_empty() {
        let hosts = load_hosts_toml(Path::new("/nonexistent/hosts.toml")).unwrap();
        assert!(hosts.is_empty());
    }
}
