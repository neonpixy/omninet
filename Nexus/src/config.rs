use serde::{Deserialize, Serialize};

/// Supported export formats.
///
/// Each variant maps to a file format that Nexus can produce. Exporters
/// declare which formats they support via `Exporter::supported_formats()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// Portable Document Format for print and archival.
    Pdf,
    /// Lossless raster image.
    Png,
    /// Lossy compressed raster image.
    Jpg,
    /// Scalable Vector Graphics for web and illustration.
    Svg,
    /// Microsoft Word document.
    Docx,
    /// Microsoft Excel spreadsheet.
    Xlsx,
    /// Microsoft PowerPoint presentation.
    Pptx,
    /// OpenDocument text format.
    Odt,
    /// OpenDocument spreadsheet format.
    Ods,
    /// OpenDocument presentation format.
    Odp,
    /// Comma-separated values for tabular data.
    Csv,
    /// JSON data interchange format.
    Json,
    /// Markdown plain text with formatting.
    Markdown,
    /// Plain text with no formatting.
    Txt,
    /// Semantic HTML for the web.
    Html,
}

impl ExportFormat {
    /// File extension for this format (without the leading dot).
    pub fn extension(&self) -> &str {
        match self {
            Self::Pdf => "pdf",
            Self::Png => "png",
            Self::Jpg => "jpg",
            Self::Svg => "svg",
            Self::Docx => "docx",
            Self::Xlsx => "xlsx",
            Self::Pptx => "pptx",
            Self::Odt => "odt",
            Self::Ods => "ods",
            Self::Odp => "odp",
            Self::Csv => "csv",
            Self::Json => "json",
            Self::Markdown => "md",
            Self::Txt => "txt",
            Self::Html => "html",
        }
    }

    /// MIME type for this format.
    pub fn mime_type(&self) -> &str {
        match self {
            Self::Pdf => "application/pdf",
            Self::Png => "image/png",
            Self::Jpg => "image/jpeg",
            Self::Svg => "image/svg+xml",
            Self::Docx => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            Self::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Self::Pptx => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            Self::Odt => "application/vnd.oasis.opendocument.text",
            Self::Ods => "application/vnd.oasis.opendocument.spreadsheet",
            Self::Odp => "application/vnd.oasis.opendocument.presentation",
            Self::Csv => "text/csv",
            Self::Json => "application/json",
            Self::Markdown => "text/markdown",
            Self::Txt => "text/plain",
            Self::Html => "text/html",
        }
    }
}

/// Output quality level for export operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportQuality {
    /// Fast, lower-fidelity output for previews.
    Draft,
    /// Balanced quality for everyday use.
    #[default]
    Standard,
    /// Maximum fidelity for print or archival.
    HighQuality,
}

/// Configuration for an export operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    /// Target output format.
    pub format: ExportFormat,
    /// Quality level.
    pub quality: ExportQuality,
    /// Optional page dimensions in points (width, height). 1 point = 1/72 inch.
    pub page_size: Option<(f64, f64)>,
    /// Whether to include accessibility metadata in the output.
    pub accessibility: bool,
    /// Optional Regalia theme to apply during export.
    #[serde(skip)]
    pub theme: Option<regalia::Reign>,
}

impl ExportConfig {
    /// Create a new config for the given format with default settings.
    pub fn new(format: ExportFormat) -> Self {
        Self {
            format,
            quality: ExportQuality::default(),
            page_size: None,
            accessibility: true,
            theme: None,
        }
    }

    /// Set the quality level.
    pub fn with_quality(mut self, quality: ExportQuality) -> Self {
        self.quality = quality;
        self
    }

    /// Set the page size in points.
    pub fn with_page_size(mut self, width: f64, height: f64) -> Self {
        self.page_size = Some((width, height));
        self
    }

    /// Set the theme.
    pub fn with_theme(mut self, theme: regalia::Reign) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Disable accessibility metadata in output.
    pub fn without_accessibility(mut self) -> Self {
        self.accessibility = false;
        self
    }
}

/// Strategy for handling conflicts during import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MergeStrategy {
    /// Replace existing digits with imported ones.
    Replace,
    /// Append imported digits alongside existing ones.
    #[default]
    Append,
}

/// Configuration for an import operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportConfig {
    /// Author crown_id to assign to imported digits.
    pub author: String,
    /// How to handle conflicts with existing content.
    pub merge_strategy: MergeStrategy,
}

impl ImportConfig {
    /// Create a new import config for the given author.
    pub fn new(author: impl Into<String>) -> Self {
        Self {
            author: author.into(),
            merge_strategy: MergeStrategy::default(),
        }
    }

    /// Set the merge strategy.
    pub fn with_merge_strategy(mut self, strategy: MergeStrategy) -> Self {
        self.merge_strategy = strategy;
        self
    }
}

/// Configuration for a protocol bridge operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeConfig {
    /// Target protocol identifier (e.g., "smtp", "activitypub", "rss").
    pub protocol: String,
    /// Protocol-specific settings as a JSON object.
    pub settings: serde_json::Value,
}

impl BridgeConfig {
    /// Create a new bridge config for the given protocol.
    pub fn new(protocol: impl Into<String>) -> Self {
        Self {
            protocol: protocol.into(),
            settings: serde_json::Value::Object(serde_json::Map::new()),
        }
    }

    /// Add a setting to the config.
    pub fn with_setting(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        if let serde_json::Value::Object(ref mut map) = self.settings {
            map.insert(key.into(), value);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_format_extensions() {
        assert_eq!(ExportFormat::Pdf.extension(), "pdf");
        assert_eq!(ExportFormat::Docx.extension(), "docx");
        assert_eq!(ExportFormat::Markdown.extension(), "md");
        assert_eq!(ExportFormat::Html.extension(), "html");
    }

    #[test]
    fn export_format_mime_types() {
        assert_eq!(ExportFormat::Pdf.mime_type(), "application/pdf");
        assert_eq!(ExportFormat::Png.mime_type(), "image/png");
        assert_eq!(ExportFormat::Csv.mime_type(), "text/csv");
    }

    #[test]
    fn export_format_serde_round_trip() {
        let formats = vec![
            ExportFormat::Pdf,
            ExportFormat::Png,
            ExportFormat::Docx,
            ExportFormat::Markdown,
            ExportFormat::Html,
        ];
        for fmt in formats {
            let json = serde_json::to_string(&fmt).unwrap();
            let decoded: ExportFormat = serde_json::from_str(&json).unwrap();
            assert_eq!(fmt, decoded);
        }
    }

    #[test]
    fn export_config_builder() {
        let config = ExportConfig::new(ExportFormat::Pdf)
            .with_quality(ExportQuality::HighQuality)
            .with_page_size(612.0, 792.0)
            .without_accessibility();

        assert_eq!(config.format, ExportFormat::Pdf);
        assert_eq!(config.quality, ExportQuality::HighQuality);
        assert_eq!(config.page_size, Some((612.0, 792.0)));
        assert!(!config.accessibility);
        assert!(config.theme.is_none());
    }

    #[test]
    fn export_config_serde_round_trip() {
        let config = ExportConfig::new(ExportFormat::Png)
            .with_quality(ExportQuality::Draft)
            .with_page_size(1920.0, 1080.0);

        let json = serde_json::to_string(&config).unwrap();
        let decoded: ExportConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.format, ExportFormat::Png);
        assert_eq!(decoded.quality, ExportQuality::Draft);
        assert_eq!(decoded.page_size, Some((1920.0, 1080.0)));
    }

    #[test]
    fn import_config_serde_round_trip() {
        let config = ImportConfig::new("cpub1author")
            .with_merge_strategy(MergeStrategy::Replace);

        let json = serde_json::to_string(&config).unwrap();
        let decoded: ImportConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.author, "cpub1author");
        assert_eq!(decoded.merge_strategy, MergeStrategy::Replace);
    }

    #[test]
    fn bridge_config_serde_round_trip() {
        let config = BridgeConfig::new("smtp")
            .with_setting("host", serde_json::json!("mail.example.com"))
            .with_setting("port", serde_json::json!(587));

        let json = serde_json::to_string(&config).unwrap();
        let decoded: BridgeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.protocol, "smtp");
        assert_eq!(decoded.settings["host"], "mail.example.com");
        assert_eq!(decoded.settings["port"], 587);
    }

    #[test]
    fn merge_strategy_default() {
        assert_eq!(MergeStrategy::default(), MergeStrategy::Append);
    }

    #[test]
    fn export_quality_default() {
        assert_eq!(ExportQuality::default(), ExportQuality::Standard);
    }
}
