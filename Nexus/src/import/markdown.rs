//! Markdown importer — parse Markdown into Ideas digits using `pulldown-cmark`.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::config::ImportConfig;
use crate::error::NexusError;
use crate::output::ImportOutput;
use crate::traits::Importer;
use ideas::digit::Digit;
use ideas::media::ImageMeta;
use ideas::richtext::{
    BlockquoteMeta, CodeBlockMeta, HeadingMeta, ListMeta, ListStyle, ParagraphMeta,
};
use x::Value;

/// Imports Markdown text into Ideas digits.
///
/// Supported elements:
/// - Headings (H1-H6) -> `text.heading`
/// - Paragraphs -> `text.paragraph`
/// - Ordered/unordered lists -> `text.list`
/// - Fenced code blocks -> `text.code`
/// - Blockquotes -> `text.blockquote`
/// - Images -> `media.image`
/// - Thematic breaks, inline formatting folded into text
///
/// Unsupported elements are skipped with a warning.
#[derive(Debug)]
pub struct MarkdownImporter;

impl Importer for MarkdownImporter {
    fn id(&self) -> &str {
        "nexus.markdown.import"
    }

    fn display_name(&self) -> &str {
        "Markdown"
    }

    fn supported_mime_types(&self) -> &[&str] {
        &["text/markdown"]
    }

    fn import(
        &self,
        data: &[u8],
        config: &ImportConfig,
    ) -> Result<ImportOutput, NexusError> {
        let text = std::str::from_utf8(data)
            .map_err(|e| NexusError::ImportFailed(format!("invalid UTF-8: {e}")))?;

        let mut digits: Vec<Digit> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();
        let author = &config.author;

        // Create a document container digit as root.
        let root = Digit::new("document".into(), Value::Null, author.clone())
            .map_err(|e| NexusError::ImportFailed(e.to_string()))?;
        let root_id = root.id();
        let mut root_digit = root;

        let opts = Options::all();
        let parser = Parser::new_ext(text, opts);

        let mut text_buf = String::new();
        let mut heading_level: u8 = 1;
        let mut in_paragraph = false;
        let mut code_lang: Option<String> = None;
        let mut in_blockquote = false;
        let mut blockquote_text = String::new();
        let mut list_ordered = false;
        let mut list_items: Vec<String> = Vec::new();
        let mut current_list_item = String::new();
        let mut in_list_item = false;
        let mut pending_image: Option<(String, String, String)> = None; // (url, mime, title)

        for event in parser {
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    heading_level = heading_level_to_u8(level);
                    text_buf.clear();
                }
                Event::End(TagEnd::Heading(_)) => {
                    let meta = HeadingMeta {
                        level: heading_level,
                        text: text_buf.clone(),
                        spans: None,
                    };
                    match ideas::richtext::heading_digit(&meta, author) {
                        Ok(digit) => {
                            root_digit = root_digit.with_child(digit.id(), author);
                            digits.push(digit);
                        }
                        Err(e) => warnings.push(format!("Failed to create heading: {e}")),
                    }
                    text_buf.clear();
                }
                Event::Start(Tag::Paragraph) => {
                    // Only treat as paragraph if not inside another block.
                    if !in_blockquote && !in_list_item {
                        in_paragraph = true;
                        text_buf.clear();
                    }
                }
                Event::End(TagEnd::Paragraph) => {
                    if in_blockquote {
                        if !blockquote_text.is_empty() {
                            blockquote_text.push('\n');
                        }
                        blockquote_text.push_str(&text_buf);
                        text_buf.clear();
                    } else if in_list_item {
                        current_list_item.push_str(&text_buf);
                        text_buf.clear();
                    } else if in_paragraph {
                        in_paragraph = false;
                        if !text_buf.is_empty() {
                            let meta = ParagraphMeta {
                                text: text_buf.clone(),
                                spans: None,
                            };
                            match ideas::richtext::paragraph_digit(&meta, author) {
                                Ok(digit) => {
                                    root_digit = root_digit.with_child(digit.id(), author);
                                    digits.push(digit);
                                }
                                Err(e) => {
                                    warnings.push(format!("Failed to create paragraph: {e}"));
                                }
                            }
                        }
                        text_buf.clear();
                    }
                }
                Event::Start(Tag::CodeBlock(kind)) => {
                    code_lang = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                            let l = lang.trim().to_string();
                            if l.is_empty() {
                                None
                            } else {
                                Some(l)
                            }
                        }
                        pulldown_cmark::CodeBlockKind::Indented => None,
                    };
                    text_buf.clear();
                }
                Event::End(TagEnd::CodeBlock) => {
                    let meta = CodeBlockMeta {
                        code: text_buf.clone(),
                        language: code_lang.take(),
                    };
                    match ideas::richtext::code_block_digit(&meta, author) {
                        Ok(digit) => {
                            root_digit = root_digit.with_child(digit.id(), author);
                            digits.push(digit);
                        }
                        Err(e) => warnings.push(format!("Failed to create code block: {e}")),
                    }
                    text_buf.clear();
                }
                Event::Start(Tag::BlockQuote(_)) => {
                    in_blockquote = true;
                    blockquote_text.clear();
                }
                Event::End(TagEnd::BlockQuote(_)) => {
                    in_blockquote = false;
                    // Flush any remaining text.
                    if !text_buf.is_empty() {
                        if !blockquote_text.is_empty() {
                            blockquote_text.push('\n');
                        }
                        blockquote_text.push_str(&text_buf);
                        text_buf.clear();
                    }
                    let meta = BlockquoteMeta {
                        text: blockquote_text.clone(),
                        attribution: None,
                        spans: None,
                    };
                    match ideas::richtext::blockquote_digit(&meta, author) {
                        Ok(digit) => {
                            root_digit = root_digit.with_child(digit.id(), author);
                            digits.push(digit);
                        }
                        Err(e) => warnings.push(format!("Failed to create blockquote: {e}")),
                    }
                    blockquote_text.clear();
                }
                Event::Start(Tag::List(start)) => {
                    list_ordered = start.is_some();
                    list_items.clear();
                }
                Event::End(TagEnd::List(_)) => {
                    let style = if list_ordered {
                        ListStyle::Ordered
                    } else {
                        ListStyle::Unordered
                    };
                    let meta = ListMeta {
                        style,
                        items: list_items.clone(),
                    };
                    match ideas::richtext::list_digit(&meta, author) {
                        Ok(digit) => {
                            root_digit = root_digit.with_child(digit.id(), author);
                            digits.push(digit);
                        }
                        Err(e) => warnings.push(format!("Failed to create list: {e}")),
                    }
                    list_items.clear();
                }
                Event::Start(Tag::Item) => {
                    in_list_item = true;
                    current_list_item.clear();
                }
                Event::End(TagEnd::Item) => {
                    in_list_item = false;
                    current_list_item.push_str(&text_buf);
                    text_buf.clear();
                    list_items.push(current_list_item.clone());
                    current_list_item.clear();
                }
                Event::Start(Tag::Image { dest_url, title, .. }) => {
                    // Save image info; alt text arrives as Text events before End.
                    let mime = guess_image_mime(&dest_url);
                    pending_image = Some((dest_url.to_string(), mime, title.to_string()));
                    text_buf.clear();
                }
                Event::End(TagEnd::Image) => {
                    if let Some((url, mime, title)) = pending_image.take() {
                        let alt = text_buf.clone();
                        text_buf.clear();
                        let meta = ImageMeta {
                            hash: url,
                            mime,
                            width: 0,
                            height: 0,
                            size: 0,
                            blurhash: None,
                            thumbnail_hash: None,
                            alt: if alt.is_empty() && title.is_empty() {
                                None
                            } else if alt.is_empty() {
                                Some(title)
                            } else {
                                Some(alt)
                            },
                        };
                        match ideas::media::image_digit(&meta, author) {
                            Ok(digit) => {
                                root_digit = root_digit.with_child(digit.id(), author);
                                digits.push(digit);
                            }
                            Err(e) => warnings.push(format!("Failed to create image: {e}")),
                        }
                    }
                }
                Event::Text(t) => {
                    text_buf.push_str(&t);
                }
                Event::Code(c) => {
                    // Inline code — append as text.
                    text_buf.push('`');
                    text_buf.push_str(&c);
                    text_buf.push('`');
                }
                Event::SoftBreak | Event::HardBreak => {
                    text_buf.push('\n');
                }
                Event::Rule => {
                    // Thematic break — emit as an empty paragraph or skip.
                    // We skip it with no warning since it's purely visual.
                }
                _ => {
                    // Other events (footnotes, tables, etc.) are not yet mapped.
                }
            }
        }

        // Insert root at the front.
        digits.insert(0, root_digit);

        let mut output = ImportOutput::new(digits, Some(root_id));
        output.warnings = warnings;
        Ok(output)
    }
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn guess_image_mime(url: &str) -> String {
    let lower = url.to_lowercase();
    if lower.ends_with(".png") {
        "image/png".into()
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg".into()
    } else if lower.ends_with(".gif") {
        "image/gif".into()
    } else if lower.ends_with(".webp") {
        "image/webp".into()
    } else if lower.ends_with(".svg") {
        "image/svg+xml".into()
    } else {
        "image/unknown".into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn import_md(md: &str) -> ImportOutput {
        let config = ImportConfig::new("cpub1test");
        MarkdownImporter.import(md.as_bytes(), &config).unwrap()
    }

    #[test]
    fn import_heading() {
        let output = import_md("# Hello World");
        // Root + heading
        assert!(output.root_digit_id.is_some());
        let heading = output
            .digits
            .iter()
            .find(|d| d.digit_type() == "text.heading")
            .expect("should have a heading digit");
        let meta = ideas::richtext::parse_heading_meta(heading).unwrap();
        assert_eq!(meta.level, 1);
        assert_eq!(meta.text, "Hello World");
    }

    #[test]
    fn import_paragraph() {
        let output = import_md("This is a paragraph.");
        let para = output
            .digits
            .iter()
            .find(|d| d.digit_type() == "text.paragraph")
            .expect("should have a paragraph");
        let meta = ideas::richtext::parse_paragraph_meta(para).unwrap();
        assert_eq!(meta.text, "This is a paragraph.");
    }

    #[test]
    fn import_multiple_headings() {
        let md = "# H1\n\n## H2\n\n### H3";
        let output = import_md(md);
        let headings: Vec<_> = output
            .digits
            .iter()
            .filter(|d| d.digit_type() == "text.heading")
            .collect();
        assert_eq!(headings.len(), 3);
    }

    #[test]
    fn import_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let output = import_md(md);
        let code = output
            .digits
            .iter()
            .find(|d| d.digit_type() == "text.code")
            .expect("should have a code block");
        let meta = ideas::richtext::parse_code_block_meta(code).unwrap();
        assert_eq!(meta.code, "fn main() {}\n");
        assert_eq!(meta.language.as_deref(), Some("rust"));
    }

    #[test]
    fn import_unordered_list() {
        let md = "- First\n- Second\n- Third";
        let output = import_md(md);
        let list = output
            .digits
            .iter()
            .find(|d| d.digit_type() == "text.list")
            .expect("should have a list");
        let meta = ideas::richtext::parse_list_meta(list).unwrap();
        assert_eq!(meta.style, ListStyle::Unordered);
        assert_eq!(meta.items.len(), 3);
    }

    #[test]
    fn import_ordered_list() {
        let md = "1. One\n2. Two\n3. Three";
        let output = import_md(md);
        let list = output
            .digits
            .iter()
            .find(|d| d.digit_type() == "text.list")
            .expect("should have a list");
        let meta = ideas::richtext::parse_list_meta(list).unwrap();
        assert_eq!(meta.style, ListStyle::Ordered);
        assert_eq!(meta.items.len(), 3);
    }

    #[test]
    fn import_blockquote() {
        let md = "> This is a quote";
        let output = import_md(md);
        let bq = output
            .digits
            .iter()
            .find(|d| d.digit_type() == "text.blockquote")
            .expect("should have a blockquote");
        let meta = ideas::richtext::parse_blockquote_meta(bq).unwrap();
        assert!(meta.text.contains("This is a quote"));
    }

    #[test]
    fn import_image() {
        let md = "![Alt text](https://example.com/image.png)";
        let output = import_md(md);
        let img = output
            .digits
            .iter()
            .find(|d| d.digit_type() == "media.image")
            .expect("should have an image");
        let meta = ideas::media::parse_image_meta(img).unwrap();
        assert_eq!(meta.hash, "https://example.com/image.png");
        assert_eq!(meta.mime, "image/png");
        assert_eq!(meta.alt.as_deref(), Some("Alt text"));
    }

    #[test]
    fn import_invalid_utf8() {
        let data = &[0xFF, 0xFE, 0xFD];
        let config = ImportConfig::new("cpub1test");
        let result = MarkdownImporter.import(data, &config);
        assert!(result.is_err());
    }

    #[test]
    fn root_has_children() {
        let md = "# Title\n\nParagraph.\n\n- item";
        let output = import_md(md);
        let root = &output.digits[0];
        assert_eq!(root.digit_type(), "document");
        assert!(root.has_children());
    }

    #[test]
    fn empty_markdown_produces_root_only() {
        let output = import_md("");
        assert_eq!(output.digits.len(), 1);
        assert_eq!(output.digits[0].digit_type(), "document");
    }
}
