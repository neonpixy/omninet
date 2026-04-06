//! Concrete DigitRenderer implementations for all digit types.
//!
//! Each renderer maps a specific digit type to a platform-agnostic `RenderSpec`
//! with a required `AccessibilitySpec`. Platform layers (Divinity) interpret
//! these specs into native views.
//!
//! ## Renderer Categories
//!
//! - **Core** — the original 9 digit types: text, image, code, container, table,
//!   embed, document, divider, link
//! - **Rich Text** — `text.heading`, `text.paragraph`, `text.list`, `text.blockquote`,
//!   `text.callout`
//! - **Presentation** — `presentation.slide`
//! - **Interactive** — `interactive.button`, `interactive.accordion`,
//!   `interactive.tab-group`
//! - **Data** — `data.sheet`
//! - **Form** — `form.container`
//! - **Commerce** — `commerce.product`

use ideas::Digit;
use x::Value;

use super::accessibility::{AccessibilityRole, AccessibilitySpec, AccessibilityTrait, LiveRegion};
use super::render::{DigitRenderer, RenderContext, RenderMode, RenderSpec};
use super::registry::RendererRegistry;

// ---------------------------------------------------------------------------
// Helper: extract text from digit properties or content
// ---------------------------------------------------------------------------

fn digit_text(digit: &Digit) -> String {
    // Try content first
    if let Some(s) = digit.content.as_str() {
        return s.to_string();
    }
    // Try "text" property
    if let Some(Value::String(s)) = digit.properties.get("text") {
        return s.clone();
    }
    // Try "label" property
    if let Some(Value::String(s)) = digit.properties.get("label") {
        return s.clone();
    }
    // Try "title" property
    if let Some(Value::String(s)) = digit.properties.get("title") {
        return s.clone();
    }
    String::new()
}

fn prop_str(digit: &Digit, key: &str) -> Option<String> {
    digit.properties.get(key).and_then(|v| {
        if let Value::String(s) = v {
            Some(s.clone())
        } else {
            None
        }
    })
}

fn prop_int(digit: &Digit, key: &str) -> Option<i64> {
    digit.properties.get(key).and_then(|v| v.as_int())
}

fn prop_bool(digit: &Digit, key: &str) -> Option<bool> {
    digit.properties.get(key).and_then(|v| {
        if let Value::Bool(b) = v {
            Some(*b)
        } else {
            None
        }
    })
}

fn prop_double(digit: &Digit, key: &str) -> Option<f64> {
    digit.properties.get(key).and_then(|v| v.as_double())
}

// ---------------------------------------------------------------------------
// Core Renderers
// ---------------------------------------------------------------------------

/// Renders `text` digits — paragraphs with inline formatting.
pub struct TextRenderer;

impl DigitRenderer for TextRenderer {
    fn digit_type(&self) -> &str { "text" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let text = digit_text(digit);
        let a11y = AccessibilitySpec::from_digit(
            digit,
            AccessibilityRole::Custom("text".into()),
            if text.is_empty() { "Empty text".to_string() } else { text.clone() },
        ).with_trait(AccessibilityTrait::StaticText);

        let font_size = prop_double(digit, "font_size").unwrap_or(16.0) * context.text_scale;
        let estimated_height = (text.len() as f64 / (context.available_width / font_size).max(1.0)).ceil() * font_size * 1.5;

        let mut spec = RenderSpec::new(digit.id(), "text", mode)
            .with_size(context.available_width, estimated_height.max(font_size * 1.5))
            .with_accessibility(a11y)
            .with_property("text", serde_json::json!(text))
            .with_property("font_size", serde_json::json!(font_size));

        if let Some(font_family) = prop_str(digit, "font_family") {
            spec = spec.with_property("font_family", serde_json::json!(font_family));
        }
        if let Some(font_weight) = prop_str(digit, "font_weight") {
            spec = spec.with_property("font_weight", serde_json::json!(font_weight));
        }
        if let Some(alignment) = prop_str(digit, "alignment") {
            spec = spec.with_property("alignment", serde_json::json!(alignment));
        }

        if mode == RenderMode::Editing {
            spec = spec.with_property("editable", serde_json::json!(true));
        }

        spec
    }

    fn estimated_size(&self, digit: &Digit, context: &RenderContext) -> (f64, f64) {
        let text = digit_text(digit);
        let font_size = prop_double(digit, "font_size").unwrap_or(16.0) * context.text_scale;
        let height = (text.len() as f64 / (context.available_width / font_size).max(1.0)).ceil() * font_size * 1.5;
        (context.available_width, height.max(font_size * 1.5))
    }
}

/// Renders `image` digits — images with aspect ratio and fit mode.
pub struct ImageRenderer;

impl DigitRenderer for ImageRenderer {
    fn digit_type(&self) -> &str { "image" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let alt = prop_str(digit, "alt").unwrap_or_else(|| "Image".to_string());
        let a11y = AccessibilitySpec::from_digit(digit, AccessibilityRole::Image, &alt);

        let width = prop_double(digit, "width").unwrap_or(context.available_width);
        let height = prop_double(digit, "height").unwrap_or(width * 0.75);
        let fit_mode = prop_str(digit, "fit_mode").unwrap_or_else(|| "fit".to_string());

        let (thumb_w, thumb_h) = if mode == RenderMode::Thumbnail {
            (120.0, 90.0)
        } else {
            (width, height)
        };

        let mut spec = RenderSpec::new(digit.id(), "image", mode)
            .with_size(thumb_w, thumb_h)
            .with_accessibility(a11y)
            .with_property("fit_mode", serde_json::json!(fit_mode));

        if let Some(hash) = prop_str(digit, "hash") {
            spec = spec.with_property("hash", serde_json::json!(hash));
        }
        if let Some(mime) = prop_str(digit, "mime") {
            spec = spec.with_property("mime", serde_json::json!(mime));
        }
        if let Some(blurhash) = prop_str(digit, "blurhash") {
            spec = spec.with_property("blurhash", serde_json::json!(blurhash));
        }

        if mode == RenderMode::Editing {
            spec = spec.with_property("show_handles", serde_json::json!(true));
        }

        spec
    }

    fn estimated_size(&self, digit: &Digit, context: &RenderContext) -> (f64, f64) {
        let width = prop_double(digit, "width").unwrap_or(context.available_width);
        let height = prop_double(digit, "height").unwrap_or(width * 0.75);
        (width, height)
    }
}

/// Renders `code` digits — syntax-highlighted code blocks.
pub struct CodeRenderer;

impl DigitRenderer for CodeRenderer {
    fn digit_type(&self) -> &str { "code" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let code = prop_str(digit, "code").unwrap_or_else(|| digit_text(digit));
        let language = prop_str(digit, "language").unwrap_or_default();
        let label = if language.is_empty() {
            "Code block".to_string()
        } else {
            format!("{language} code block")
        };

        let a11y = AccessibilitySpec::from_digit(
            digit, AccessibilityRole::Custom("code".into()), label,
        ).with_trait(AccessibilityTrait::StaticText);

        let line_count = code.lines().count().max(1);
        let line_height = 20.0 * context.text_scale;
        let estimated_height = line_count as f64 * line_height + 24.0; // padding

        let mut spec = RenderSpec::new(digit.id(), "code", mode)
            .with_size(context.available_width, estimated_height)
            .with_accessibility(a11y)
            .with_property("code", serde_json::json!(code))
            .with_property("language", serde_json::json!(language))
            .with_property("monospace", serde_json::json!(true));

        if mode == RenderMode::Editing {
            spec = spec.with_property("editable", serde_json::json!(true))
                .with_property("line_numbers", serde_json::json!(true));
        }

        spec
    }

    fn estimated_size(&self, digit: &Digit, context: &RenderContext) -> (f64, f64) {
        let code = prop_str(digit, "code").unwrap_or_else(|| digit_text(digit));
        let line_count = code.lines().count().max(1);
        let line_height = 20.0 * context.text_scale;
        (context.available_width, line_count as f64 * line_height + 24.0)
    }
}

/// Renders `container` digits — layout containers holding children.
pub struct ContainerRenderer;

impl DigitRenderer for ContainerRenderer {
    fn digit_type(&self) -> &str { "container" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let label = prop_str(digit, "title").unwrap_or_else(|| "Container".to_string());
        let a11y = AccessibilitySpec::from_digit(
            digit, AccessibilityRole::Custom("group".into()), label,
        );

        let formation = prop_str(digit, "formation").unwrap_or_else(|| "column".to_string());
        let child_count = digit.children.as_ref().map_or(0, |c| c.len());

        let mut spec = RenderSpec::new(digit.id(), "container", mode)
            .with_size(context.available_width, context.available_height)
            .with_accessibility(a11y)
            .with_property("formation", serde_json::json!(formation))
            .with_property("child_count", serde_json::json!(child_count));

        if let Some(padding) = prop_double(digit, "padding") {
            spec = spec.with_property("padding", serde_json::json!(padding));
        }
        if let Some(spacing) = prop_double(digit, "spacing") {
            spec = spec.with_property("spacing", serde_json::json!(spacing));
        }
        if let Some(bg) = prop_str(digit, "background") {
            spec = spec.with_property("background", serde_json::json!(bg));
        }

        if mode == RenderMode::Editing {
            spec = spec.with_property("show_bounds", serde_json::json!(true));
        }

        spec
    }

    fn estimated_size(&self, _digit: &Digit, context: &RenderContext) -> (f64, f64) {
        (context.available_width, context.available_height)
    }
}

/// Renders `table` digits — rows, columns, header row, cells.
pub struct TableRenderer;

impl DigitRenderer for TableRenderer {
    fn digit_type(&self) -> &str { "table" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let label = prop_str(digit, "title").unwrap_or_else(|| "Table".to_string());
        let a11y = AccessibilitySpec::from_digit(digit, AccessibilityRole::Table, label);

        let rows = prop_int(digit, "rows").unwrap_or(0);
        let columns = prop_int(digit, "columns").unwrap_or(0);
        let row_height = 40.0 * context.text_scale;
        let estimated_height = (rows + 1) as f64 * row_height; // +1 for header

        let mut spec = RenderSpec::new(digit.id(), "table", mode)
            .with_size(context.available_width, estimated_height)
            .with_accessibility(a11y)
            .with_property("rows", serde_json::json!(rows))
            .with_property("columns", serde_json::json!(columns));

        if let Some(has_header) = prop_bool(digit, "has_header") {
            spec = spec.with_property("has_header", serde_json::json!(has_header));
        }

        if mode == RenderMode::Editing {
            spec = spec.with_property("editable_cells", serde_json::json!(true));
        }

        spec
    }

    fn estimated_size(&self, digit: &Digit, context: &RenderContext) -> (f64, f64) {
        let rows = prop_int(digit, "rows").unwrap_or(3);
        let row_height = 40.0 * context.text_scale;
        (context.available_width, (rows + 1) as f64 * row_height)
    }
}

/// Renders `embed` digits — external content with fallback.
pub struct EmbedRenderer;

impl DigitRenderer for EmbedRenderer {
    fn digit_type(&self) -> &str { "embed" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let label = prop_str(digit, "title")
            .or_else(|| prop_str(digit, "url"))
            .unwrap_or_else(|| "Embedded content".to_string());
        let a11y = AccessibilitySpec::from_digit(
            digit, AccessibilityRole::Custom("embed".into()), label,
        );

        let mut spec = RenderSpec::new(digit.id(), "embed", mode)
            .with_size(context.available_width, 300.0)
            .with_accessibility(a11y);

        if let Some(url) = prop_str(digit, "url") {
            spec = spec.with_property("url", serde_json::json!(url));
        }
        if let Some(embed_type) = prop_str(digit, "embed_type") {
            spec = spec.with_property("embed_type", serde_json::json!(embed_type));
        }
        if let Some(fallback) = prop_str(digit, "fallback_text") {
            spec = spec.with_property("fallback_text", serde_json::json!(fallback));
        }

        spec
    }

    fn estimated_size(&self, _digit: &Digit, context: &RenderContext) -> (f64, f64) {
        (context.available_width, 300.0)
    }
}

/// Renders `document` digits — nested .idea references.
pub struct DocumentRenderer;

impl DigitRenderer for DocumentRenderer {
    fn digit_type(&self) -> &str { "document" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let title = prop_str(digit, "title").unwrap_or_else(|| "Document".to_string());
        let a11y = AccessibilitySpec::from_digit(
            digit, AccessibilityRole::Custom("document".into()), &title,
        );

        let mut spec = RenderSpec::new(digit.id(), "document", mode)
            .with_size(context.available_width, 200.0)
            .with_accessibility(a11y)
            .with_property("title", serde_json::json!(title));

        if let Some(idea_ref) = prop_str(digit, "idea_ref") {
            spec = spec.with_property("idea_ref", serde_json::json!(idea_ref));
        }

        if mode == RenderMode::Thumbnail {
            spec = spec.with_size(120.0, 90.0);
        }

        spec
    }

    fn estimated_size(&self, _digit: &Digit, context: &RenderContext) -> (f64, f64) {
        (context.available_width, 200.0)
    }
}

/// Renders `divider` digits — horizontal rules.
pub struct DividerRenderer;

impl DigitRenderer for DividerRenderer {
    fn digit_type(&self) -> &str { "divider" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        // Dividers are decorative — hidden from accessibility
        let a11y = AccessibilitySpec::decorative();

        let thickness = prop_double(digit, "thickness").unwrap_or(1.0);
        let spacing = prop_double(digit, "spacing").unwrap_or(16.0);

        RenderSpec::new(digit.id(), "divider", mode)
            .with_size(context.available_width, thickness + spacing * 2.0)
            .with_accessibility(a11y)
            .with_property("thickness", serde_json::json!(thickness))
            .with_property("spacing", serde_json::json!(spacing))
    }

    fn estimated_size(&self, digit: &Digit, _context: &RenderContext) -> (f64, f64) {
        let thickness = prop_double(digit, "thickness").unwrap_or(1.0);
        let spacing = prop_double(digit, "spacing").unwrap_or(16.0);
        (f64::INFINITY, thickness + spacing * 2.0)
    }
}

/// Renders `link` digits — navigation targets with display text.
pub struct LinkRenderer;

impl DigitRenderer for LinkRenderer {
    fn digit_type(&self) -> &str { "link" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let display_text = digit_text(digit);
        let url = prop_str(digit, "url").unwrap_or_default();
        let label = if display_text.is_empty() {
            format!("Link to {url}")
        } else {
            display_text.clone()
        };

        let a11y = AccessibilitySpec::from_digit(digit, AccessibilityRole::Link, &label)
            .with_trait(AccessibilityTrait::Interactive);

        let font_size = prop_double(digit, "font_size").unwrap_or(16.0) * context.text_scale;

        let mut spec = RenderSpec::new(digit.id(), "link", mode)
            .with_size(context.available_width, font_size * 1.5)
            .with_accessibility(a11y)
            .with_property("display_text", serde_json::json!(label))
            .with_property("url", serde_json::json!(url));

        if mode == RenderMode::Editing {
            spec = spec.with_property("editable", serde_json::json!(true));
        }

        spec
    }

    fn estimated_size(&self, digit: &Digit, context: &RenderContext) -> (f64, f64) {
        let font_size = prop_double(digit, "font_size").unwrap_or(16.0) * context.text_scale;
        (context.available_width, font_size * 1.5)
    }
}

// ---------------------------------------------------------------------------
// Rich Text Renderers (Phase 1B digit types)
// ---------------------------------------------------------------------------

/// Renders `text.heading` digits.
pub struct HeadingRenderer;

impl DigitRenderer for HeadingRenderer {
    fn digit_type(&self) -> &str { "text.heading" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let text = prop_str(digit, "text").unwrap_or_else(|| digit_text(digit));
        let level = prop_int(digit, "level").unwrap_or(1) as u8;
        let label = if text.is_empty() {
            format!("Heading level {level}")
        } else {
            text.clone()
        };

        let a11y = AccessibilitySpec::from_digit(digit, AccessibilityRole::Heading, &label)
            .with_trait(AccessibilityTrait::Header);

        // Heading sizes: H1=32, H2=28, H3=24, H4=20, H5=18, H6=16
        let base_size = match level {
            1 => 32.0,
            2 => 28.0,
            3 => 24.0,
            4 => 20.0,
            5 => 18.0,
            _ => 16.0,
        };
        let font_size = base_size * context.text_scale;

        let mut spec = RenderSpec::new(digit.id(), "text.heading", mode)
            .with_size(context.available_width, font_size * 1.6)
            .with_accessibility(a11y)
            .with_property("text", serde_json::json!(text))
            .with_property("level", serde_json::json!(level))
            .with_property("font_size", serde_json::json!(font_size));

        if mode == RenderMode::Editing {
            spec = spec.with_property("editable", serde_json::json!(true));
        }

        spec
    }

    fn estimated_size(&self, digit: &Digit, context: &RenderContext) -> (f64, f64) {
        let level = prop_int(digit, "level").unwrap_or(1) as u8;
        let base_size = match level { 1 => 32.0, 2 => 28.0, 3 => 24.0, 4 => 20.0, 5 => 18.0, _ => 16.0 };
        (context.available_width, base_size * context.text_scale * 1.6)
    }
}

/// Renders `text.paragraph` digits.
pub struct ParagraphRenderer;

impl DigitRenderer for ParagraphRenderer {
    fn digit_type(&self) -> &str { "text.paragraph" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let text = prop_str(digit, "text").unwrap_or_else(|| digit_text(digit));
        let a11y = AccessibilitySpec::from_digit(
            digit,
            AccessibilityRole::Custom("paragraph".into()),
            if text.is_empty() { "Empty paragraph".to_string() } else { text.clone() },
        ).with_trait(AccessibilityTrait::StaticText);

        let font_size = 16.0 * context.text_scale;
        let estimated_height = (text.len() as f64 / (context.available_width / font_size).max(1.0)).ceil() * font_size * 1.5;

        let mut spec = RenderSpec::new(digit.id(), "text.paragraph", mode)
            .with_size(context.available_width, estimated_height.max(font_size * 1.5))
            .with_accessibility(a11y)
            .with_property("text", serde_json::json!(text))
            .with_property("font_size", serde_json::json!(font_size));

        if mode == RenderMode::Editing {
            spec = spec.with_property("editable", serde_json::json!(true));
        }

        spec
    }
}

/// Renders `text.list` digits.
pub struct ListRenderer;

impl DigitRenderer for ListRenderer {
    fn digit_type(&self) -> &str { "text.list" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let style = prop_str(digit, "style").unwrap_or_else(|| "unordered".to_string());
        let items: Vec<String> = digit.properties.get("items")
            .and_then(|v| {
                if let Value::Array(arr) = v {
                    Some(arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let label = format!("{} list, {} items", style, items.len());
        let a11y = AccessibilitySpec::from_digit(digit, AccessibilityRole::List, &label);

        let item_height = 24.0 * context.text_scale;
        let estimated_height = items.len().max(1) as f64 * item_height;

        

        RenderSpec::new(digit.id(), "text.list", mode)
            .with_size(context.available_width, estimated_height)
            .with_accessibility(a11y)
            .with_property("style", serde_json::json!(style))
            .with_property("items", serde_json::json!(items))
            .with_property("item_count", serde_json::json!(items.len()))
    }
}

/// Renders `text.blockquote` digits.
pub struct BlockquoteRenderer;

impl DigitRenderer for BlockquoteRenderer {
    fn digit_type(&self) -> &str { "text.blockquote" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let text = prop_str(digit, "text").unwrap_or_default();
        let attribution = prop_str(digit, "attribution");
        let label = if let Some(ref attr) = attribution {
            format!("Quote by {attr}: {text}")
        } else {
            format!("Quote: {text}")
        };

        let a11y = AccessibilitySpec::from_digit(
            digit, AccessibilityRole::Custom("blockquote".into()), label,
        ).with_trait(AccessibilityTrait::StaticText);

        let font_size = 16.0 * context.text_scale;
        let estimated_height = (text.len() as f64 / (context.available_width / font_size).max(1.0)).ceil() * font_size * 1.5 + 16.0;

        let mut spec = RenderSpec::new(digit.id(), "text.blockquote", mode)
            .with_size(context.available_width, estimated_height)
            .with_accessibility(a11y)
            .with_property("text", serde_json::json!(text));

        if let Some(attr) = attribution {
            spec = spec.with_property("attribution", serde_json::json!(attr));
        }

        spec
    }
}

/// Renders `text.callout` digits.
pub struct CalloutRenderer;

impl DigitRenderer for CalloutRenderer {
    fn digit_type(&self) -> &str { "text.callout" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let text = prop_str(digit, "text").unwrap_or_default();
        let style = prop_str(digit, "style").unwrap_or_else(|| "info".to_string());

        let role = if style == "warning" || style == "error" {
            AccessibilityRole::Alert
        } else {
            AccessibilityRole::Custom("callout".into())
        };
        let live = if style == "error" { LiveRegion::Assertive } else { LiveRegion::Off };

        let a11y = AccessibilitySpec::from_digit(digit, role, format!("{style}: {text}"))
            .with_live_region(live);

        let font_size = 16.0 * context.text_scale;
        let estimated_height = (text.len() as f64 / (context.available_width / font_size).max(1.0)).ceil() * font_size * 1.5 + 32.0;

        RenderSpec::new(digit.id(), "text.callout", mode)
            .with_size(context.available_width, estimated_height)
            .with_accessibility(a11y)
            .with_property("text", serde_json::json!(text))
            .with_property("style", serde_json::json!(style))
    }
}

// ---------------------------------------------------------------------------
// Presentation Renderers
// ---------------------------------------------------------------------------

/// Renders `presentation.slide` digits.
pub struct SlideRenderer;

impl DigitRenderer for SlideRenderer {
    fn digit_type(&self) -> &str { "presentation.slide" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, _context: &RenderContext) -> RenderSpec {
        let title = prop_str(digit, "title").unwrap_or_else(|| "Untitled slide".to_string());
        let order = prop_int(digit, "order").unwrap_or(0);
        let layout = prop_str(digit, "layout").unwrap_or_else(|| "content".to_string());

        let label = format!("Slide {}: {}", order + 1, title);
        let a11y = AccessibilitySpec::from_digit(digit, AccessibilityRole::Custom("slide".into()), &label);

        // Standard slide dimensions (16:9)
        let (width, height) = match mode {
            RenderMode::Thumbnail => (192.0, 108.0),
            _ => (960.0, 540.0),
        };

        let mut spec = RenderSpec::new(digit.id(), "presentation.slide", mode)
            .with_size(width, height)
            .with_accessibility(a11y)
            .with_property("title", serde_json::json!(title))
            .with_property("order", serde_json::json!(order))
            .with_property("layout", serde_json::json!(layout));

        if let Some(notes) = prop_str(digit, "speaker_notes") {
            spec = spec.with_property("speaker_notes", serde_json::json!(notes));
        }
        if let Some(transition) = prop_str(digit, "transition") {
            spec = spec.with_property("transition", serde_json::json!(transition));
        }

        if mode == RenderMode::Editing {
            spec = spec.with_property("show_notes", serde_json::json!(true));
        }

        spec
    }

    fn estimated_size(&self, _digit: &Digit, _context: &RenderContext) -> (f64, f64) {
        (960.0, 540.0)
    }
}

// ---------------------------------------------------------------------------
// Interactive Renderers
// ---------------------------------------------------------------------------

/// Renders `interactive.button` digits.
pub struct ButtonRenderer;

impl DigitRenderer for ButtonRenderer {
    fn digit_type(&self) -> &str { "interactive.button" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let label = prop_str(digit, "label").unwrap_or_else(|| "Button".to_string());
        let style = prop_str(digit, "style").unwrap_or_else(|| "primary".to_string());

        let a11y = AccessibilitySpec::from_digit(digit, AccessibilityRole::Button, &label)
            .with_trait(AccessibilityTrait::Interactive)
            .with_hint("Double-tap to activate");

        let font_size = 16.0 * context.text_scale;
        let padding = 16.0;
        let width = (label.len() as f64 * font_size * 0.6 + padding * 2.0).min(context.available_width);
        let height = font_size + padding * 2.0;

        let mut spec = RenderSpec::new(digit.id(), "interactive.button", mode)
            .with_size(width, height)
            .with_accessibility(a11y)
            .with_property("label", serde_json::json!(label))
            .with_property("style", serde_json::json!(style));

        if let Some(action_ref) = prop_str(digit, "action_ref") {
            spec = spec.with_property("action_ref", serde_json::json!(action_ref));
        }

        spec
    }

    fn estimated_size(&self, digit: &Digit, context: &RenderContext) -> (f64, f64) {
        let label = prop_str(digit, "label").unwrap_or_else(|| "Button".to_string());
        let font_size = 16.0 * context.text_scale;
        let padding = 16.0;
        (label.len() as f64 * font_size * 0.6 + padding * 2.0, font_size + padding * 2.0)
    }
}

/// Renders `interactive.accordion` digits.
pub struct AccordionRenderer;

impl DigitRenderer for AccordionRenderer {
    fn digit_type(&self) -> &str { "interactive.accordion" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let title = prop_str(digit, "title").unwrap_or_else(|| "Section".to_string());
        let expanded = prop_bool(digit, "expanded").unwrap_or(false);

        let a11y = AccessibilitySpec::from_digit(digit, AccessibilityRole::Button, &title)
            .with_trait(AccessibilityTrait::Interactive)
            .with_value(if expanded { "expanded" } else { "collapsed" })
            .with_hint("Double-tap to toggle");

        let header_height = 48.0 * context.text_scale;
        let content_height = if expanded { 200.0 } else { 0.0 };

        RenderSpec::new(digit.id(), "interactive.accordion", mode)
            .with_size(context.available_width, header_height + content_height)
            .with_accessibility(a11y)
            .with_property("title", serde_json::json!(title))
            .with_property("expanded", serde_json::json!(expanded))
    }
}

/// Renders `interactive.tab-group` digits.
pub struct TabGroupRenderer;

impl DigitRenderer for TabGroupRenderer {
    fn digit_type(&self) -> &str { "interactive.tab-group" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let tabs: Vec<String> = digit.properties.get("tabs")
            .and_then(|v| {
                if let Value::Array(arr) = v {
                    Some(arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let active_index = prop_int(digit, "active_index").unwrap_or(0) as usize;
        let active_tab = tabs.get(active_index).cloned().unwrap_or_else(|| "Tab".to_string());

        let label = format!("Tab group, {} tabs, {} selected", tabs.len(), active_tab);
        let a11y = AccessibilitySpec::from_digit(
            digit, AccessibilityRole::Custom("tablist".into()), label,
        );

        let tab_bar_height = 44.0 * context.text_scale;
        let content_height = 300.0;

        RenderSpec::new(digit.id(), "interactive.tab-group", mode)
            .with_size(context.available_width, tab_bar_height + content_height)
            .with_accessibility(a11y)
            .with_property("tabs", serde_json::json!(tabs))
            .with_property("active_index", serde_json::json!(active_index))
    }
}

// ---------------------------------------------------------------------------
// Data Renderers
// ---------------------------------------------------------------------------

/// Renders `data.sheet` digits — spreadsheet grids.
pub struct SheetRenderer;

impl DigitRenderer for SheetRenderer {
    fn digit_type(&self) -> &str { "data.sheet" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let name = prop_str(digit, "name").unwrap_or_else(|| "Sheet".to_string());
        let default_view = prop_str(digit, "default_view").unwrap_or_else(|| "grid".to_string());

        let a11y = AccessibilitySpec::from_digit(digit, AccessibilityRole::Table, &name);

        let mut spec = RenderSpec::new(digit.id(), "data.sheet", mode)
            .with_size(context.available_width, 400.0)
            .with_accessibility(a11y)
            .with_property("name", serde_json::json!(name))
            .with_property("default_view", serde_json::json!(default_view));

        if mode == RenderMode::Editing {
            spec = spec.with_property("editable_cells", serde_json::json!(true));
        }

        if mode == RenderMode::Thumbnail {
            spec = spec.with_size(192.0, 108.0);
        }

        spec
    }

    fn estimated_size(&self, _digit: &Digit, context: &RenderContext) -> (f64, f64) {
        (context.available_width, 400.0)
    }
}

// ---------------------------------------------------------------------------
// Form Renderers
// ---------------------------------------------------------------------------

/// Renders `form.container` digits — form layout.
pub struct FormRenderer;

impl DigitRenderer for FormRenderer {
    fn digit_type(&self) -> &str { "form.container" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let name = prop_str(digit, "name").unwrap_or_else(|| "Form".to_string());
        let a11y = AccessibilitySpec::from_digit(digit, AccessibilityRole::Form, &name);

        let child_count = digit.children.as_ref().map_or(0, |c| c.len());

        let mut spec = RenderSpec::new(digit.id(), "form.container", mode)
            .with_size(context.available_width, 300.0)
            .with_accessibility(a11y)
            .with_property("name", serde_json::json!(name))
            .with_property("child_count", serde_json::json!(child_count));

        if let Some(handler) = prop_str(digit, "submit_handler_ref") {
            spec = spec.with_property("submit_handler_ref", serde_json::json!(handler));
        }

        if mode == RenderMode::Editing {
            spec = spec.with_property("show_bounds", serde_json::json!(true));
        }

        spec
    }
}

// ---------------------------------------------------------------------------
// Commerce Renderers
// ---------------------------------------------------------------------------

/// Renders `commerce.product` digits — product cards.
pub struct ProductRenderer;

impl DigitRenderer for ProductRenderer {
    fn digit_type(&self) -> &str { "commerce.product" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let title = prop_str(digit, "title").unwrap_or_else(|| "Product".to_string());
        let price_cents = prop_int(digit, "price_cents").unwrap_or(0);
        let price_display = format!("{}.{:02}", price_cents / 100, (price_cents % 100).abs());

        let label = format!("{title}, {price_display} Cool");
        let a11y = AccessibilitySpec::from_digit(
            digit, AccessibilityRole::Custom("product".into()), &label,
        ).with_custom_action("Add to cart", "add_to_cart");

        let card_height = if mode == RenderMode::Thumbnail { 160.0 } else { 320.0 };
        let card_width = if mode == RenderMode::Thumbnail { 120.0 } else { context.available_width.min(320.0) };

        let mut spec = RenderSpec::new(digit.id(), "commerce.product", mode)
            .with_size(card_width, card_height)
            .with_accessibility(a11y)
            .with_property("title", serde_json::json!(title))
            .with_property("price_cents", serde_json::json!(price_cents))
            .with_property("price_display", serde_json::json!(price_display));

        if let Some(desc) = prop_str(digit, "description") {
            spec = spec.with_property("description", serde_json::json!(desc));
        }
        if let Some(seller) = prop_str(digit, "seller_pubkey") {
            spec = spec.with_property("seller_pubkey", serde_json::json!(seller));
        }

        spec
    }

    fn estimated_size(&self, _digit: &Digit, context: &RenderContext) -> (f64, f64) {
        (context.available_width.min(320.0), 320.0)
    }
}

// ---------------------------------------------------------------------------
// Academic/Reference Renderers
// ---------------------------------------------------------------------------

/// Renders `text.footnote` digits.
pub struct FootnoteRenderer;

impl DigitRenderer for FootnoteRenderer {
    fn digit_type(&self) -> &str { "text.footnote" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let marker = prop_str(digit, "marker").unwrap_or_else(|| "?".to_string());
        let text = prop_str(digit, "text").unwrap_or_default();

        let label = format!("Footnote {marker}: {text}");
        let a11y = AccessibilitySpec::from_digit(
            digit, AccessibilityRole::Custom("note".into()), label,
        ).with_trait(AccessibilityTrait::StaticText);

        let font_size = 14.0 * context.text_scale;

        RenderSpec::new(digit.id(), "text.footnote", mode)
            .with_size(context.available_width, font_size * 1.5)
            .with_accessibility(a11y)
            .with_property("marker", serde_json::json!(marker))
            .with_property("text", serde_json::json!(text))
            .with_property("font_size", serde_json::json!(font_size))
    }
}

/// Renders `text.citation` digits.
pub struct CitationRenderer;

impl DigitRenderer for CitationRenderer {
    fn digit_type(&self) -> &str { "text.citation" }

    fn supported_modes(&self) -> Vec<RenderMode> {
        vec![RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print]
    }

    fn render(&self, digit: &Digit, mode: RenderMode, context: &RenderContext) -> RenderSpec {
        let source = prop_str(digit, "source").unwrap_or_else(|| "Unknown source".to_string());
        let author = prop_str(digit, "author");
        let url = prop_str(digit, "url");

        let label = if let Some(ref auth) = author {
            format!("Citation: {source} by {auth}")
        } else {
            format!("Citation: {source}")
        };

        let a11y = AccessibilitySpec::from_digit(
            digit, AccessibilityRole::Custom("reference".into()), label,
        ).with_trait(AccessibilityTrait::StaticText);

        let font_size = 14.0 * context.text_scale;

        let mut spec = RenderSpec::new(digit.id(), "text.citation", mode)
            .with_size(context.available_width, font_size * 1.5)
            .with_accessibility(a11y)
            .with_property("source", serde_json::json!(source));

        if let Some(auth) = author {
            spec = spec.with_property("author", serde_json::json!(auth));
        }
        if let Some(u) = url {
            spec = spec.with_property("url", serde_json::json!(u));
        }

        spec
    }
}

// ---------------------------------------------------------------------------
// Registry helpers
// ---------------------------------------------------------------------------

/// Register all built-in renderers with the given registry.
///
/// Call this once at startup to populate the renderer registry with all
/// core and Phase 1B digit type renderers.
pub fn register_all_renderers(registry: &mut RendererRegistry) {
    // Core renderers
    registry.register(Box::new(TextRenderer));
    registry.register(Box::new(ImageRenderer));
    registry.register(Box::new(CodeRenderer));
    registry.register(Box::new(ContainerRenderer));
    registry.register(Box::new(TableRenderer));
    registry.register(Box::new(EmbedRenderer));
    registry.register(Box::new(DocumentRenderer));
    registry.register(Box::new(DividerRenderer));
    registry.register(Box::new(LinkRenderer));

    // Rich text renderers
    registry.register(Box::new(HeadingRenderer));
    registry.register(Box::new(ParagraphRenderer));
    registry.register(Box::new(ListRenderer));
    registry.register(Box::new(BlockquoteRenderer));
    registry.register(Box::new(CalloutRenderer));

    // Presentation renderers
    registry.register(Box::new(SlideRenderer));

    // Interactive renderers
    registry.register(Box::new(ButtonRenderer));
    registry.register(Box::new(AccordionRenderer));
    registry.register(Box::new(TabGroupRenderer));

    // Data renderers
    registry.register(Box::new(SheetRenderer));

    // Form renderers
    registry.register(Box::new(FormRenderer));

    // Commerce renderers
    registry.register(Box::new(ProductRenderer));

    // Academic renderers
    registry.register(Box::new(FootnoteRenderer));
    registry.register(Box::new(CitationRenderer));
}

/// Total number of built-in renderers.
pub const BUILTIN_RENDERER_COUNT: usize = 23;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_digit(dtype: &str) -> Digit {
        Digit::new(dtype.into(), Value::Null, "cpub1test".into()).unwrap()
    }

    fn ctx() -> RenderContext {
        RenderContext::default()
    }

    // --- Core renderer tests ---

    #[test]
    fn text_renderer_basic() {
        let digit = make_digit("text")
            .with_content(Value::from("Hello, world!"), "test");
        let spec = TextRenderer.render(&digit, RenderMode::Display, &ctx());
        assert_eq!(spec.digit_type, "text");
        assert_eq!(spec.accessibility.role, AccessibilityRole::Custom("text".into()));
        assert!(!spec.accessibility.hidden);
        assert!(spec.properties.contains_key("text"));
    }

    #[test]
    fn text_renderer_editing_mode() {
        let digit = make_digit("text")
            .with_content(Value::from("Edit me"), "test");
        let spec = TextRenderer.render(&digit, RenderMode::Editing, &ctx());
        assert_eq!(spec.properties.get("editable"), Some(&serde_json::json!(true)));
    }

    #[test]
    fn image_renderer_basic() {
        let digit = make_digit("image")
            .with_property("alt".into(), Value::from("A sunset"), "test");
        let spec = ImageRenderer.render(&digit, RenderMode::Display, &ctx());
        assert_eq!(spec.accessibility.role, AccessibilityRole::Image);
        assert_eq!(spec.accessibility.label, "A sunset");
    }

    #[test]
    fn image_renderer_thumbnail() {
        let digit = make_digit("image");
        let spec = ImageRenderer.render(&digit, RenderMode::Thumbnail, &ctx());
        assert_eq!(spec.estimated_width, 120.0);
        assert_eq!(spec.estimated_height, 90.0);
    }

    #[test]
    fn code_renderer_with_language() {
        let digit = make_digit("code")
            .with_property("code".into(), Value::from("fn main() {}"), "test")
            .with_property("language".into(), Value::from("rust"), "test");
        let spec = CodeRenderer.render(&digit, RenderMode::Display, &ctx());
        assert_eq!(spec.properties.get("language"), Some(&serde_json::json!("rust")));
        assert!(spec.properties.contains_key("monospace"));
    }

    #[test]
    fn container_renderer_formation() {
        let digit = make_digit("container")
            .with_property("formation".into(), Value::from("rank"), "test");
        let spec = ContainerRenderer.render(&digit, RenderMode::Display, &ctx());
        assert_eq!(spec.properties.get("formation"), Some(&serde_json::json!("rank")));
    }

    #[test]
    fn table_renderer_dimensions() {
        let digit = make_digit("table")
            .with_property("rows".into(), Value::Int(5), "test")
            .with_property("columns".into(), Value::Int(3), "test");
        let spec = TableRenderer.render(&digit, RenderMode::Display, &ctx());
        assert_eq!(spec.accessibility.role, AccessibilityRole::Table);
        assert_eq!(spec.properties.get("rows"), Some(&serde_json::json!(5)));
        assert_eq!(spec.properties.get("columns"), Some(&serde_json::json!(3)));
    }

    #[test]
    fn divider_renderer_is_decorative() {
        let digit = make_digit("divider");
        let spec = DividerRenderer.render(&digit, RenderMode::Display, &ctx());
        assert!(spec.accessibility.hidden);
    }

    #[test]
    fn link_renderer_interactive() {
        let digit = make_digit("link")
            .with_content(Value::from("Click here"), "test")
            .with_property("url".into(), Value::from("https://example.com"), "test");
        let spec = LinkRenderer.render(&digit, RenderMode::Display, &ctx());
        assert_eq!(spec.accessibility.role, AccessibilityRole::Link);
        assert!(spec.accessibility.traits.contains(&AccessibilityTrait::Interactive));
    }

    // --- Rich text renderer tests ---

    #[test]
    fn heading_renderer_levels() {
        for level in 1..=6 {
            let digit = make_digit("text.heading")
                .with_property("text".into(), Value::from("Title"), "test")
                .with_property("level".into(), Value::Int(level), "test");
            let spec = HeadingRenderer.render(&digit, RenderMode::Display, &ctx());
            assert_eq!(spec.accessibility.role, AccessibilityRole::Heading);
            assert!(spec.accessibility.traits.contains(&AccessibilityTrait::Header));
            assert_eq!(spec.properties.get("level"), Some(&serde_json::json!(level)));
        }
    }

    #[test]
    fn paragraph_renderer_basic() {
        let digit = make_digit("text.paragraph")
            .with_property("text".into(), Value::from("Hello"), "test");
        let spec = ParagraphRenderer.render(&digit, RenderMode::Display, &ctx());
        assert!(spec.accessibility.traits.contains(&AccessibilityTrait::StaticText));
    }

    #[test]
    fn list_renderer_items() {
        let items = Value::Array(vec![Value::from("A"), Value::from("B"), Value::from("C")]);
        let digit = make_digit("text.list")
            .with_property("style".into(), Value::from("ordered"), "test")
            .with_property("items".into(), items, "test");
        let spec = ListRenderer.render(&digit, RenderMode::Display, &ctx());
        assert_eq!(spec.accessibility.role, AccessibilityRole::List);
        assert_eq!(spec.properties.get("item_count"), Some(&serde_json::json!(3)));
    }

    #[test]
    fn callout_renderer_warning() {
        let digit = make_digit("text.callout")
            .with_property("text".into(), Value::from("Be careful!"), "test")
            .with_property("style".into(), Value::from("warning"), "test");
        let spec = CalloutRenderer.render(&digit, RenderMode::Display, &ctx());
        assert_eq!(spec.accessibility.role, AccessibilityRole::Alert);
    }

    // --- Presentation renderer tests ---

    #[test]
    fn slide_renderer_basic() {
        let digit = make_digit("presentation.slide")
            .with_property("title".into(), Value::from("Introduction"), "test")
            .with_property("order".into(), Value::Int(0), "test")
            .with_property("layout".into(), Value::from("title"), "test");
        let spec = SlideRenderer.render(&digit, RenderMode::Display, &ctx());
        assert_eq!(spec.estimated_width, 960.0);
        assert_eq!(spec.estimated_height, 540.0);
        assert!(spec.accessibility.label.contains("Introduction"));
    }

    #[test]
    fn slide_renderer_thumbnail() {
        let digit = make_digit("presentation.slide")
            .with_property("title".into(), Value::from("Slide"), "test")
            .with_property("order".into(), Value::Int(0), "test")
            .with_property("layout".into(), Value::from("content"), "test");
        let spec = SlideRenderer.render(&digit, RenderMode::Thumbnail, &ctx());
        assert_eq!(spec.estimated_width, 192.0);
        assert_eq!(spec.estimated_height, 108.0);
    }

    // --- Interactive renderer tests ---

    #[test]
    fn button_renderer_primary() {
        let digit = make_digit("interactive.button")
            .with_property("label".into(), Value::from("Save"), "test")
            .with_property("style".into(), Value::from("primary"), "test");
        let spec = ButtonRenderer.render(&digit, RenderMode::Display, &ctx());
        assert_eq!(spec.accessibility.role, AccessibilityRole::Button);
        assert!(spec.accessibility.traits.contains(&AccessibilityTrait::Interactive));
        assert!(spec.accessibility.hint.is_some());
    }

    #[test]
    fn accordion_renderer_expanded() {
        let digit = make_digit("interactive.accordion")
            .with_property("title".into(), Value::from("Details"), "test")
            .with_property("expanded".into(), Value::Bool(true), "test");
        let spec = AccordionRenderer.render(&digit, RenderMode::Display, &ctx());
        assert_eq!(spec.accessibility.value.as_deref(), Some("expanded"));
    }

    #[test]
    fn accordion_renderer_collapsed() {
        let digit = make_digit("interactive.accordion")
            .with_property("title".into(), Value::from("Details"), "test")
            .with_property("expanded".into(), Value::Bool(false), "test");
        let spec = AccordionRenderer.render(&digit, RenderMode::Display, &ctx());
        assert_eq!(spec.accessibility.value.as_deref(), Some("collapsed"));
    }

    #[test]
    fn tab_group_renderer_tabs() {
        let tabs = Value::Array(vec![Value::from("Tab A"), Value::from("Tab B")]);
        let digit = make_digit("interactive.tab-group")
            .with_property("tabs".into(), tabs, "test")
            .with_property("active_index".into(), Value::Int(1), "test");
        let spec = TabGroupRenderer.render(&digit, RenderMode::Display, &ctx());
        assert!(spec.accessibility.label.contains("Tab B"));
    }

    // --- Data renderer tests ---

    #[test]
    fn sheet_renderer_basic() {
        let digit = make_digit("data.sheet")
            .with_property("name".into(), Value::from("Inventory"), "test")
            .with_property("default_view".into(), Value::from("grid"), "test");
        let spec = SheetRenderer.render(&digit, RenderMode::Display, &ctx());
        assert_eq!(spec.accessibility.role, AccessibilityRole::Table);
        assert_eq!(spec.accessibility.label, "Inventory");
    }

    // --- Form renderer tests ---

    #[test]
    fn form_renderer_basic() {
        let digit = make_digit("form.container")
            .with_property("name".into(), Value::from("Signup"), "test");
        let spec = FormRenderer.render(&digit, RenderMode::Display, &ctx());
        assert_eq!(spec.accessibility.role, AccessibilityRole::Form);
        assert_eq!(spec.accessibility.label, "Signup");
    }

    // --- Commerce renderer tests ---

    #[test]
    fn product_renderer_with_price() {
        let digit = make_digit("commerce.product")
            .with_property("title".into(), Value::from("Widget"), "test")
            .with_property("price_cents".into(), Value::Int(1999), "test");
        let spec = ProductRenderer.render(&digit, RenderMode::Display, &ctx());
        assert!(spec.accessibility.label.contains("Widget"));
        assert!(spec.accessibility.label.contains("19.99"));
        assert_eq!(spec.accessibility.custom_actions.len(), 1);
        assert_eq!(spec.accessibility.custom_actions[0].name, "Add to cart");
    }

    // --- Academic renderer tests ---

    #[test]
    fn footnote_renderer_basic() {
        let digit = make_digit("text.footnote")
            .with_property("marker".into(), Value::from("1"), "test")
            .with_property("text".into(), Value::from("See reference"), "test");
        let spec = FootnoteRenderer.render(&digit, RenderMode::Display, &ctx());
        assert!(spec.accessibility.label.contains("Footnote 1"));
    }

    #[test]
    fn citation_renderer_with_author() {
        let digit = make_digit("text.citation")
            .with_property("source".into(), Value::from("The Book"), "test")
            .with_property("author".into(), Value::from("J. Doe"), "test");
        let spec = CitationRenderer.render(&digit, RenderMode::Display, &ctx());
        assert!(spec.accessibility.label.contains("J. Doe"));
    }

    // --- Registry tests ---

    #[test]
    fn register_all_populates_registry() {
        let mut registry = RendererRegistry::new();
        register_all_renderers(&mut registry);
        assert_eq!(registry.count(), BUILTIN_RENDERER_COUNT);
    }

    #[test]
    fn all_registered_types_can_render() {
        let mut registry = RendererRegistry::new();
        register_all_renderers(&mut registry);

        let types = [
            "text", "image", "code", "container", "table", "embed", "document",
            "divider", "link", "text.heading", "text.paragraph", "text.list",
            "text.blockquote", "text.callout", "presentation.slide",
            "interactive.button", "interactive.accordion", "interactive.tab-group",
            "data.sheet", "form.container", "commerce.product",
            "text.footnote", "text.citation",
        ];

        for dtype in types {
            assert!(registry.has_renderer(dtype), "missing renderer for {dtype}");
            let digit = make_digit(dtype);
            let spec = registry.render(&digit, RenderMode::Display, &ctx());
            assert_eq!(spec.digit_type, dtype);
        }
    }

    #[test]
    fn all_renderers_produce_accessibility() {
        let mut registry = RendererRegistry::new();
        register_all_renderers(&mut registry);

        let types = [
            "text", "image", "code", "container", "table", "embed", "document",
            "link", "text.heading", "text.paragraph", "text.list",
            "text.blockquote", "text.callout", "presentation.slide",
            "interactive.button", "interactive.accordion", "interactive.tab-group",
            "data.sheet", "form.container", "commerce.product",
            "text.footnote", "text.citation",
        ];

        for dtype in types {
            let digit = make_digit(dtype);
            let spec = registry.render(&digit, RenderMode::Display, &ctx());
            // Non-decorative elements should have a non-empty label
            assert!(!spec.accessibility.label.is_empty(),
                "renderer for {dtype} produced empty accessibility label");
        }
    }

    #[test]
    fn divider_is_decorative_in_registry() {
        let mut registry = RendererRegistry::new();
        register_all_renderers(&mut registry);
        let digit = make_digit("divider");
        let spec = registry.render(&digit, RenderMode::Display, &ctx());
        assert!(spec.accessibility.hidden);
    }

    #[test]
    fn all_renderers_support_all_modes() {
        let mut registry = RendererRegistry::new();
        register_all_renderers(&mut registry);

        let modes = [RenderMode::Display, RenderMode::Editing, RenderMode::Thumbnail, RenderMode::Print];

        for dtype in ["text", "image", "interactive.button", "presentation.slide"] {
            let digit = make_digit(dtype);
            for mode in &modes {
                let spec = registry.render(&digit, *mode, &ctx());
                assert_eq!(spec.mode, *mode);
            }
        }
    }
}
