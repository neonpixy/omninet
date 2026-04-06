//! # Nexus -- Federation & Interoperability
//!
//! The bridge. Nexus connects Omnidea to the legacy world -- export to PDF, DOCX,
//! XLSX, HTML, images; import from those formats back into .idea Digits; bridge
//! to external protocols (SMTP, ActivityPub, RSS, etc.).
//!
//! ## Architecture
//!
//! Three plug-and-play subsystems, each backed by a trait registry:
//!
//! - **Export** -- `Exporter` trait + `ExporterRegistry`. Takes `&[Digit]` and
//!   produces bytes in any supported format. Profiles group formats by use case.
//! - **Import** -- `Importer` trait + `ImporterRegistry`. Reads external file
//!   bytes and produces Digits.
//! - **Bridge** -- `ProtocolBridge` trait + `BridgeRegistry`. Translates
//!   Equipment `MailMessage` payloads to/from external protocols.
//!
//! ## Design Decisions
//!
//! - Exporters take `&[Digit]` + `Option<Uuid>`, not `&DocumentState`. This
//!   keeps Nexus decoupled from Magic internals -- the caller extracts digits
//!   from `DocumentState` before calling export.
//! - Publish (on Globe) is primary; export (via Nexus) is the legacy escape
//!   hatch. Deterministic templates, optional AI polish.
//! - All registries follow Magic's `RendererRegistry` pattern: HashMap-backed,
//!   register/get/list, `with_defaults()` constructor.
//!
//! ## Covenant Alignment
//!
//! **Sovereignty** -- your data, your formats. Export means you can always leave.
//! **Dignity** -- quality exports respect the design intent (Regalia theming).
//! **Consent** -- bridge protocols are explicit opt-in registrations.

mod config;
mod error;
mod federation_scope;
mod output;
mod profile;
mod registry;
mod traits;

pub mod bridge;
pub mod export;
pub mod import;

// Error
pub use error::NexusError;

// Federation
pub use federation_scope::FederationScope;

// Config
pub use config::{BridgeConfig, ExportConfig, ExportFormat, ExportQuality, ImportConfig, MergeStrategy};

// Output
pub use output::{BridgeResult, ExportOutput, ImportOutput};

// Traits
pub use traits::{Exporter, Importer, ProtocolBridge};

// Registry
pub use registry::{BridgeRegistry, ExporterRegistry, ImporterRegistry};

// Profile
pub use profile::{ExportProfile, profile_formats};
