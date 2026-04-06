use std::fmt;

/// Errors from the Omnibus runtime.
#[derive(Debug)]
pub enum OmnibusError {
    /// Identity not loaded — call create_identity or load_identity first.
    NoIdentity,
    /// Server failed to start.
    ServerFailed(String),
    /// Discovery failed.
    DiscoveryFailed(String),
    /// Identity operation failed.
    IdentityFailed(String),
    /// Network operation failed.
    NetworkFailed(String),
    /// Crown error.
    Crown(crown::CrownError),
    /// Globe error.
    Globe(globe::GlobeError),
}

impl fmt::Display for OmnibusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoIdentity => write!(f, "no identity loaded"),
            Self::ServerFailed(msg) => write!(f, "server failed: {msg}"),
            Self::DiscoveryFailed(msg) => write!(f, "discovery failed: {msg}"),
            Self::IdentityFailed(msg) => write!(f, "identity failed: {msg}"),
            Self::NetworkFailed(msg) => write!(f, "network failed: {msg}"),
            Self::Crown(e) => write!(f, "crown: {e}"),
            Self::Globe(e) => write!(f, "globe: {e}"),
        }
    }
}

impl std::error::Error for OmnibusError {}

impl From<crown::CrownError> for OmnibusError {
    fn from(e: crown::CrownError) -> Self {
        Self::Crown(e)
    }
}

impl From<globe::GlobeError> for OmnibusError {
    fn from(e: globe::GlobeError) -> Self {
        Self::Globe(e)
    }
}
