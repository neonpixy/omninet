//! TOML-based daemon configuration.
//!
//! `DaemonConfig` is a file-friendly representation of Omnibus settings.
//! Load it from a `.toml` file, then convert to `OmnibusConfig` for runtime use.
//! Closures (event filters, search handlers) are NOT part of the config file —
//! the daemon sets those after loading.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::OmnibusConfig;

/// Top-level daemon configuration, loadable from TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    /// Omnibus runtime settings.
    pub omnibus: OmnibusSection,
    /// Tower-specific settings (optional, defaults to disabled).
    #[serde(default)]
    pub tower: TowerSection,
}

/// Omnibus runtime section of the config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmnibusSection {
    /// Port for the local relay server. 0 = OS-assigned.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Whether to bind to all interfaces (true = LAN reachable).
    #[serde(default)]
    pub bind_all: bool,
    /// Human-readable device name for mDNS discovery.
    #[serde(default = "default_device_name")]
    pub device_name: String,
    /// Directory for persistent storage (Soul, relay database).
    #[serde(default)]
    pub data_dir: Option<PathBuf>,
    /// Whether to attempt UPnP port mapping.
    #[serde(default)]
    pub enable_upnp: bool,
    /// Home node URL for persistent sync.
    #[serde(default)]
    pub home_node: Option<String>,
}

/// Tower-specific section of the config file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TowerSection {
    /// Whether Tower mode is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Tower mode: "pharos" (gospel-only) or "harbor" (community content).
    #[serde(default = "default_tower_mode")]
    pub mode: String,
    /// Human-readable Tower name.
    #[serde(default = "default_tower_name")]
    pub name: String,
    /// Seed relay URLs for gospel peering.
    #[serde(default)]
    pub seeds: Vec<String>,
    /// Community pubkeys this Harbor serves (Harbor mode only).
    #[serde(default)]
    pub communities: Vec<String>,
    /// Lighthouse announcement interval in seconds.
    #[serde(default)]
    pub announce_interval_secs: Option<u64>,
    /// Gospel bilateral sync interval in seconds.
    #[serde(default)]
    pub gospel_interval_secs: Option<u64>,
    /// Gospel live sync polling interval in seconds.
    #[serde(default)]
    pub gospel_live_interval_secs: Option<u64>,
    /// Public URL for lighthouse announcements (optional).
    #[serde(default)]
    pub public_url: Option<String>,
}

impl Default for TowerSection {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: default_tower_mode(),
            name: default_tower_name(),
            seeds: Vec::new(),
            communities: Vec::new(),
            announce_interval_secs: None,
            gospel_interval_secs: None,
            gospel_live_interval_secs: None,
            public_url: None,
        }
    }
}

fn default_port() -> u16 {
    4040
}

fn default_device_name() -> String {
    "Omnidea Device".into()
}

fn default_tower_mode() -> String {
    "pharos".into()
}

fn default_tower_name() -> String {
    "My Tower".into()
}

impl DaemonConfig {
    /// Load a `DaemonConfig` from a TOML file.
    pub fn load(path: &Path) -> Result<Self, DaemonConfigError> {
        let contents = std::fs::read_to_string(path).map_err(|e| DaemonConfigError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        toml::from_str(&contents).map_err(|e| DaemonConfigError::Parse {
            path: path.to_path_buf(),
            source: e,
        })
    }

    /// Convert to an `OmnibusConfig` for runtime use.
    ///
    /// This sets all data-driven fields. Closure-based fields (event filter,
    /// search handler, server config) must be set by the caller after conversion.
    pub fn to_omnibus_config(&self) -> OmnibusConfig {
        let home_node = self
            .omnibus
            .home_node
            .as_deref()
            .and_then(|s| s.parse().ok());

        OmnibusConfig {
            data_dir: self.omnibus.data_dir.clone(),
            device_name: self.omnibus.device_name.clone(),
            port: self.omnibus.port,
            bind_all: self.omnibus.bind_all,
            home_node,
            enable_upnp: self.omnibus.enable_upnp,
            ..Default::default()
        }
    }
}

/// Errors from loading a daemon config file.
#[derive(Debug)]
pub enum DaemonConfigError {
    /// Failed to read the config file.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Failed to parse the TOML.
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
}

impl std::fmt::Display for DaemonConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(f, "failed to read config {}: {source}", path.display())
            }
            Self::Parse { path, source } => {
                write!(f, "failed to parse config {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for DaemonConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Parse { source, .. } => Some(source),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_minimal_toml() {
        let toml_str = r#"
[omnibus]
"#;
        let config: DaemonConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.omnibus.port, 4040);
        assert_eq!(config.omnibus.device_name, "Omnidea Device");
        assert!(!config.omnibus.bind_all);
        assert!(config.omnibus.data_dir.is_none());
        assert!(!config.omnibus.enable_upnp);
        assert!(config.omnibus.home_node.is_none());
        // Tower defaults.
        assert!(!config.tower.enabled);
        assert_eq!(config.tower.mode, "pharos");
        assert_eq!(config.tower.name, "My Tower");
        assert!(config.tower.seeds.is_empty());
    }

    #[test]
    fn parse_full_toml() {
        let toml_str = r#"
[omnibus]
port = 8080
bind_all = true
device_name = "Sam's Mac"
data_dir = "/var/lib/omnibus"
enable_upnp = true
home_node = "ws://192.168.1.10:4040"

[tower]
enabled = true
mode = "harbor"
name = "Community Tower"
seeds = ["ws://seed1.example.com", "ws://seed2.example.com"]
"#;
        let config: DaemonConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.omnibus.port, 8080);
        assert!(config.omnibus.bind_all);
        assert_eq!(config.omnibus.device_name, "Sam's Mac");
        assert_eq!(
            config.omnibus.data_dir,
            Some(PathBuf::from("/var/lib/omnibus"))
        );
        assert!(config.omnibus.enable_upnp);
        assert_eq!(
            config.omnibus.home_node,
            Some("ws://192.168.1.10:4040".into())
        );
        assert!(config.tower.enabled);
        assert_eq!(config.tower.mode, "harbor");
        assert_eq!(config.tower.name, "Community Tower");
        assert_eq!(config.tower.seeds.len(), 2);
    }

    #[test]
    fn to_omnibus_config_maps_fields() {
        let toml_str = r#"
[omnibus]
port = 9090
bind_all = true
device_name = "Test Node"
home_node = "ws://home.local:4040"
"#;
        let daemon: DaemonConfig = toml::from_str(toml_str).unwrap();
        let config = daemon.to_omnibus_config();

        assert_eq!(config.port, 9090);
        assert!(config.bind_all);
        assert_eq!(config.device_name, "Test Node");
        assert!(config.home_node.is_some());
        assert_eq!(
            config.home_node.unwrap().to_string(),
            "ws://home.local:4040/"
        );
    }

    #[test]
    fn to_omnibus_config_invalid_home_node_becomes_none() {
        let toml_str = r#"
[omnibus]
home_node = "not a valid url"
"#;
        let daemon: DaemonConfig = toml::from_str(toml_str).unwrap();
        let config = daemon.to_omnibus_config();
        assert!(config.home_node.is_none());
    }

    #[test]
    fn load_nonexistent_file_returns_io_error() {
        let result = DaemonConfig::load(Path::new("/tmp/nonexistent_omnibus_config.toml"));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, DaemonConfigError::Io { .. }));
    }

    #[test]
    fn load_from_temp_file() {
        let dir = std::env::temp_dir().join("omnibus_config_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.toml");
        std::fs::write(
            &path,
            r#"
[omnibus]
port = 5555
device_name = "File Test"
"#,
        )
        .unwrap();

        let config = DaemonConfig::load(&path).unwrap();
        assert_eq!(config.omnibus.port, 5555);
        assert_eq!(config.omnibus.device_name, "File Test");

        // Cleanup.
        let _ = std::fs::remove_dir_all(&dir);
    }
}
