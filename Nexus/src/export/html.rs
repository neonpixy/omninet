//! HTML exporter -- wraps Magic's HtmlProjection to produce a standalone HTML file.
//!
//! Uses Magic's projection infrastructure (ProjectionContext + HtmlProjection)
//! to generate semantic HTML with CSS, then packages the result as a single
//! combined HTML file with embedded styles.

use uuid::Uuid;

use crate::config::{ExportConfig, ExportFormat};
use crate::error::NexusError;
use crate::output::ExportOutput;
use crate::traits::Exporter;
use ideas::Digit;
use magic::projection::{FileContents, HtmlProjection, CodeProjection, ProjectionContext};

/// Exports Ideas digits as HTML (.html).
///
/// Delegates to Magic's `HtmlProjection` for the actual HTML/CSS generation,
/// then combines the output into a single standalone HTML file with embedded
/// `<style>` tags.
///
/// # Example
///
/// ```ignore
/// let exporter = HtmlExporter;
/// let config = ExportConfig::new(ExportFormat::Html);
/// let output = exporter.export(&digits, None, &config)?;
/// let html = String::from_utf8(output.data).unwrap();
/// assert!(html.contains("<!DOCTYPE html>"));
/// ```
pub struct HtmlExporter;

impl Exporter for HtmlExporter {
    fn id(&self) -> &str {
        "html"
    }

    fn display_name(&self) -> &str {
        "HTML Exporter"
    }

    fn supported_formats(&self) -> &[ExportFormat] {
        &[ExportFormat::Html]
    }

    fn export(
        &self,
        digits: &[Digit],
        root_id: Option<Uuid>,
        config: &ExportConfig,
    ) -> Result<ExportOutput, NexusError> {
        let live: Vec<Digit> = digits
            .iter()
            .filter(|d| !d.is_deleted())
            .cloned()
            .collect();

        if live.is_empty() {
            return Ok(ExportOutput::new(
                empty_html_page().into_bytes(),
                "export.html",
                "text/html",
            ));
        }

        // Resolve the Reign theme -- use config's if provided, else default
        let reign = config
            .theme
            .clone()
            .unwrap_or_default();

        // Build projection context
        let context = ProjectionContext::build(&live, root_id, reign);

        // Run the HTML projection
        let projection = HtmlProjection;
        let generated_files = projection
            .project(&context)
            .map_err(|e| NexusError::ExportFailed(format!("HTML projection failed: {e}")))?;

        if generated_files.is_empty() {
            return Ok(ExportOutput::new(
                empty_html_page().into_bytes(),
                "export.html",
                "text/html",
            ));
        }

        // Find the HTML and CSS files
        let mut html_content = None;
        let mut css_content = None;

        for file in &generated_files {
            if file.relative_path.ends_with(".html") {
                if let FileContents::Text(ref s) = file.contents {
                    html_content = Some(s.clone());
                }
            } else if file.relative_path.ends_with(".css") {
                if let FileContents::Text(ref s) = file.contents {
                    css_content = Some(s.clone());
                }
            }
        }

        // Combine HTML + CSS into a single standalone file
        let output = match (html_content, css_content) {
            (Some(html), Some(css)) => embed_css_in_html(&html, &css),
            (Some(html), None) => html,
            _ => empty_html_page(),
        };

        Ok(ExportOutput::new(
            output.into_bytes(),
            "export.html",
            "text/html",
        ))
    }
}

/// Embed CSS into an HTML document by replacing the `<link>` stylesheet tag
/// with an inline `<style>` block.
fn embed_css_in_html(html: &str, css: &str) -> String {
    // Look for the <link rel="stylesheet" ...> tag and replace it with inline style
    let mut result = String::with_capacity(html.len() + css.len());
    let mut found_link = false;

    for line in html.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("<link") && trimmed.contains("stylesheet") {
            // Replace the link tag with embedded style
            result.push_str(&format!("    <style>\n{css}    </style>\n"));
            found_link = true;
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }

    // If we didn't find a link tag, inject the style before </head>
    if !found_link {
        result = result.replace(
            "</head>",
            &format!("    <style>\n{css}    </style>\n</head>"),
        );
    }

    result
}

/// Generate a minimal empty HTML page.
fn empty_html_page() -> String {
    concat!(
        "<!DOCTYPE html>\n",
        "<html lang=\"en\">\n",
        "<head>\n",
        "    <meta charset=\"UTF-8\" />\n",
        "    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\n",
        "    <title>Export</title>\n",
        "</head>\n",
        "<body>\n",
        "    <main></main>\n",
        "</body>\n",
        "</html>\n",
    )
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ExportConfig;
    use x::Value;

    fn make_digit(dtype: &str) -> Digit {
        Digit::new(dtype.into(), Value::Null, "cpub1test".into()).unwrap()
    }

    #[test]
    fn exports_empty_as_valid_html() {
        let config = ExportConfig::new(ExportFormat::Html);
        let output = HtmlExporter.export(&[], None, &config).unwrap();
        let html = String::from_utf8(output.data).unwrap();
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<html"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn exports_text_digit() {
        let digit = make_digit("text")
            .with_content(Value::from("Hello World"), "test");
        let id = digit.id();
        let config = ExportConfig::new(ExportFormat::Html);
        let output = HtmlExporter.export(&[digit], Some(id), &config).unwrap();
        let html = String::from_utf8(output.data).unwrap();

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("Hello World"));
    }

    #[test]
    fn exports_heading_digit() {
        let digit = make_digit("text.heading")
            .with_property("text".into(), Value::from("My Title"), "test")
            .with_property("level".into(), Value::Int(2), "test");
        let id = digit.id();
        let config = ExportConfig::new(ExportFormat::Html);
        let output = HtmlExporter.export(&[digit], Some(id), &config).unwrap();
        let html = String::from_utf8(output.data).unwrap();

        assert!(html.contains("<h2>My Title</h2>"));
    }

    #[test]
    fn embeds_css_inline() {
        let digit = make_digit("text")
            .with_content(Value::from("Styled"), "test");
        let id = digit.id();
        let config = ExportConfig::new(ExportFormat::Html);
        let output = HtmlExporter.export(&[digit], Some(id), &config).unwrap();
        let html = String::from_utf8(output.data).unwrap();

        // CSS should be embedded as <style>, not linked
        assert!(html.contains("<style>"));
        assert!(!html.contains("<link"));
    }

    #[test]
    fn skips_tombstoned_digits() {
        let live = make_digit("text")
            .with_content(Value::from("visible"), "test");
        let dead = make_digit("text")
            .with_content(Value::from("hidden"), "test")
            .deleted("test");

        let config = ExportConfig::new(ExportFormat::Html);
        let output = HtmlExporter
            .export(&[live, dead], None, &config)
            .unwrap();
        let html = String::from_utf8(output.data).unwrap();

        assert!(html.contains("visible"));
        assert!(!html.contains("hidden"));
    }

    #[test]
    fn metadata() {
        assert_eq!(HtmlExporter.id(), "html");
        assert_eq!(HtmlExporter.display_name(), "HTML Exporter");
        assert_eq!(HtmlExporter.supported_formats(), &[ExportFormat::Html]);
    }

    #[test]
    fn output_metadata() {
        let config = ExportConfig::new(ExportFormat::Html);
        let output = HtmlExporter.export(&[], None, &config).unwrap();
        assert_eq!(output.filename, "export.html");
        assert_eq!(output.mime_type, "text/html");
    }

    #[test]
    fn embed_css_replaces_link_tag() {
        let html = r#"<!DOCTYPE html>
<html>
<head>
    <link rel="stylesheet" href="style.css" />
</head>
<body></body>
</html>"#;
        let css = "body { color: red; }\n";
        let result = embed_css_in_html(html, css);
        assert!(result.contains("<style>"));
        assert!(result.contains("body { color: red; }"));
        assert!(!result.contains("<link"));
    }

    #[test]
    fn embed_css_injects_before_head_close() {
        let html = "<!DOCTYPE html>\n<html>\n<head>\n</head>\n<body></body>\n</html>";
        let css = "p { margin: 0; }\n";
        let result = embed_css_in_html(html, css);
        assert!(result.contains("<style>"));
        assert!(result.contains("p { margin: 0; }"));
    }
}
