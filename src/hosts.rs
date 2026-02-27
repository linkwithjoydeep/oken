use std::collections::HashMap;

use anyhow::Result;

use crate::config;
use crate::hosts_toml;
use crate::ssh_config;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostSource {
    SshConfig,
    HostsToml,
}

#[derive(Debug, Clone)]
pub struct Host {
    pub alias: String,
    pub hostname: Option<String>,
    pub user: Option<String>,
    pub port: Option<u16>,
    pub identity_file: Option<String>,
    pub tags: Vec<String>,
    pub source: HostSource,
}

/// Load all hosts from ssh_config and hosts.toml, with hosts.toml winning on conflicts.
pub fn list_all_hosts() -> Result<Vec<Host>> {
    let mut hosts_map: HashMap<String, Host> = HashMap::new();

    // 1. Load from ~/.ssh/config
    let ssh_hosts = ssh_config::parse_ssh_config().unwrap_or_default();
    for alias in ssh_hosts {
        hosts_map.insert(
            alias.clone(),
            Host {
                alias,
                hostname: None, // resolved lazily via ssh -G
                user: None,
                port: None,
                identity_file: None,
                tags: Vec::new(),
                source: HostSource::SshConfig,
            },
        );
    }

    // 2. Overlay from hosts.toml (wins on conflict)
    let config_dir = config::config_dir()?;
    let toml_path = config_dir.join("hosts.toml");
    let toml_hosts = hosts_toml::load_hosts_toml(&toml_path).unwrap_or_default();
    for (alias, entry) in toml_hosts {
        hosts_map.insert(
            alias.clone(),
            Host {
                alias,
                hostname: Some(entry.hostname),
                user: entry.user,
                port: entry.port,
                identity_file: entry.identity_file,
                tags: entry.tags,
                source: HostSource::HostsToml,
            },
        );
    }

    let mut hosts: Vec<Host> = hosts_map.into_values().collect();
    hosts.sort_by(|a, b| a.alias.cmp(&b.alias));
    Ok(hosts)
}
