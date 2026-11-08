// src/config.rs

use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use directories::{BaseDirs, ProjectDirs};

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub general: GeneralConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GeneralConfig {
    /// Refresh interval in seconds (fractional values allowed, e.g. 0.5).
    #[serde(default)]
    pub refresh_interval: Option<f64>,

    /// Refresh interval in milliseconds. Takes precedence over `refresh_interval`.
    #[serde(default)]
    pub refresh_interval_ms: Option<u64>,

    #[serde(default = "default_show_directory")]
    pub show_directory: bool,


    #[serde(default = "default_path_display_mode")]
    pub path_display_mode: String,

    #[serde(default = "default_show_git_branch")]
    pub show_git_branch: bool,

    #[serde(default = "default_idle_threshold")]
    pub idle_threshold_secs: u64,

    #[serde(default = "default_clear_when_closed")]
    pub clear_when_closed: bool,

    #[serde(default)]
    pub exclude: Vec<String>,

    #[serde(default)]
    pub blacklist_commands: Vec<String>,

    #[serde(default = "default_client_id")]
    pub client_id: String,

    #[serde(default = "default_large_image")]
    pub large_image: String,

    #[serde(default = "default_small_image")]
    pub small_image: String,
}

fn default_client_id() -> String { "1429846275737518222".to_string() }


const DEFAULT_REFRESH_INTERVAL_MS: u64 = 500;
const MIN_REFRESH_INTERVAL_MS: u64 = 100;

fn default_show_directory() -> bool { true }
fn default_path_display_mode() -> String { "folder_only".to_string() }
fn default_show_git_branch() -> bool { true }
fn default_idle_threshold() -> u64 { 300 }
fn default_clear_when_closed() -> bool { true }
fn default_large_image() -> String { "ghostty".to_string() }
fn default_small_image() -> String { "terminal".to_string() }

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig {
                refresh_interval: None,
                refresh_interval_ms: None,

                show_directory: default_show_directory(),
                path_display_mode: default_path_display_mode(),
                show_git_branch: default_show_git_branch(),
                idle_threshold_secs: default_idle_threshold(),
                clear_when_closed: default_clear_when_closed(),
                exclude: Vec::new(),
                blacklist_commands: vec![
                    "sudo".to_string(),
                    "ssh".to_string(),
                    "op".to_string(),
                    "bw".to_string(),
                    "pass".to_string(),
                ],
                client_id: default_client_id(),
                large_image: default_large_image(),
                small_image: default_small_image(),

            },
        }
    }
}

impl GeneralConfig {
    /// Effective refresh interval in milliseconds.
    ///
    /// Precedence: `refresh_interval_ms`, then `refresh_interval` (seconds),
    /// then the default. Clamped to a minimum to avoid busy-looping.
    pub fn refresh_interval_ms(&self) -> u64 {
        let ms = self
            .refresh_interval_ms
            .or_else(|| {
                self.refresh_interval
                    .filter(|s| s.is_finite() && *s >= 0.0)
                    .map(|s| (s * 1000.0) as u64)
            })
            .unwrap_or(DEFAULT_REFRESH_INTERVAL_MS);
        ms.max(MIN_REFRESH_INTERVAL_MS)
    }
}

impl Config {
    /// Loads the configuration from the default location, falling back to defaults if missing or invalid.
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_path = match find_config_path() {
            Some(p) => p,
            None => {
                log::info!(
                    "No config file found (looked in {}), using defaults",
                    candidate_config_paths()
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                return Ok(Config::default());
            }
        };
        let config_content = fs::read_to_string(&config_path)?;
        match toml::from_str::<Config>(&config_content) {
            Ok(config) => {
                log::info!("Loaded config from {}", config_path.display());
                Ok(config)
            }
            Err(err) => {
                log::warn!(
                    "Failed to parse config {}: {} (using defaults)",
                    config_path.display(),
                    err
                );
                Ok(Config::default())
            }
        }
    }
}

/// Loads the configuration from a specific path.
pub fn load_config(path: PathBuf) -> Result<Config, Box<dyn std::error::Error>> {
    let config_content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&config_content)?;
    Ok(config)
}



/// Candidate config file locations, in priority order.
///
/// The documented location is `$XDG_CONFIG_HOME/ghostty-rpc/config.toml`
/// (defaulting to `~/.config/ghostty-rpc/config.toml`) on every platform.
/// The platform-native directory is kept as a fallback for older installs
/// (e.g. `~/Library/Application Support/com.your_name.ghostty-rpc` on macOS).
fn candidate_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    let xdg_config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| BaseDirs::new().map(|b| b.home_dir().join(".config")));
    if let Some(dir) = xdg_config {
        paths.push(dir.join("ghostty-rpc").join("config.toml"));
    }

    if let Some(project_dirs) = ProjectDirs::from("com", "your_name", "ghostty-rpc") {
        let legacy = project_dirs.config_dir().join("config.toml");
        if !paths.contains(&legacy) {
            paths.push(legacy);
        }
    }

    paths
}

/// Returns the first existing config file, if any.
fn find_config_path() -> Option<PathBuf> {
    candidate_config_paths().into_iter().find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(toml_src: &str) -> GeneralConfig {
        toml::from_str::<Config>(toml_src).unwrap().general
    }

    #[test]
    fn refresh_interval_seconds() {
        assert_eq!(parse("[general]\nrefresh_interval = 1").refresh_interval_ms(), 1000);
        assert_eq!(parse("[general]\nrefresh_interval = 2.5").refresh_interval_ms(), 2500);
    }

    #[test]
    fn refresh_interval_ms_takes_precedence() {
        let cfg = parse("[general]\nrefresh_interval = 5\nrefresh_interval_ms = 250");
        assert_eq!(cfg.refresh_interval_ms(), 250);
    }

    #[test]
    fn refresh_interval_default() {
        assert_eq!(parse("[general]").refresh_interval_ms(), DEFAULT_REFRESH_INTERVAL_MS);
        assert_eq!(Config::default().general.refresh_interval_ms(), DEFAULT_REFRESH_INTERVAL_MS);
    }

    #[test]
    fn refresh_interval_clamped() {
        assert_eq!(parse("[general]\nrefresh_interval = 0").refresh_interval_ms(), MIN_REFRESH_INTERVAL_MS);
        assert_eq!(parse("[general]\nrefresh_interval_ms = 0").refresh_interval_ms(), MIN_REFRESH_INTERVAL_MS);
    }
}
