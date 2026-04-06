//! Import plugins -- convert legacy file formats into Ideas digits.
//!
//! Each submodule implements the `Importer` trait for a specific file format
//! or family of formats. Register them with `ImporterRegistry`.

pub mod csv_import;
pub mod docx;
pub mod json_import;
pub mod markdown;
#[cfg(not(target_os = "ios"))]
pub mod pdf;
pub mod pptx;
pub mod xlsx;

pub use csv_import::CsvImporter;
pub use docx::DocxImporter;
pub use json_import::JsonImporter;
pub use markdown::MarkdownImporter;
#[cfg(not(target_os = "ios"))]
pub use pdf::PdfImporter;
pub use pptx::PptxImporter;
pub use xlsx::XlsxImporter;
