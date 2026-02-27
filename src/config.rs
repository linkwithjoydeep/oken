use std::path::PathBuf;

use anyhow::{Context, Result};

/// Returns the oken config directory (`$XDG_CONFIG_HOME/oken/` or `~/.config/oken/`).
pub fn config_dir() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .context("could not determine config directory")?
        .join("oken");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create config dir: {}", dir.display()))?;
    Ok(dir)
}

/// Returns the oken data directory (`$XDG_DATA_HOME/oken/` or `~/.local/share/oken/`).
pub fn data_dir() -> Result<PathBuf> {
    let dir = dirs::data_dir()
        .context("could not determine data directory")?
        .join("oken");
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("failed to create data dir: {}", dir.display()))?;
    Ok(dir)
}
