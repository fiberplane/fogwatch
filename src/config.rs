use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct Auth {
    pub api_token: Option<String>,
    pub account_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UI {
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_detail_width")]
    pub default_detail_width: u8,
    #[serde(default = "default_max_log_entries")]
    pub max_log_entries: usize,
    #[serde(default = "default_timestamp_format")]
    pub timestamp_format: String,
    #[serde(default = "default_true")]
    pub show_status_bar: bool,
    #[serde(default = "default_true")]
    pub enable_mouse: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Logs {
    #[serde(default = "default_true")]
    pub syntax_highlight: bool,
    #[serde(default = "default_log_level")]
    pub default_log_level: String,
    #[serde(default = "default_true")]
    pub auto_scroll: bool,
    #[serde(default = "default_true")]
    pub preserve_scroll: bool,
    #[serde(default = "default_false")]
    pub show_line_numbers: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Workers {
    #[serde(default = "default_true")]
    pub auto_reconnect: bool,
    #[serde(default = "default_reconnect_interval")]
    pub reconnect_interval: u64,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_environments")]
    pub environments: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Advanced {
    #[serde(default = "default_websocket_timeout")]
    pub websocket_timeout: u64,
    #[serde(default = "default_max_retry_attempts")]
    pub max_retry_attempts: u32,
    #[serde(default = "default_false")]
    pub debug_mode: bool,
    #[serde(default = "default_false")]
    pub log_persistence: bool,
    #[serde(default = "default_log_file")]
    pub log_file: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub auth: Auth,
    #[serde(default)]
    pub ui: UI,
    #[serde(default)]
    pub logs: Logs,
    #[serde(default)]
    pub workers: Workers,
    #[serde(default)]
    pub advanced: Advanced,
}

impl Default for Auth {
    fn default() -> Self {
        Self {
            api_token: None,
            account_id: None,
        }
    }
}

impl Default for UI {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            default_detail_width: default_detail_width(),
            max_log_entries: default_max_log_entries(),
            timestamp_format: default_timestamp_format(),
            show_status_bar: default_true(),
            enable_mouse: default_true(),
        }
    }
}

impl Default for Logs {
    fn default() -> Self {
        Self {
            syntax_highlight: default_true(),
            default_log_level: default_log_level(),
            auto_scroll: default_true(),
            preserve_scroll: default_true(),
            show_line_numbers: default_false(),
        }
    }
}

impl Default for Workers {
    fn default() -> Self {
        Self {
            auto_reconnect: default_true(),
            reconnect_interval: default_reconnect_interval(),
            batch_size: default_batch_size(),
            environments: default_environments(),
        }
    }
}

impl Default for Advanced {
    fn default() -> Self {
        Self {
            websocket_timeout: default_websocket_timeout(),
            max_retry_attempts: default_max_retry_attempts(),
            debug_mode: default_false(),
            log_persistence: default_false(),
            log_file: default_log_file(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auth: Auth::default(),
            ui: UI::default(),
            logs: Logs::default(),
            workers: Workers::default(),
            advanced: Advanced::default(),
        }
    }
}

// Default value functions
fn default_theme() -> String {
    "dark".to_string()
}
fn default_detail_width() -> u8 {
    50
}
fn default_max_log_entries() -> usize {
    1000
}
fn default_timestamp_format() -> String {
    "RFC3339".to_string()
}
fn default_true() -> bool {
    true
}
fn default_false() -> bool {
    false
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_reconnect_interval() -> u64 {
    5
}
fn default_batch_size() -> usize {
    100
}
fn default_environments() -> Vec<String> {
    vec!["production".to_string()]
}
fn default_websocket_timeout() -> u64 {
    30
}
fn default_max_retry_attempts() -> u32 {
    3
}
fn default_log_file() -> String {
    "~/.fogwatch/logs".to_string()
}

impl Config {
    pub fn load() -> Result<Self> {
        // Try loading from project directory first
        let project_config = std::env::current_dir()?.join("fogwatch.toml");

        // Then try user config directory
        let user_config = Self::get_config_path()?;

        // Load and merge configs in order of precedence
        let config = if project_config.exists() {
            let config_str = fs::read_to_string(&project_config).with_context(|| {
                format!(
                    "Failed to read project config file: {}",
                    project_config.display()
                )
            })?;

            toml::from_str(&config_str).with_context(|| {
                format!(
                    "Failed to parse project config file: {}",
                    project_config.display()
                )
            })?
        } else if user_config.exists() {
            let config_str = fs::read_to_string(&user_config).with_context(|| {
                format!("Failed to read user config file: {}", user_config.display())
            })?;

            toml::from_str(&config_str).with_context(|| {
                format!(
                    "Failed to parse user config file: {}",
                    user_config.display()
                )
            })?
        } else {
            Config::default()
        };

        Ok(config)
    }

    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        // Try to save to project directory first
        let project_config = std::env::current_dir()?.join("fogwatch.toml");
        if project_config.exists() {
            let config_str = toml::to_string_pretty(self).context("Failed to serialize config")?;

            fs::write(&project_config, config_str).with_context(|| {
                format!(
                    "Failed to write project config file: {}",
                    project_config.display()
                )
            })?;

            return Ok(());
        }

        // Fall back to user config directory
        let user_config = Self::get_config_path()?;

        // Create parent directories if they don't exist
        if let Some(parent) = user_config.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create config directory: {}", parent.display())
            })?;
        }

        let config_str = toml::to_string_pretty(self).context("Failed to serialize config")?;

        fs::write(&user_config, config_str).with_context(|| {
            format!(
                "Failed to write user config file: {}",
                user_config.display()
            )
        })?;

        Ok(())
    }

    fn get_config_path() -> Result<PathBuf> {
        let home = dirs::home_dir().context("Failed to determine home directory")?;

        Ok(home.join(".config").join("fogwatch").join("config.toml"))
    }

    #[allow(dead_code)]
    pub fn merge_environment(&mut self) {
        if let Ok(token) = std::env::var("CLOUDFLARE_API_TOKEN") {
            self.auth.api_token = Some(token);
        }
        if let Ok(account_id) = std::env::var("CLOUDFLARE_ACCOUNT_ID") {
            self.auth.account_id = Some(account_id);
        }
    }
}
