use std::path::PathBuf;

use anyhow::{Context, Result};

fn home() -> Result<PathBuf> {
    dirs::home_dir().context("could not determine home directory")
}

/// Returns `$XDG_CONFIG_HOME/oken` or `~/.config/oken`.
pub fn config_dir() -> Result<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().unwrap().join(".config"));
    let dir = base.join("oken");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create config dir: {}", dir.display()))?;
    Ok(dir)
}

/// Returns `$XDG_DATA_HOME/oken` or `~/.local/share/oken`.
pub fn data_dir() -> Result<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().unwrap().join(".local/share"));
    let dir = base.join("oken");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create data dir: {}", dir.display()))?;
    Ok(dir)
}
