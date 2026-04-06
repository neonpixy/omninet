use thiserror::Error;

/// Errors that can occur during Nexus operations.
#[derive(Debug, Error)]
pub enum NexusError {
    /// An export operation failed.
    #[error("export failed: {0}")]
    ExportFailed(String),

    /// An import operation failed.
    #[error("import failed: {0}")]
    ImportFailed(String),

    /// A protocol bridge operation failed.
    #[error("bridge failed: {0}")]
    BridgeFailed(String),

    /// The requested format is not supported.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    /// The provided configuration is invalid.
    #[error("invalid config: {0}")]
    InvalidConfig(String),

    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// Serialization or deserialization failed.
    #[error("serialization error: {0}")]
    SerializationError(String),

    /// The requested feature is not yet implemented.
    #[error("not implemented: {0}")]
    NotImplemented(String),
}

impl From<serde_json::Error> for NexusError {
    fn from(err: serde_json::Error) -> Self {
        NexusError::SerializationError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_messages() {
        let cases: Vec<(NexusError, &str)> = vec![
            (
                NexusError::ExportFailed("PDF generation crashed".into()),
                "export failed: PDF generation crashed",
            ),
            (
                NexusError::ImportFailed("corrupt file".into()),
                "import failed: corrupt file",
            ),
            (
                NexusError::BridgeFailed("SMTP timeout".into()),
                "bridge failed: SMTP timeout",
            ),
            (
                NexusError::UnsupportedFormat("bmp".into()),
                "unsupported format: bmp",
            ),
            (
                NexusError::InvalidConfig("missing page size".into()),
                "invalid config: missing page size",
            ),
            (
                NexusError::SerializationError("unexpected EOF".into()),
                "serialization error: unexpected EOF",
            ),
            (
                NexusError::NotImplemented("PPTX export".into()),
                "not implemented: PPTX export",
            ),
        ];

        for (err, expected) in cases {
            assert_eq!(err.to_string(), expected);
        }
    }

    #[test]
    fn io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let nexus_err: NexusError = io_err.into();
        assert!(matches!(nexus_err, NexusError::IoError(_)));
        assert!(nexus_err.to_string().contains("file missing"));
    }

    #[test]
    fn serde_error_conversion() {
        let json_err = serde_json::from_str::<serde_json::Value>("not valid json").unwrap_err();
        let nexus_err: NexusError = json_err.into();
        assert!(matches!(nexus_err, NexusError::SerializationError(_)));
    }
}
