//! AppCatalog — Application registry for Omnidea.
//!
//! A pure in-memory catalog of application manifests, their install states,
//! and lifecycle operations. AppCatalog is a registry — it does not perform
//! discovery, downloading, or platform-specific installation. Those are the
//! caller's responsibilities.
//!
//! # Architecture
//!
//! ```text
//! AppCatalog
//!   ├── manifest.rs   — AppManifest, AppVersion, Platform, Permission
//!   ├── catalog.rs    — AppCatalog, CatalogEntry, InstallStatus
//!   ├── lifecycle.rs  — InstallRequest, UpdateRequest, UninstallRequest, InstallAction
//!   └── error.rs      — AppCatalogError
//! ```
//!
//! # Dependencies
//!
//! - **Crown** — for signature verification of manifests (Signature type).
//! - No dependency on Globe or any networking crate.

pub mod catalog;
pub mod error;
pub mod lifecycle;
pub mod manifest;

pub use catalog::{AppCatalog, CatalogEntry, InstallStatus};
pub use error::AppCatalogError;
pub use lifecycle::{
    resolve_install_action, InstallAction, InstallRequest, UninstallRequest, UpdateRequest,
};
pub use manifest::{AppManifest, AppVersion, Permission, Platform};
