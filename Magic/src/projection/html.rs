//! Static HTML/CSS/JS code projection — pure semantic HTML with CSS.
//!
//! Generates framework-free semantic HTML + CSS. Formations map to CSS
//! Grid/Flexbox. Accessibility maps to native HTML semantics + ARIA.


use ideas::Digit;
use regalia::FormationKind;

use super::builder::CodeBuilder;
use super::context::ProjectionContext;
use super::name_resolver::NameResolver;
use super::{CodeProjection, GeneratedFile};
use crate::error::MagicError;

/// Static HTML/CSS/JS code projection target.
///
/// Generates pure semantic HTML with embedded CSS. No framework dependencies.
/// Formations map to CSS Grid/Flexbox. Accessibility maps to native HTML
/// semantics (landmark elements, headings) + ARIA attributes where needed.
pub struct HtmlProjection;

impl CodeProjection for HtmlProjection {
    fn name(&self) -> &str {
        "HTML"
    }

    fn file_extension(&self) -> &str {
        "html"
    }

    fn project(
        &self,
        context: &ProjectionContext,
    ) -> Result<Vec<GeneratedFile>, MagicError> {
        let mut files = Vec::new();
        let mut resolver = NameResolver::new();

        for &root_id in &context.root_ids {
            if let Some(digit) = context.digit(root_id) {
                let page_name = resolver.property_name(
                    &digit_display_name(digit),
                );
                let mut html = CodeBuilder::new();
                let mut css = CodeBuilder::new();

                // HTML document
                html.line("<!DOCTYPE html>");
                html.line("<html lang=\"en\">");
                html.braced("<head>", |h| {
                    h.line("<meta charset=\"UTF-8\" />");
                    h.line("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />");
                    h.line(format!("<title>{}</title>", escape_html(&page_name)));
                    h.line(format!("<link rel=\"stylesheet\" href=\"{page_name}.css\" />"));
                });
                html.line("</head>");
                html.line("<body>");
                html.indent(|h| {
                    h.line("<main>");
                    h.indent(|h| {
                        emit_digit_html(h, digit, context, &mut css, &mut resolver);
                    });
                    h.line("</main>");
                });
                html.line("</body>");
                html.line("</html>");

                files.push(GeneratedFile::text(
                    format!("{page_name}.html"),
                    html.output(),
                ));

                // CSS file
                build_css_reset(&mut css);
                files.push(GeneratedFile::text(
                    format!("{page_name}.css"),
                    css.output(),
                ));
            }
        }

        Ok(files)
    }
}

/// Emit HTML for a single digit and its children.
fn emit_digit_html(
    builder: &mut CodeBuilder,
    digit: &Digit,
    context: &ProjectionContext,
    css: &mut CodeBuilder,
    resolver: &mut NameResolver,
) {
    let children = context.children_of(digit.id());

    if !children.is_empty() {
        let formation = context.formation_for(digit.id());
        let css_class = emit_formation_css(css, formation, digit, resolver);
        let aria = aria_attrs(digit);

        builder.line(format!("<div class=\"{css_class}\"{aria}>"));
        builder.indent(|b| {
            for &child_id in children {
                if let Some(child) = context.digit(child_id) {
                    emit_digit_html(b, child, context, css, resolver);
                }
            }
        });
        builder.line("</div>");
    } else {
        emit_leaf_html(builder, digit);
    }
}

/// Emit CSS for a formation container and return the class name.
fn emit_formation_css(
    css: &mut CodeBuilder,
    formation: Option<&FormationKind>,
    digit: &Digit,
    resolver: &mut NameResolver,
) -> String {
    let class_name = resolver.property_name(
        &format!("{}-container", digit.digit_type().replace('.', "-")),
    );

    let direction = match formation {
        Some(FormationKind::Rank { .. }) => "row",
        Some(FormationKind::Tier) => "row",
        _ => "column",
    };

    let spacing = digit.properties.get("spacing")
        .and_then(|v| v.as_double())
        .unwrap_or(8.0);

    css.braced(format!(".{class_name}"), |c| {
        c.line("display: flex;");
        c.line(format!("flex-direction: {direction};"));
        c.line(format!("gap: {spacing}px;"));

        if let Some(padding) = digit.properties.get("padding").and_then(|v| v.as_double()) {
            c.line(format!("padding: {padding}px;"));
        }
        if matches!(formation, Some(FormationKind::Procession { .. })) {
            c.line("flex-wrap: wrap;");
        }
    });
    css.blank();

    class_name
}

/// Emit HTML for a leaf digit.
fn emit_leaf_html(builder: &mut CodeBuilder, digit: &Digit) {
    match digit.digit_type() {
        "text" | "text.paragraph" => {
            let text = digit_text_content(digit);
            builder.line(format!("<p>{}</p>", escape_html(&text)));
        }
        "text.heading" => {
            let text = digit.properties.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("Heading");
            let level = digit.properties.get("level")
                .and_then(|v| v.as_int())
                .unwrap_or(1)
                .min(6);
            builder.line(format!("<h{level}>{}</h{level}>", escape_html(text)));
        }
        "image" => {
            let alt = digit.properties.get("alt")
                .and_then(|v| v.as_str())
                .unwrap_or("Image");
            builder.line(format!("<img src=\"\" alt=\"{}\" loading=\"lazy\" />", escape_html(alt)));
        }
        "code" | "text.code" => {
            let code = digit.properties.get("code")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let language = digit.properties.get("language")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let lang_class = if language.is_empty() {
                String::new()
            } else {
                format!(" class=\"language-{language}\"")
            };
            builder.line(format!("<pre><code{lang_class}>{}</code></pre>", escape_html(code)));
        }
        "divider" => {
            builder.line("<hr />");
        }
        "link" => {
            let text = digit_text_content(digit);
            let url = digit.properties.get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("#");
            builder.line(format!("<a href=\"{}\">{}</a>", escape_html(url), escape_html(&text)));
        }
        "interactive.button" => {
            let label = digit.properties.get("label")
                .and_then(|v| v.as_str())
                .unwrap_or("Button");
            let style = digit.properties.get("style")
                .and_then(|v| v.as_str())
                .unwrap_or("primary");
            builder.line(format!("<button class=\"btn btn-{}\">{}</button>", escape_html(style), escape_html(label)));
        }
        "text.list" => {
            let style = digit.properties.get("style")
                .and_then(|v| v.as_str())
                .unwrap_or("unordered");
            let items: Vec<String> = digit.properties.get("items")
                .and_then(|v| {
                    if let x::Value::Array(arr) = v {
                        Some(arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                    } else {
                        None
                    }
                })
                .unwrap_or_default();

            let tag = if style == "ordered" { "ol" } else { "ul" };
            builder.line(format!("<{tag}>"));
            builder.indent(|b| {
                for item in &items {
                    b.line(format!("<li>{}</li>", escape_html(item)));
                }
            });
            builder.line(format!("</{tag}>"));
        }
        "text.blockquote" => {
            let text = digit.properties.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let attribution = digit.properties.get("attribution")
                .and_then(|v| v.as_str());

            builder.line("<blockquote>");
            builder.indent(|b| {
                b.line(format!("<p>{}</p>", escape_html(text)));
                if let Some(attr) = attribution {
                    b.line(format!("<footer>&mdash; {}</footer>", escape_html(attr)));
                }
            });
            builder.line("</blockquote>");
        }
        "text.callout" => {
            let text = digit.properties.get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let style = digit.properties.get("style")
                .and_then(|v| v.as_str())
                .unwrap_or("info");
            let role = if style == "warning" || style == "error" { " role=\"alert\"" } else { "" };
            builder.line(format!("<div class=\"callout callout-{}\"{role}>", escape_html(style)));
            builder.indent(|b| {
                b.line(format!("<p>{}</p>", escape_html(text)));
            });
            builder.line("</div>");
        }
        _ => {
            let dtype = digit.digit_type();
            builder.line(format!("<!-- TODO: Implement {dtype} -->"));
            builder.line(format!("<div class=\"unknown\">[{dtype}]</div>"));
        }
    }
}

/// Generate ARIA attributes for a digit.
fn aria_attrs(digit: &Digit) -> String {
    if let Some(meta) = digit.accessibility() {
        let mut attrs = Vec::new();
        attrs.push(format!("aria-label=\"{}\"", escape_html(&meta.label)));

        if let Some(ref hint) = meta.hint {
            attrs.push(format!("aria-description=\"{}\"", escape_html(hint)));
        }
        if let Some(ref value) = meta.value {
            attrs.push(format!("aria-valuenow=\"{}\"", escape_html(value)));
        }

        let role_str = match &meta.role {
            ideas::AccessibilityRole::Navigation => "navigation",
            ideas::AccessibilityRole::Form => "form",
            ideas::AccessibilityRole::Alert => "alert",
            ideas::AccessibilityRole::Dialog => "dialog",
            _ => "",
        };
        if !role_str.is_empty() {
            attrs.push(format!("role=\"{role_str}\""));
        }

        if attrs.is_empty() {
            String::new()
        } else {
            format!(" {}", attrs.join(" "))
        }
    } else {
        String::new()
    }
}

/// Build a minimal CSS reset.
fn build_css_reset(css: &mut CodeBuilder) {
    css.blank();
    css.comment("Reset");
    css.braced("*, *::before, *::after", |c| {
        c.line("box-sizing: border-box;");
        c.line("margin: 0;");
        c.line("padding: 0;");
    });
    css.blank();
    css.braced("body", |c| {
        c.line("font-family: system-ui, -apple-system, sans-serif;");
        c.line("line-height: 1.5;");
        c.line("color: #1a1a1a;");
    });
    css.blank();
    css.braced("main", |c| {
        c.line("max-width: 960px;");
        c.line("margin: 0 auto;");
        c.line("padding: 2rem;");
    });
    css.blank();
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn digit_display_name(digit: &Digit) -> String {
    digit.properties.get("title")
        .or(digit.properties.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{}-page", digit.digit_type().replace('.', "-")))
}

fn digit_text_content(digit: &Digit) -> String {
    if let Some(s) = digit.content.as_str() { return s.to_string(); }
    if let Some(s) = digit.properties.get("text").and_then(|v| v.as_str()) { return s.to_string(); }
    if let Some(s) = digit.properties.get("label").and_then(|v| v.as_str()) { return s.to_string(); }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ideas::Digit;
    use regalia::Reign;
    use uuid::Uuid;
    use x::Value;

    fn make_digit(dtype: &str) -> Digit {
        Digit::new(dtype.into(), Value::Null, "cpub1test".into()).unwrap()
    }

    fn simple_context(digits: &[Digit], root_id: Option<Uuid>) -> ProjectionContext {
        ProjectionContext::build(digits, root_id, Reign::default())
    }

    #[test]
    fn projects_single_text() {
        let digit = make_digit("text")
            .with_content(Value::from("Hello"), "test");
        let id = digit.id();
        let ctx = simple_context(&[digit], Some(id));
        let proj = HtmlProjection;
        let files = proj.project(&ctx).unwrap();

        assert_eq!(files.len(), 2); // HTML + CSS
        let html_content = match &files[0].contents {
            super::super::FileContents::Text(s) => s,
            _ => panic!("expected text"),
        };
        assert!(html_content.contains("<!DOCTYPE html>"));
        assert!(html_content.contains("<p>Hello</p>"));
        assert!(html_content.contains("<main>"));
    }

    #[test]
    fn projects_heading() {
        let digit = make_digit("text.heading")
            .with_property("text".into(), Value::from("My Title"), "test")
            .with_property("level".into(), Value::Int(1), "test");
        let id = digit.id();
        let ctx = simple_context(&[digit], Some(id));
        let proj = HtmlProjection;
        let files = proj.project(&ctx).unwrap();

        let html_content = match &files[0].contents {
            super::super::FileContents::Text(s) => s,
            _ => panic!("expected text"),
        };
        assert!(html_content.contains("<h1>My Title</h1>"));
    }

    #[test]
    fn generates_css_file() {
        let digit = make_digit("text")
            .with_content(Value::from("Test"), "test");
        let id = digit.id();
        let ctx = simple_context(&[digit], Some(id));
        let proj = HtmlProjection;
        let files = proj.project(&ctx).unwrap();

        assert!(files[1].relative_path.ends_with(".css"));
        let css_content = match &files[1].contents {
            super::super::FileContents::Text(s) => s,
            _ => panic!("expected text"),
        };
        assert!(css_content.contains("box-sizing: border-box"));
    }

    #[test]
    fn escapes_html_entities() {
        let text = "A < B & C > D \"E\" 'F'";
        let escaped = escape_html(text);
        assert!(escaped.contains("&lt;"));
        assert!(escaped.contains("&amp;"));
        assert!(escaped.contains("&gt;"));
        assert!(escaped.contains("&quot;"));
        assert!(escaped.contains("&#39;"));
    }

    #[test]
    fn name_and_extension() {
        let proj = HtmlProjection;
        assert_eq!(proj.name(), "HTML");
        assert_eq!(proj.file_extension(), "html");
    }

    #[test]
    fn projects_list() {
        let items = Value::Array(vec![Value::from("One"), Value::from("Two")]);
        let digit = make_digit("text.list")
            .with_property("style".into(), Value::from("ordered"), "test")
            .with_property("items".into(), items, "test");
        let id = digit.id();
        let ctx = simple_context(&[digit], Some(id));
        let proj = HtmlProjection;
        let files = proj.project(&ctx).unwrap();

        let html_content = match &files[0].contents {
            super::super::FileContents::Text(s) => s,
            _ => panic!("expected text"),
        };
        assert!(html_content.contains("<ol>"));
        assert!(html_content.contains("<li>One</li>"));
        assert!(html_content.contains("<li>Two</li>"));
        assert!(html_content.contains("</ol>"));
    }
}
