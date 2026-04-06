mod builder;
mod context;
pub mod html;
mod name_resolver;
pub mod react;
pub mod swiftui;

pub use builder::CodeBuilder;
pub use context::ProjectionContext;
pub use html::HtmlProjection;
pub use name_resolver::NameResolver;
pub use react::ReactProjection;
pub use swiftui::SwiftUIProjection;

use serde::{Deserialize, Serialize};

/// Generated file contents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FileContents {
    /// UTF-8 source code or markup.
    Text(String),
    /// Raw binary data (images, compiled assets).
    Binary(Vec<u8>),
}

/// A generated output file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratedFile {
    pub relative_path: String,
    pub contents: FileContents,
}

impl GeneratedFile {
    /// Create a text file with the given relative path and content.
    pub fn text(path: impl Into<String>, contents: impl Into<String>) -> Self {
        Self {
            relative_path: path.into(),
            contents: FileContents::Text(contents.into()),
        }
    }

    /// Create a binary file with the given relative path and data.
    pub fn binary(path: impl Into<String>, data: Vec<u8>) -> Self {
        Self {
            relative_path: path.into(),
            contents: FileContents::Binary(data),
        }
    }
}

/// Trait for code projection targets (SwiftUI, React, Flutter, HTML, etc.).
///
/// Each implementation is a plugin that reads the ProjectionContext and
/// emits GeneratedFiles. Quality improves independently per target.
pub trait CodeProjection: Send + Sync {
    /// Target name (e.g. "SwiftUI", "React", "Flutter").
    fn name(&self) -> &str;

    /// File extension for the primary output (e.g. "swift", "tsx", "dart").
    fn file_extension(&self) -> &str;

    /// Generate all output files from the projection context.
    fn project(
        &self,
        context: &ProjectionContext,
    ) -> Result<Vec<GeneratedFile>, crate::error::MagicError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_file_text() {
        let f = GeneratedFile::text("src/main.swift", "import SwiftUI");
        assert_eq!(f.relative_path, "src/main.swift");
        assert!(matches!(f.contents, FileContents::Text(ref s) if s == "import SwiftUI"));
    }

    #[test]
    fn generated_file_binary() {
        let f = GeneratedFile::binary("icon.png", vec![0x89, 0x50, 0x4E, 0x47]);
        assert!(matches!(f.contents, FileContents::Binary(ref d) if d.len() == 4));
    }

    #[test]
    fn serde_roundtrip() {
        let f = GeneratedFile::text("test.rs", "fn main() {}");
        let json = serde_json::to_string(&f).unwrap();
        let decoded: GeneratedFile = serde_json::from_str(&json).unwrap();
        assert_eq!(f, decoded);
    }

    #[test]
    fn code_projection_is_object_safe() {
        fn _accepts(_: &dyn CodeProjection) {}
    }

    #[test]
    fn file_contents_equality() {
        let a = FileContents::Text("hello".into());
        let b = FileContents::Text("hello".into());
        assert_eq!(a, b);
    }
}
