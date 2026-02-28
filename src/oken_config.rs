use serde::Deserialize;

fn default_reconnect() -> bool {
    true
}
fn default_retries() -> u32 {
    3
}
fn default_delay() -> u64 {
    5
}
fn default_keepalive() -> u32 {
    60
}
fn default_danger_tags() -> Vec<String> {
    vec!["prod".to_string(), "production".to_string()]
}

#[derive(Deserialize)]
pub struct OkenConfig {
    #[serde(default = "default_reconnect")]
    pub reconnect: bool,
    #[serde(default = "default_retries")]
    pub reconnect_retries: u32,
    #[serde(default = "default_delay")]
    pub reconnect_delay_secs: u64,
    #[serde(default = "default_keepalive")]
    pub keepalive_interval: u32,
    #[serde(default = "default_danger_tags")]
    pub danger_tags: Vec<String>,
}

impl Default for OkenConfig {
    fn default() -> Self {
        Self {
            reconnect: default_reconnect(),
            reconnect_retries: default_retries(),
            reconnect_delay_secs: default_delay(),
            keepalive_interval: default_keepalive(),
            danger_tags: default_danger_tags(),
        }
    }
}

/// Load config from `~/.config/oken/config.toml`. Falls back to defaults on missing/invalid file.
pub fn load_config() -> OkenConfig {
    load_config_impl().unwrap_or_default()
}

fn load_config_impl() -> Option<OkenConfig> {
    let config_dir = crate::config::config_dir().ok()?;
    let path = config_dir.join("config.toml");
    let content = std::fs::read_to_string(path).ok()?;
    toml::from_str(&content).ok()
}
