use std::fmt;

/// Errors from the Tower runtime.
#[derive(Debug)]
pub enum TowerError {
    /// Omnibus failed to start.
    OmnibusStartFailed(String),
    /// Identity creation/loading failed.
    IdentityFailed(String),
    /// Gospel peering failed.
    PeeringFailed(String),
    /// Announcement failed.
    AnnounceFailed(String),
    /// Configuration error.
    ConfigError(String),
    /// Underlying Omnibus error.
    Omnibus(omnibus::OmnibusError),
    /// Underlying Globe error.
    Globe(globe::GlobeError),
}

impl fmt::Display for TowerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OmnibusStartFailed(msg) => write!(f, "omnibus start failed: {msg}"),
            Self::IdentityFailed(msg) => write!(f, "identity failed: {msg}"),
            Self::PeeringFailed(msg) => write!(f, "peering failed: {msg}"),
            Self::AnnounceFailed(msg) => write!(f, "announcement failed: {msg}"),
            Self::ConfigError(msg) => write!(f, "config error: {msg}"),
            Self::Omnibus(e) => write!(f, "omnibus: {e}"),
            Self::Globe(e) => write!(f, "globe: {e}"),
        }
    }
}

impl std::error::Error for TowerError {}

impl From<omnibus::OmnibusError> for TowerError {
    fn from(e: omnibus::OmnibusError) -> Self {
        Self::Omnibus(e)
    }
}

impl From<globe::GlobeError> for TowerError {
    fn from(e: globe::GlobeError) -> Self {
        Self::Globe(e)
    }
}
