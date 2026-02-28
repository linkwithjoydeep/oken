use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TunnelEntry {
    pub host: String,
    pub ssh_flags: Vec<String>,
}

pub fn load_tunnels(path: &Path) -> Result<HashMap<String, TunnelEntry>> {
    if !path.exists() {
        return Ok(HashMap::new());
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let parsed: HashMap<String, TunnelEntry> = toml::from_str(&content)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(parsed)
}

pub fn add_tunnel(path: &Path, name: &str, entry: TunnelEntry) -> Result<()> {
    let mut tunnels = load_tunnels(path)?;
    tunnels.insert(name.to_string(), entry);
    save_tunnels(path, &tunnels)
}

pub fn remove_tunnel(path: &Path, name: &str) -> Result<()> {
    let mut tunnels = load_tunnels(path)?;
    if tunnels.remove(name).is_none() {
        bail!("tunnel '{name}' not found");
    }
    save_tunnels(path, &tunnels)
}

fn save_tunnels(path: &Path, tunnels: &HashMap<String, TunnelEntry>) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string(tunnels)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Returns `~/.local/share/oken/tunnels/<name>.sock`
pub fn socket_path(name: &str) -> Result<PathBuf> {
    let data_dir = crate::config::data_dir()?;
    let tunnels_dir = data_dir.join("tunnels");
    std::fs::create_dir_all(&tunnels_dir)?;
    Ok(tunnels_dir.join(format!("{name}.sock")))
}

/// Check if a tunnel is running via SSH ControlMaster check.
pub fn is_running(name: &str, host: &str) -> bool {
    let Ok(sock) = socket_path(name) else {
        return false;
    };
    if !sock.exists() {
        return false;
    }
    let ssh = crate::ssh::find_ssh()
        .unwrap_or_else(|_| PathBuf::from("ssh"));
    Command::new(&ssh)
        .args(["-S", &sock.to_string_lossy(), "-O", "check", host])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
