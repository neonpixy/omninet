use serde::{Deserialize, Serialize};

use crate::config::ExportFormat;

/// Pre-configured export profiles that group formats by use case.
///
/// When a user wants to "export for print" or "export for web," profiles
/// determine which formats apply based on the digit types present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportProfile {
    /// High-quality output for physical printing (PDF, PNG).
    Print,
    /// Formats compatible with Microsoft Office and LibreOffice (DOCX, XLSX, PPTX, ODF).
    Office,
    /// Web-ready formats (HTML, SVG, PNG, JSON).
    Web,
    /// Source/data formats for developers and data scientists (JSON, CSV, Markdown, TXT).
    Source,
    /// Media export (PNG, JPG, SVG).
    Media,
    /// Structured data export (CSV, JSON, XLSX).
    Data,
    /// Every format that could apply to the content.
    Everything,
}

/// Determine which export formats apply for a profile given the digit types
/// present in the content.
///
/// Inspects digit types to include only relevant formats. For example, the
/// Office profile only includes XLSX if sheet-type digits are present.
pub fn profile_formats(profile: ExportProfile, digits: &[ideas::Digit]) -> Vec<ExportFormat> {
    let has_type = |prefix: &str| digits.iter().any(|d| d.digit_type().starts_with(prefix));

    let has_text = has_type("text.") || has_type("richtext.");
    let has_sheet = has_type("data.sheet") || has_type("data.cell");
    let has_slide = has_type("presentation.");
    let has_media = has_type("media.");
    let has_any = !digits.is_empty();

    match profile {
        ExportProfile::Print => {
            let mut formats = vec![ExportFormat::Pdf];
            if has_media {
                formats.push(ExportFormat::Png);
            }
            formats
        }

        ExportProfile::Office => {
            let mut formats = Vec::new();
            if has_text || has_any {
                formats.push(ExportFormat::Docx);
                formats.push(ExportFormat::Odt);
            }
            if has_sheet {
                formats.push(ExportFormat::Xlsx);
                formats.push(ExportFormat::Ods);
            }
            if has_slide {
                formats.push(ExportFormat::Pptx);
                formats.push(ExportFormat::Odp);
            }
            formats
        }

        ExportProfile::Web => {
            let mut formats = vec![ExportFormat::Html];
            if has_media {
                formats.push(ExportFormat::Png);
                formats.push(ExportFormat::Svg);
            }
            formats.push(ExportFormat::Json);
            formats
        }

        ExportProfile::Source => {
            let mut formats = vec![ExportFormat::Json];
            if has_text || has_any {
                formats.push(ExportFormat::Markdown);
                formats.push(ExportFormat::Txt);
            }
            if has_sheet {
                formats.push(ExportFormat::Csv);
            }
            formats
        }

        ExportProfile::Media => {
            vec![ExportFormat::Png, ExportFormat::Jpg, ExportFormat::Svg]
        }

        ExportProfile::Data => {
            let mut formats = vec![ExportFormat::Json];
            if has_sheet {
                formats.push(ExportFormat::Csv);
                formats.push(ExportFormat::Xlsx);
            }
            formats
        }

        ExportProfile::Everything => {
            let mut formats = vec![
                ExportFormat::Pdf,
                ExportFormat::Html,
                ExportFormat::Json,
                ExportFormat::Markdown,
                ExportFormat::Txt,
            ];
            if has_media {
                formats.push(ExportFormat::Png);
                formats.push(ExportFormat::Jpg);
                formats.push(ExportFormat::Svg);
            }
            if has_text || has_any {
                formats.push(ExportFormat::Docx);
                formats.push(ExportFormat::Odt);
            }
            if has_sheet {
                formats.push(ExportFormat::Xlsx);
                formats.push(ExportFormat::Ods);
                formats.push(ExportFormat::Csv);
            }
            if has_slide {
                formats.push(ExportFormat::Pptx);
                formats.push(ExportFormat::Odp);
            }
            formats
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x::Value;

    fn make_digit(dtype: &str) -> ideas::Digit {
        ideas::Digit::new(dtype.into(), Value::Null, "cpub1test".into()).unwrap()
    }

    #[test]
    fn print_profile_always_includes_pdf() {
        let digits = vec![make_digit("text.paragraph")];
        let formats = profile_formats(ExportProfile::Print, &digits);
        assert!(formats.contains(&ExportFormat::Pdf));
    }

    #[test]
    fn print_profile_includes_png_for_media() {
        let digits = vec![make_digit("media.image")];
        let formats = profile_formats(ExportProfile::Print, &digits);
        assert!(formats.contains(&ExportFormat::Pdf));
        assert!(formats.contains(&ExportFormat::Png));
    }

    #[test]
    fn office_profile_includes_xlsx_for_sheets() {
        let digits = vec![make_digit("data.sheet")];
        let formats = profile_formats(ExportProfile::Office, &digits);
        assert!(formats.contains(&ExportFormat::Xlsx));
        assert!(formats.contains(&ExportFormat::Ods));
    }

    #[test]
    fn office_profile_includes_pptx_for_slides() {
        let digits = vec![make_digit("presentation.slide")];
        let formats = profile_formats(ExportProfile::Office, &digits);
        assert!(formats.contains(&ExportFormat::Pptx));
        assert!(formats.contains(&ExportFormat::Odp));
    }

    #[test]
    fn office_profile_includes_docx_for_text() {
        let digits = vec![make_digit("text.heading")];
        let formats = profile_formats(ExportProfile::Office, &digits);
        assert!(formats.contains(&ExportFormat::Docx));
        assert!(formats.contains(&ExportFormat::Odt));
    }

    #[test]
    fn web_profile_always_includes_html_and_json() {
        let digits = vec![make_digit("text.paragraph")];
        let formats = profile_formats(ExportProfile::Web, &digits);
        assert!(formats.contains(&ExportFormat::Html));
        assert!(formats.contains(&ExportFormat::Json));
    }

    #[test]
    fn source_profile_includes_csv_for_sheets() {
        let digits = vec![make_digit("data.sheet")];
        let formats = profile_formats(ExportProfile::Source, &digits);
        assert!(formats.contains(&ExportFormat::Json));
        assert!(formats.contains(&ExportFormat::Csv));
    }

    #[test]
    fn media_profile_formats() {
        let digits = vec![make_digit("media.image")];
        let formats = profile_formats(ExportProfile::Media, &digits);
        assert!(formats.contains(&ExportFormat::Png));
        assert!(formats.contains(&ExportFormat::Jpg));
        assert!(formats.contains(&ExportFormat::Svg));
    }

    #[test]
    fn data_profile_formats() {
        let digits = vec![make_digit("data.sheet"), make_digit("data.cell")];
        let formats = profile_formats(ExportProfile::Data, &digits);
        assert!(formats.contains(&ExportFormat::Json));
        assert!(formats.contains(&ExportFormat::Csv));
        assert!(formats.contains(&ExportFormat::Xlsx));
    }

    #[test]
    fn everything_profile_with_mixed_content() {
        let digits = vec![
            make_digit("text.heading"),
            make_digit("media.image"),
            make_digit("data.sheet"),
            make_digit("presentation.slide"),
        ];
        let formats = profile_formats(ExportProfile::Everything, &digits);
        assert!(formats.contains(&ExportFormat::Pdf));
        assert!(formats.contains(&ExportFormat::Html));
        assert!(formats.contains(&ExportFormat::Png));
        assert!(formats.contains(&ExportFormat::Docx));
        assert!(formats.contains(&ExportFormat::Xlsx));
        assert!(formats.contains(&ExportFormat::Pptx));
        assert!(formats.contains(&ExportFormat::Csv));
    }

    #[test]
    fn empty_digits_print() {
        let formats = profile_formats(ExportProfile::Print, &[]);
        assert!(formats.contains(&ExportFormat::Pdf));
        assert!(!formats.contains(&ExportFormat::Png));
    }

    #[test]
    fn profile_serde_round_trip() {
        let profiles = vec![
            ExportProfile::Print,
            ExportProfile::Office,
            ExportProfile::Web,
            ExportProfile::Source,
            ExportProfile::Media,
            ExportProfile::Data,
            ExportProfile::Everything,
        ];
        for profile in profiles {
            let json = serde_json::to_string(&profile).unwrap();
            let decoded: ExportProfile = serde_json::from_str(&json).unwrap();
            assert_eq!(profile, decoded);
        }
    }
}
