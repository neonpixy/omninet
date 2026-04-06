//! Error types for the AppCatalog.

use serde::{Deserialize, Serialize};

/// Errors that can occur during app catalog operations.
#[derive(Clone, Debug, thiserror::Error, Serialize, Deserialize, PartialEq)]
pub enum AppCatalogError {
    /// The manifest failed validation.
    #[error("invalid manifest: {0}")]
    ManifestInvalid(String),

    /// Signature verification of a manifest failed.
    #[error("signature verification failed")]
    SignatureInvalid,

    /// No app with the given ID exists in the catalog.
    #[error("app not found: {0}")]
    AppNotFound(String),

    /// The requested version does not exist in the manifest.
    #[error("version not found: {0}")]
    VersionNotFound(String),

    /// The app does not support the requested platform.
    #[error("incompatible platform: {0}")]
    IncompatiblePlatform(String),

    /// Attempted to install an app that is already installed.
    #[error("app already installed: {0}")]
    AlreadyInstalled(String),

    /// Attempted to uninstall an app that is not installed.
    #[error("app not installed: {0}")]
    NotInstalled(String),
}
