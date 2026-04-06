//! Export plugins -- convert Ideas digits to legacy file formats.
//!
//! Each submodule implements the `Exporter` trait for a specific output format.
//! Register them with `ExporterRegistry`.

pub mod csv_export;
pub mod docx;
pub mod html;
pub mod jpg;
pub mod json;
pub mod markdown;
pub mod ods;
pub mod odp;
pub mod odt;
#[cfg(not(target_os = "ios"))]
pub mod pdf;
pub mod png;
pub mod pptx;
pub mod svg;
pub mod txt;
pub mod xlsx;

pub use csv_export::CsvExporter;
pub use docx::DocxExporter;
pub use html::HtmlExporter;
pub use jpg::JpgExporter;
pub use json::JsonExporter;
pub use markdown::MarkdownExporter;
pub use ods::OdsExporter;
pub use odp::OdpExporter;
pub use odt::OdtExporter;
#[cfg(not(target_os = "ios"))]
pub use pdf::PdfExporter;
pub use png::PngExporter;
pub use pptx::PptxExporter;
pub use svg::SvgExporter;
pub use txt::TxtExporter;
pub use xlsx::XlsxExporter;
