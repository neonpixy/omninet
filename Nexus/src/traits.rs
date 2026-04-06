use uuid::Uuid;

use crate::config::{BridgeConfig, ExportConfig, ExportFormat, ImportConfig};
use crate::error::NexusError;
use crate::output::{BridgeResult, ExportOutput, ImportOutput};

/// A plugin that exports Digits to a legacy file format.
///
/// Exporters are stateless and thread-safe. Each implementation handles one
/// or more `ExportFormat` variants. Register them with `ExporterRegistry`.
///
/// # Example
///
/// ```ignore
/// struct MarkdownExporter;
///
/// impl Exporter for MarkdownExporter {
///     fn id(&self) -> &str { "markdown" }
///     fn display_name(&self) -> &str { "Markdown Exporter" }
///     fn supported_formats(&self) -> &[ExportFormat] { &[ExportFormat::Markdown] }
///     fn export(&self, digits: &[ideas::Digit], root_id: Option<Uuid>, config: &ExportConfig)
///         -> Result<ExportOutput, NexusError> {
///         // ...convert digits to markdown...
///     }
/// }
/// ```
pub trait Exporter: Send + Sync {
    /// Unique identifier for this exporter (e.g., "pdf", "markdown").
    fn id(&self) -> &str;

    /// Human-readable name (e.g., "PDF Exporter").
    fn display_name(&self) -> &str;

    /// Formats this exporter can produce.
    fn supported_formats(&self) -> &[ExportFormat];

    /// Export the given digits to the configured format.
    ///
    /// `root_id` identifies the top-level digit (document container) if one
    /// exists. Exporters use it to determine traversal order.
    fn export(
        &self,
        digits: &[ideas::Digit],
        root_id: Option<Uuid>,
        config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError>;
}

/// A plugin that imports legacy file data into Digits.
///
/// Importers are stateless and thread-safe. Each handles one or more MIME
/// types. Register them with `ImporterRegistry`.
///
/// # Example
///
/// ```ignore
/// struct CsvImporter;
///
/// impl Importer for CsvImporter {
///     fn id(&self) -> &str { "csv" }
///     fn display_name(&self) -> &str { "CSV Importer" }
///     fn supported_mime_types(&self) -> &[&str] { &["text/csv"] }
///     fn import(&self, data: &[u8], config: &ImportConfig)
///         -> Result<ImportOutput, NexusError> {
///         // ...parse CSV into digits...
///     }
/// }
/// ```
pub trait Importer: Send + Sync {
    /// Unique identifier for this importer (e.g., "csv", "docx").
    fn id(&self) -> &str;

    /// Human-readable name (e.g., "CSV Importer").
    fn display_name(&self) -> &str;

    /// MIME types this importer can handle.
    fn supported_mime_types(&self) -> &[&str];

    /// Import data from bytes into Digits.
    fn import(
        &self,
        data: &[u8],
        config: &ImportConfig,
    ) -> Result<ImportOutput, NexusError>;
}

/// A plugin that bridges Equipment mail messages to/from external protocols.
///
/// Protocol bridges translate between Omnidea's `MailMessage` format and
/// external systems (SMTP, ActivityPub, RSS, etc.). Register them with
/// `BridgeRegistry`.
///
/// # Example
///
/// ```ignore
/// struct SmtpBridge;
///
/// impl ProtocolBridge for SmtpBridge {
///     fn id(&self) -> &str { "smtp" }
///     fn display_name(&self) -> &str { "SMTP Email Bridge" }
///     fn bridge(&self, message: &equipment::MailMessage, config: &BridgeConfig)
///         -> Result<BridgeResult, NexusError> {
///         // ...send via SMTP...
///     }
/// }
/// ```
pub trait ProtocolBridge: Send + Sync {
    /// Unique identifier for this bridge (e.g., "smtp", "activitypub").
    fn id(&self) -> &str;

    /// Human-readable name (e.g., "SMTP Email Bridge").
    fn display_name(&self) -> &str;

    /// Translate and deliver a mail message via the external protocol.
    fn bridge(
        &self,
        message: &equipment::MailMessage,
        config: &BridgeConfig,
    ) -> Result<BridgeResult, NexusError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verify the traits are object-safe (can be used as dyn Trait).

    #[test]
    fn exporter_is_object_safe() {
        fn _accepts(_: &dyn Exporter) {}
    }

    #[test]
    fn importer_is_object_safe() {
        fn _accepts(_: &dyn Importer) {}
    }

    #[test]
    fn protocol_bridge_is_object_safe() {
        fn _accepts(_: &dyn ProtocolBridge) {}
    }
}
