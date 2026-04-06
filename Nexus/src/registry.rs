use std::collections::HashMap;

use uuid::Uuid;

use crate::config::{BridgeConfig, ExportConfig, ExportFormat, ImportConfig};
use crate::error::NexusError;
use crate::federation_scope::FederationScope;
use crate::output::{BridgeResult, ExportOutput, ImportOutput};
use crate::traits::{Exporter, Importer, ProtocolBridge};

/// Registry of exporters, keyed by their `id()`.
///
/// Follows the same pattern as Magic's `RendererRegistry`: HashMap-backed,
/// register/get/list, with a `with_defaults()` constructor for built-in
/// exporters.
pub struct ExporterRegistry {
    exporters: HashMap<String, Box<dyn Exporter>>,
}

impl ExporterRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            exporters: HashMap::new(),
        }
    }

    /// Create a registry pre-loaded with all built-in exporters.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(crate::export::MarkdownExporter));
        registry.register(Box::new(crate::export::CsvExporter));
        registry.register(Box::new(crate::export::JsonExporter));
        registry.register(Box::new(crate::export::TxtExporter));
        registry.register(Box::new(crate::export::HtmlExporter));
        #[cfg(not(target_os = "ios"))]
        registry.register(Box::new(crate::export::PdfExporter));
        registry.register(Box::new(crate::export::XlsxExporter));
        registry.register(Box::new(crate::export::PngExporter));
        registry.register(Box::new(crate::export::JpgExporter));
        registry.register(Box::new(crate::export::SvgExporter));
        registry.register(Box::new(crate::export::DocxExporter));
        registry.register(Box::new(crate::export::PptxExporter));
        registry.register(Box::new(crate::export::OdpExporter));
        registry.register(Box::new(crate::export::OdtExporter));
        registry.register(Box::new(crate::export::OdsExporter));
        registry
    }

    /// Register an exporter. Replaces any existing exporter with the same id.
    pub fn register(&mut self, exporter: Box<dyn Exporter>) {
        self.exporters.insert(exporter.id().to_string(), exporter);
    }

    /// Get an exporter by id.
    pub fn get(&self, id: &str) -> Option<&dyn Exporter> {
        self.exporters.get(id).map(|e| e.as_ref())
    }

    /// Find the first exporter that supports the given format.
    pub fn find_for_format(&self, format: ExportFormat) -> Option<&dyn Exporter> {
        self.exporters
            .values()
            .find(|e| e.supported_formats().contains(&format))
            .map(|e| e.as_ref())
    }

    /// List all registered exporter ids.
    pub fn list(&self) -> Vec<&str> {
        self.exporters.keys().map(|k| k.as_str()).collect()
    }

    /// List all formats supported across all registered exporters.
    pub fn supported_formats(&self) -> Vec<ExportFormat> {
        let mut formats: Vec<ExportFormat> = self
            .exporters
            .values()
            .flat_map(|e| e.supported_formats().iter().copied())
            .collect();
        formats.sort_by_key(|f| format!("{f:?}"));
        formats.dedup();
        formats
    }

    /// Check if an exporter with the given id is registered.
    pub fn has(&self, id: &str) -> bool {
        self.exporters.contains_key(id)
    }

    /// Number of registered exporters.
    pub fn count(&self) -> usize {
        self.exporters.len()
    }

    /// Export using the first exporter that supports the configured format.
    ///
    /// Returns `NexusError::UnsupportedFormat` if no exporter handles it.
    pub fn export(
        &self,
        digits: &[ideas::Digit],
        root_id: Option<Uuid>,
        config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError> {
        let exporter = self
            .find_for_format(config.format)
            .ok_or_else(|| {
                NexusError::UnsupportedFormat(format!(
                    "no exporter registered for format {:?}",
                    config.format
                ))
            })?;
        exporter.export(digits, root_id, config)
    }

    /// Export, but only if the target community is visible under the given
    /// federation scope.
    ///
    /// Returns `NexusError::ExportFailed` if the community is defederated.
    pub fn export_scoped(
        &self,
        digits: &[ideas::Digit],
        root_id: Option<Uuid>,
        config: &ExportConfig,
        scope: &FederationScope,
        community_id: &str,
    ) -> Result<ExportOutput, NexusError> {
        if !scope.is_visible(community_id) {
            return Err(NexusError::ExportFailed(format!(
                "community '{community_id}' is not visible under current federation scope"
            )));
        }
        self.export(digits, root_id, config)
    }
}

impl Default for ExporterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry of importers, keyed by their `id()`.
pub struct ImporterRegistry {
    importers: HashMap<String, Box<dyn Importer>>,
}

impl ImporterRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            importers: HashMap::new(),
        }
    }

    /// Create a registry pre-loaded with all built-in importers.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(crate::import::MarkdownImporter));
        registry.register(Box::new(crate::import::CsvImporter));
        registry.register(Box::new(crate::import::JsonImporter));
        registry.register(Box::new(crate::import::XlsxImporter));
        registry.register(Box::new(crate::import::DocxImporter));
        registry.register(Box::new(crate::import::PptxImporter));
        #[cfg(not(target_os = "ios"))]
        registry.register(Box::new(crate::import::PdfImporter));
        registry
    }

    /// Register an importer. Replaces any existing importer with the same id.
    pub fn register(&mut self, importer: Box<dyn Importer>) {
        self.importers.insert(importer.id().to_string(), importer);
    }

    /// Get an importer by id.
    pub fn get(&self, id: &str) -> Option<&dyn Importer> {
        self.importers.get(id).map(|i| i.as_ref())
    }

    /// Find the first importer that supports the given MIME type.
    pub fn find_for_mime(&self, mime_type: &str) -> Option<&dyn Importer> {
        self.importers
            .values()
            .find(|i| i.supported_mime_types().contains(&mime_type))
            .map(|i| i.as_ref())
    }

    /// List all registered importer ids.
    pub fn list(&self) -> Vec<&str> {
        self.importers.keys().map(|k| k.as_str()).collect()
    }

    /// Check if an importer with the given id is registered.
    pub fn has(&self, id: &str) -> bool {
        self.importers.contains_key(id)
    }

    /// Number of registered importers.
    pub fn count(&self) -> usize {
        self.importers.len()
    }

    /// Import using the first importer that supports the given MIME type.
    ///
    /// Returns `NexusError::UnsupportedFormat` if no importer handles it.
    pub fn import(
        &self,
        data: &[u8],
        mime_type: &str,
        config: &ImportConfig,
    ) -> Result<ImportOutput, NexusError> {
        let importer = self
            .find_for_mime(mime_type)
            .ok_or_else(|| {
                NexusError::UnsupportedFormat(format!(
                    "no importer registered for MIME type: {mime_type}"
                ))
            })?;
        importer.import(data, config)
    }
}

impl Default for ImporterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry of protocol bridges, keyed by their `id()`.
pub struct BridgeRegistry {
    bridges: HashMap<String, Box<dyn ProtocolBridge>>,
}

impl BridgeRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            bridges: HashMap::new(),
        }
    }

    /// Create a registry pre-loaded with all built-in bridges.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(crate::bridge::SmtpBridge));
        registry
    }

    /// Register a bridge. Replaces any existing bridge with the same id.
    pub fn register(&mut self, bridge: Box<dyn ProtocolBridge>) {
        self.bridges.insert(bridge.id().to_string(), bridge);
    }

    /// Get a bridge by id.
    pub fn get(&self, id: &str) -> Option<&dyn ProtocolBridge> {
        self.bridges.get(id).map(|b| b.as_ref())
    }

    /// List all registered bridge ids.
    pub fn list(&self) -> Vec<&str> {
        self.bridges.keys().map(|k| k.as_str()).collect()
    }

    /// Check if a bridge with the given id is registered.
    pub fn has(&self, id: &str) -> bool {
        self.bridges.contains_key(id)
    }

    /// Number of registered bridges.
    pub fn count(&self) -> usize {
        self.bridges.len()
    }

    /// Bridge a message using the specified protocol.
    ///
    /// Returns `NexusError::UnsupportedFormat` if no bridge handles the
    /// protocol in the config.
    pub fn bridge(
        &self,
        message: &equipment::MailMessage,
        config: &BridgeConfig,
    ) -> Result<BridgeResult, NexusError> {
        let bridge = self
            .get(&config.protocol)
            .ok_or_else(|| {
                NexusError::UnsupportedFormat(format!(
                    "no bridge registered for protocol: {}",
                    config.protocol
                ))
            })?;
        bridge.bridge(message, config)
    }

    /// Bridge a message, but only if the target community is visible under
    /// the given federation scope.
    ///
    /// Returns `NexusError::BridgeFailed` if the community is defederated.
    /// Returns `NexusError::UnsupportedFormat` if no bridge handles the protocol.
    pub fn bridge_scoped(
        &self,
        message: &equipment::MailMessage,
        config: &BridgeConfig,
        scope: &FederationScope,
        community_id: &str,
    ) -> Result<BridgeResult, NexusError> {
        if !scope.is_visible(community_id) {
            return Err(NexusError::BridgeFailed(format!(
                "community '{community_id}' is not visible under current federation scope"
            )));
        }
        self.bridge(message, config)
    }

    /// List bridge IDs filtered by federation scope.
    ///
    /// Given a mapping of bridge ID to community ID, returns only the bridge
    /// IDs whose community is visible under the scope.
    pub fn list_scoped<'a>(
        &'a self,
        scope: &FederationScope,
        bridge_communities: &'a HashMap<String, String>,
    ) -> Vec<&'a str> {
        self.bridges
            .keys()
            .filter(|id| {
                bridge_communities
                    .get(id.as_str())
                    .map(|cid| scope.is_visible(cid))
                    .unwrap_or(true) // bridges without a community mapping are always visible
            })
            .map(|k| k.as_str())
            .collect()
    }
}

impl Default for BridgeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- Mock exporter for testing --

    struct MockJsonExporter;

    impl Exporter for MockJsonExporter {
        fn id(&self) -> &str {
            "json"
        }
        fn display_name(&self) -> &str {
            "JSON Exporter"
        }
        fn supported_formats(&self) -> &[ExportFormat] {
            &[ExportFormat::Json]
        }
        fn export(
            &self,
            digits: &[ideas::Digit],
            _root_id: Option<Uuid>,
            _config: &ExportConfig,
        ) -> Result<ExportOutput, NexusError> {
            let json = serde_json::to_string(digits)
                .map_err(|e| NexusError::SerializationError(e.to_string()))?;
            Ok(ExportOutput::new(
                json.into_bytes(),
                "export.json",
                "application/json",
            ))
        }
    }

    struct MockTxtExporter;

    impl Exporter for MockTxtExporter {
        fn id(&self) -> &str {
            "txt"
        }
        fn display_name(&self) -> &str {
            "Plain Text Exporter"
        }
        fn supported_formats(&self) -> &[ExportFormat] {
            &[ExportFormat::Txt, ExportFormat::Markdown]
        }
        fn export(
            &self,
            _digits: &[ideas::Digit],
            _root_id: Option<Uuid>,
            _config: &ExportConfig,
        ) -> Result<ExportOutput, NexusError> {
            Ok(ExportOutput::new(
                b"hello".to_vec(),
                "export.txt",
                "text/plain",
            ))
        }
    }

    // -- Mock importer for testing --

    struct MockCsvImporter;

    impl Importer for MockCsvImporter {
        fn id(&self) -> &str {
            "csv"
        }
        fn display_name(&self) -> &str {
            "CSV Importer"
        }
        fn supported_mime_types(&self) -> &[&str] {
            &["text/csv"]
        }
        fn import(
            &self,
            _data: &[u8],
            _config: &ImportConfig,
        ) -> Result<ImportOutput, NexusError> {
            Ok(ImportOutput::new(vec![], None)
                .with_warning("Mock import"))
        }
    }

    // -- Mock bridge for testing --

    struct MockSmtpBridge;

    impl ProtocolBridge for MockSmtpBridge {
        fn id(&self) -> &str {
            "smtp"
        }
        fn display_name(&self) -> &str {
            "Mock SMTP Bridge"
        }
        fn bridge(
            &self,
            _message: &equipment::MailMessage,
            _config: &BridgeConfig,
        ) -> Result<BridgeResult, NexusError> {
            Ok(BridgeResult::ok("sent", serde_json::json!({"id": "mock"})))
        }
    }

    // -- ExporterRegistry tests --

    #[test]
    fn exporter_registry_register_and_get() {
        let mut reg = ExporterRegistry::new();
        assert_eq!(reg.count(), 0);
        assert!(!reg.has("json"));

        reg.register(Box::new(MockJsonExporter));
        assert_eq!(reg.count(), 1);
        assert!(reg.has("json"));
        assert_eq!(reg.get("json").unwrap().display_name(), "JSON Exporter");
    }

    #[test]
    fn exporter_registry_list() {
        let mut reg = ExporterRegistry::new();
        reg.register(Box::new(MockJsonExporter));
        reg.register(Box::new(MockTxtExporter));

        let mut ids = reg.list();
        ids.sort();
        assert_eq!(ids, vec!["json", "txt"]);
    }

    #[test]
    fn exporter_registry_find_for_format() {
        let mut reg = ExporterRegistry::new();
        reg.register(Box::new(MockJsonExporter));
        reg.register(Box::new(MockTxtExporter));

        assert!(reg.find_for_format(ExportFormat::Json).is_some());
        assert!(reg.find_for_format(ExportFormat::Txt).is_some());
        assert!(reg.find_for_format(ExportFormat::Markdown).is_some());
        assert!(reg.find_for_format(ExportFormat::Pdf).is_none());
    }

    #[test]
    fn exporter_registry_supported_formats() {
        let mut reg = ExporterRegistry::new();
        reg.register(Box::new(MockJsonExporter));
        reg.register(Box::new(MockTxtExporter));

        let formats = reg.supported_formats();
        assert!(formats.contains(&ExportFormat::Json));
        assert!(formats.contains(&ExportFormat::Txt));
        assert!(formats.contains(&ExportFormat::Markdown));
    }

    #[test]
    fn exporter_registry_export_unsupported() {
        let reg = ExporterRegistry::new();
        let config = ExportConfig::new(ExportFormat::Pdf);
        let result = reg.export(&[], None, &config);
        assert!(matches!(result, Err(NexusError::UnsupportedFormat(_))));
    }

    #[test]
    fn exporter_registry_export_success() {
        let mut reg = ExporterRegistry::new();
        reg.register(Box::new(MockJsonExporter));

        let config = ExportConfig::new(ExportFormat::Json);
        let output = reg.export(&[], None, &config).unwrap();
        assert_eq!(output.mime_type, "application/json");
    }

    // -- ImporterRegistry tests --

    #[test]
    fn importer_registry_register_and_get() {
        let mut reg = ImporterRegistry::new();
        assert_eq!(reg.count(), 0);

        reg.register(Box::new(MockCsvImporter));
        assert_eq!(reg.count(), 1);
        assert!(reg.has("csv"));
        assert_eq!(reg.get("csv").unwrap().display_name(), "CSV Importer");
    }

    #[test]
    fn importer_registry_find_for_mime() {
        let mut reg = ImporterRegistry::new();
        reg.register(Box::new(MockCsvImporter));

        assert!(reg.find_for_mime("text/csv").is_some());
        assert!(reg.find_for_mime("application/pdf").is_none());
    }

    #[test]
    fn importer_registry_import_unsupported() {
        let reg = ImporterRegistry::new();
        let config = ImportConfig::new("cpub1test");
        let result = reg.import(b"data", "application/pdf", &config);
        assert!(matches!(result, Err(NexusError::UnsupportedFormat(_))));
    }

    #[test]
    fn importer_registry_import_success() {
        let mut reg = ImporterRegistry::new();
        reg.register(Box::new(MockCsvImporter));

        let config = ImportConfig::new("cpub1test");
        let output = reg.import(b"col1,col2\na,b", "text/csv", &config).unwrap();
        assert_eq!(output.warnings.len(), 1);
    }

    // -- BridgeRegistry tests --

    #[test]
    fn bridge_registry_register_and_get() {
        let mut reg = BridgeRegistry::new();
        assert_eq!(reg.count(), 0);

        reg.register(Box::new(MockSmtpBridge));
        assert_eq!(reg.count(), 1);
        assert!(reg.has("smtp"));
        assert_eq!(reg.get("smtp").unwrap().display_name(), "Mock SMTP Bridge");
    }

    #[test]
    fn bridge_registry_list() {
        let mut reg = BridgeRegistry::new();
        reg.register(Box::new(MockSmtpBridge));

        let ids = reg.list();
        assert_eq!(ids, vec!["smtp"]);
    }

    #[test]
    fn exporter_registry_with_defaults_has_builtin() {
        let reg = ExporterRegistry::with_defaults();
        #[cfg(not(target_os = "ios"))]
        assert_eq!(reg.count(), 15);
        #[cfg(target_os = "ios")]
        assert_eq!(reg.count(), 14);
        assert!(reg.has("markdown"));
        assert!(reg.has("csv"));
        assert!(reg.has("json"));
        assert!(reg.has("txt"));
        assert!(reg.has("html"));
        #[cfg(not(target_os = "ios"))]
        assert!(reg.has("nexus.pdf"));
        assert!(reg.has("nexus.xlsx"));
        assert!(reg.has("nexus.png"));
        assert!(reg.has("nexus.jpg"));
        assert!(reg.has("nexus.svg"));
        assert!(reg.has("nexus.docx"));
        assert!(reg.has("nexus.pptx"));
        assert!(reg.has("nexus.odp"));
        assert!(reg.has("nexus.odt"));
        assert!(reg.has("nexus.ods"));
    }

    #[test]
    fn importer_registry_with_defaults_has_builtin() {
        let reg = ImporterRegistry::with_defaults();
        #[cfg(not(target_os = "ios"))]
        assert_eq!(reg.count(), 7);
        #[cfg(target_os = "ios")]
        assert_eq!(reg.count(), 6);
        assert!(reg.has("nexus.markdown.import"));
        assert!(reg.has("nexus.csv.import"));
        assert!(reg.has("nexus.json.import"));
        assert!(reg.has("nexus.xlsx.import"));
        assert!(reg.has("nexus.docx.import"));
        assert!(reg.has("nexus.pptx.import"));
        #[cfg(not(target_os = "ios"))]
        assert!(reg.has("nexus.pdf.import"));
    }

    #[test]
    fn bridge_registry_with_defaults_has_builtin() {
        let reg = BridgeRegistry::with_defaults();
        assert_eq!(reg.count(), 1);
        assert!(reg.has("nexus.smtp.bridge"));
    }

    // -- FederationScope integration tests --

    fn test_mail_message() -> equipment::MailMessage {
        equipment::MailMessage {
            id: Uuid::new_v4(),
            from: "cpub1alice".into(),
            recipients: vec![equipment::MailRecipientEntry {
                recipient: equipment::MailRecipient {
                    crown_id: "cpub1bob".into(),
                    display_name: Some("Bob".into()),
                },
                role: equipment::RecipientRole::To,
            }],
            subject: "Test".into(),
            body: "Hello".into(),
            attachments: vec![],
            thread_id: None,
            in_reply_to: None,
            timestamp: chrono::Utc::now(),
            read: false,
        }
    }

    #[test]
    fn bridge_scoped_allows_visible_community() {
        let mut reg = BridgeRegistry::new();
        reg.register(Box::new(MockSmtpBridge));

        let scope = FederationScope::from_communities(["alpha", "beta"]);
        let msg = test_mail_message();
        let config = BridgeConfig::new("smtp");
        let result = reg.bridge_scoped(&msg, &config, &scope, "alpha");
        assert!(result.is_ok());
    }

    #[test]
    fn bridge_scoped_blocks_defederated_community() {
        let mut reg = BridgeRegistry::new();
        reg.register(Box::new(MockSmtpBridge));

        let scope = FederationScope::from_communities(["alpha"]);
        let msg = test_mail_message();
        let config = BridgeConfig::new("smtp");
        let result = reg.bridge_scoped(&msg, &config, &scope, "gamma");
        assert!(result.is_err());
        match result {
            Err(NexusError::BridgeFailed(msg)) => {
                assert!(msg.contains("gamma"));
                assert!(msg.contains("not visible"));
            }
            other => panic!("expected BridgeFailed, got {other:?}"),
        }
    }

    #[test]
    fn bridge_scoped_unrestricted_allows_all() {
        let mut reg = BridgeRegistry::new();
        reg.register(Box::new(MockSmtpBridge));

        let scope = FederationScope::new();
        let msg = test_mail_message();
        let config = BridgeConfig::new("smtp");
        let result = reg.bridge_scoped(&msg, &config, &scope, "any-community");
        assert!(result.is_ok());
    }

    #[test]
    fn bridge_list_scoped_filters_by_community() {
        let mut reg = BridgeRegistry::new();
        reg.register(Box::new(MockSmtpBridge));

        // Map the mock bridge to a community.
        let mut bridge_communities = HashMap::new();
        bridge_communities.insert("smtp".to_string(), "alpha".to_string());

        // Alpha is visible.
        let scope = FederationScope::from_communities(["alpha"]);
        let visible = reg.list_scoped(&scope, &bridge_communities);
        assert_eq!(visible, vec!["smtp"]);

        // Beta only -- smtp is not in beta.
        let scope = FederationScope::from_communities(["beta"]);
        let visible = reg.list_scoped(&scope, &bridge_communities);
        assert!(visible.is_empty());
    }

    #[test]
    fn bridge_list_scoped_unmapped_bridges_always_visible() {
        let mut reg = BridgeRegistry::new();
        reg.register(Box::new(MockSmtpBridge));

        // No community mapping for the bridge -- should always be visible.
        let bridge_communities = HashMap::new();
        let scope = FederationScope::from_communities(["beta"]);
        let visible = reg.list_scoped(&scope, &bridge_communities);
        assert_eq!(visible, vec!["smtp"]);
    }

    #[test]
    fn export_scoped_allows_visible_community() {
        let mut reg = ExporterRegistry::new();
        reg.register(Box::new(MockJsonExporter));

        let scope = FederationScope::from_communities(["alpha"]);
        let config = ExportConfig::new(ExportFormat::Json);
        let result = reg.export_scoped(&[], None, &config, &scope, "alpha");
        assert!(result.is_ok());
    }

    #[test]
    fn export_scoped_blocks_defederated_community() {
        let mut reg = ExporterRegistry::new();
        reg.register(Box::new(MockJsonExporter));

        let scope = FederationScope::from_communities(["alpha"]);
        let config = ExportConfig::new(ExportFormat::Json);
        let result = reg.export_scoped(&[], None, &config, &scope, "gamma");
        assert!(result.is_err());
        match result {
            Err(NexusError::ExportFailed(msg)) => {
                assert!(msg.contains("gamma"));
                assert!(msg.contains("not visible"));
            }
            other => panic!("expected ExportFailed, got {other:?}"),
        }
    }

    #[test]
    fn export_scoped_unrestricted_allows_all() {
        let mut reg = ExporterRegistry::new();
        reg.register(Box::new(MockJsonExporter));

        let scope = FederationScope::new();
        let config = ExportConfig::new(ExportFormat::Json);
        let result = reg.export_scoped(&[], None, &config, &scope, "any-community");
        assert!(result.is_ok());
    }
}
