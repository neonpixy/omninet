//! SMTP bridge — convert Equipment `MailMessage` to RFC 5322 email bytes.
//!
//! This bridge produces the raw email bytes (headers + MIME multipart body)
//! but does NOT send them. The platform layer is responsible for opening
//! the socket and transmitting the bytes.

use chrono::Utc;

use crate::config::BridgeConfig;
use crate::error::NexusError;
use crate::output::BridgeResult;
use crate::traits::ProtocolBridge;
use equipment::{MailMessage, RecipientRole};

/// Bridge that converts an Equipment `MailMessage` into RFC 5322 email bytes.
///
/// The bridge:
/// 1. Maps `from` (crown_id) to an email address using config settings.
/// 2. Maps recipients (crown_id) to email addresses using config settings.
/// 3. Converts the `.idea` body content to basic HTML.
/// 4. Produces RFC 5322 formatted bytes with MIME multipart (text/plain + text/html).
///
/// The result contains the raw email bytes in `response["rfc5322"]` as a
/// base64-encoded string, ready for the platform layer to send via SMTP.
///
/// # Config Settings
///
/// The `BridgeConfig.settings` object may contain:
/// - `"from_email"` — email address for the From header (required).
/// - `"from_name"` — display name for the From header (optional).
/// - `"address_map"` — JSON object mapping crown_id -> email address (required
///   for any recipient that needs to receive the email).
/// - `"domain"` — domain for the Message-ID header (defaults to "omnidea.local").
#[derive(Debug)]
pub struct SmtpBridge;

impl ProtocolBridge for SmtpBridge {
    fn id(&self) -> &str {
        "nexus.smtp.bridge"
    }

    fn display_name(&self) -> &str {
        "SMTP Email Bridge"
    }

    fn bridge(
        &self,
        message: &MailMessage,
        config: &BridgeConfig,
    ) -> Result<BridgeResult, NexusError> {
        let settings = &config.settings;

        // Extract required from_email.
        let from_email = settings
            .get("from_email")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                NexusError::InvalidConfig("missing 'from_email' in bridge settings".into())
            })?;
        let from_name = settings
            .get("from_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let domain = settings
            .get("domain")
            .and_then(|v| v.as_str())
            .unwrap_or("omnidea.local");
        let address_map = settings
            .get("address_map")
            .and_then(|v| v.as_object())
            .ok_or_else(|| {
                NexusError::InvalidConfig("missing 'address_map' in bridge settings".into())
            })?;

        // Resolve recipient email addresses.
        let mut to_addrs: Vec<String> = Vec::new();
        let mut cc_addrs: Vec<String> = Vec::new();
        let mut bcc_addrs: Vec<String> = Vec::new();

        for entry in &message.recipients {
            let email = address_map
                .get(&entry.recipient.crown_id)
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    NexusError::BridgeFailed(format!(
                        "no email address mapped for crown ID: {}",
                        entry.recipient.crown_id
                    ))
                })?;
            let formatted = match &entry.recipient.display_name {
                Some(name) => format!("{name} <{email}>"),
                None => email.to_string(),
            };
            match entry.role {
                RecipientRole::To => to_addrs.push(formatted),
                RecipientRole::Cc => cc_addrs.push(formatted),
                RecipientRole::Bcc => bcc_addrs.push(formatted),
            }
        }

        if to_addrs.is_empty() && cc_addrs.is_empty() && bcc_addrs.is_empty() {
            return Err(NexusError::BridgeFailed(
                "no recipients resolved to email addresses".into(),
            ));
        }

        // Convert the .idea body to plain text and HTML.
        let plain_text = body_to_plain_text(&message.body);
        let html_body = body_to_html(&message.body);

        // Generate a unique Message-ID.
        let message_id = format!("<{}.{}@{}>", message.id, Utc::now().timestamp(), domain);

        // Build the MIME boundary.
        let boundary = format!("--nexus-{}", message.id);

        // Construct RFC 5322 headers.
        let mut rfc5322 = String::new();

        // From header.
        if from_name.is_empty() {
            rfc5322.push_str(&format!("From: {from_email}\r\n"));
        } else {
            rfc5322.push_str(&format!("From: {from_name} <{from_email}>\r\n"));
        }

        // To header.
        if !to_addrs.is_empty() {
            rfc5322.push_str(&format!("To: {}\r\n", to_addrs.join(", ")));
        }
        // Cc header.
        if !cc_addrs.is_empty() {
            rfc5322.push_str(&format!("Cc: {}\r\n", cc_addrs.join(", ")));
        }
        // Bcc is NOT included in headers (hidden recipients).

        // Subject.
        rfc5322.push_str(&format!("Subject: {}\r\n", message.subject));

        // Date.
        let date = message.timestamp.format("%a, %d %b %Y %H:%M:%S %z");
        rfc5322.push_str(&format!("Date: {date}\r\n"));

        // Message-ID.
        rfc5322.push_str(&format!("Message-ID: {message_id}\r\n"));

        // MIME version.
        rfc5322.push_str("MIME-Version: 1.0\r\n");

        // In-Reply-To if present.
        if let Some(ref reply_id) = message.in_reply_to {
            rfc5322.push_str(&format!("In-Reply-To: <{reply_id}@{domain}>\r\n"));
        }

        // Thread-ID as References if present.
        if let Some(ref thread_id) = message.thread_id {
            rfc5322.push_str(&format!("References: <{thread_id}@{domain}>\r\n"));
        }

        // Content-Type: multipart/alternative.
        rfc5322.push_str(&format!(
            "Content-Type: multipart/alternative; boundary=\"{boundary}\"\r\n"
        ));

        // End of headers.
        rfc5322.push_str("\r\n");

        // Plain text part.
        rfc5322.push_str(&format!("--{boundary}\r\n"));
        rfc5322.push_str("Content-Type: text/plain; charset=utf-8\r\n");
        rfc5322.push_str("Content-Transfer-Encoding: 8bit\r\n");
        rfc5322.push_str("\r\n");
        rfc5322.push_str(&plain_text);
        rfc5322.push_str("\r\n");

        // HTML part.
        rfc5322.push_str(&format!("--{boundary}\r\n"));
        rfc5322.push_str("Content-Type: text/html; charset=utf-8\r\n");
        rfc5322.push_str("Content-Transfer-Encoding: 8bit\r\n");
        rfc5322.push_str("\r\n");
        rfc5322.push_str(&html_body);
        rfc5322.push_str("\r\n");

        // Closing boundary.
        rfc5322.push_str(&format!("--{boundary}--\r\n"));

        let rfc5322_bytes = rfc5322.into_bytes();
        let encoded = base64_encode(&rfc5322_bytes);

        let response = serde_json::json!({
            "rfc5322": encoded,
            "message_id": message_id,
            "size_bytes": rfc5322_bytes.len(),
            "to_count": to_addrs.len(),
            "cc_count": cc_addrs.len(),
            "bcc_count": bcc_addrs.len(),
        });

        Ok(BridgeResult::ok(
            format!(
                "Email prepared for {} recipient(s)",
                to_addrs.len() + cc_addrs.len() + bcc_addrs.len()
            ),
            response,
        ))
    }
}

/// Convert .idea body JSON to plain text.
///
/// The body is opaque JSON from Equipment. We attempt to extract readable text.
fn body_to_plain_text(body: &str) -> String {
    // Try parsing as a digit.
    if let Ok(digit) = serde_json::from_str::<ideas::Digit>(body) {
        return digit.extract_text();
    }
    // Try parsing as an array of digits.
    if let Ok(digits) = serde_json::from_str::<Vec<ideas::Digit>>(body) {
        return digits
            .iter()
            .map(|d| d.extract_text())
            .collect::<Vec<_>>()
            .join("\n\n");
    }
    // Fall back to raw body as plain text.
    body.to_string()
}

/// Convert .idea body JSON to basic HTML.
///
/// Produces a minimal HTML document wrapping the body content.
fn body_to_html(body: &str) -> String {
    let plain = body_to_plain_text(body);
    // Escape HTML special characters.
    let escaped = plain
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");
    // Convert newlines to <br> tags.
    let html_body = escaped.replace('\n', "<br>\n");

    format!(
        "<!DOCTYPE html>\n<html>\n<head><meta charset=\"utf-8\"></head>\n<body>\n{html_body}\n</body>\n</html>"
    )
}

/// Simple base64 encoding without external dependency.
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use equipment::{MailMessage, MailRecipient, MailRecipientEntry, RecipientRole};
    use uuid::Uuid;

    fn test_message() -> MailMessage {
        MailMessage {
            id: Uuid::new_v4(),
            from: "cpub1alice".into(),
            recipients: vec![MailRecipientEntry {
                recipient: MailRecipient {
                    crown_id: "cpub1bob".into(),
                    display_name: Some("Bob".into()),
                },
                role: RecipientRole::To,
            }],
            subject: "Hello from Omnidea".into(),
            body: r#"Hello Bob, how are you?"#.into(),
            attachments: vec![],
            thread_id: None,
            in_reply_to: None,
            timestamp: Utc::now(),
            read: false,
        }
    }

    fn test_config() -> BridgeConfig {
        BridgeConfig::new("smtp")
            .with_setting("from_email", serde_json::json!("alice@example.com"))
            .with_setting("from_name", serde_json::json!("Alice"))
            .with_setting("domain", serde_json::json!("example.com"))
            .with_setting(
                "address_map",
                serde_json::json!({
                    "cpub1bob": "bob@example.com"
                }),
            )
    }

    #[test]
    fn bridge_produces_result() {
        let msg = test_message();
        let config = test_config();
        let result = SmtpBridge.bridge(&msg, &config).unwrap();
        assert!(result.success);
        assert!(result.response["rfc5322"].is_string());
        assert!(result.response["message_id"].is_string());
        assert_eq!(result.response["to_count"], 1);
    }

    #[test]
    fn bridge_missing_from_email() {
        let msg = test_message();
        let config = BridgeConfig::new("smtp")
            .with_setting(
                "address_map",
                serde_json::json!({"cpub1bob": "bob@example.com"}),
            );
        let result = SmtpBridge.bridge(&msg, &config);
        assert!(result.is_err());
    }

    #[test]
    fn bridge_missing_address_map() {
        let msg = test_message();
        let config = BridgeConfig::new("smtp")
            .with_setting("from_email", serde_json::json!("alice@example.com"));
        let result = SmtpBridge.bridge(&msg, &config);
        assert!(result.is_err());
    }

    #[test]
    fn bridge_unmapped_recipient() {
        let msg = test_message();
        let config = BridgeConfig::new("smtp")
            .with_setting("from_email", serde_json::json!("alice@example.com"))
            .with_setting("address_map", serde_json::json!({}));
        let result = SmtpBridge.bridge(&msg, &config);
        assert!(result.is_err());
    }

    #[test]
    fn bridge_with_cc_and_bcc() {
        let msg = MailMessage {
            id: Uuid::new_v4(),
            from: "cpub1alice".into(),
            recipients: vec![
                MailRecipientEntry {
                    recipient: MailRecipient {
                        crown_id: "cpub1bob".into(),
                        display_name: None,
                    },
                    role: RecipientRole::To,
                },
                MailRecipientEntry {
                    recipient: MailRecipient {
                        crown_id: "cpub1carol".into(),
                        display_name: Some("Carol".into()),
                    },
                    role: RecipientRole::Cc,
                },
                MailRecipientEntry {
                    recipient: MailRecipient {
                        crown_id: "cpub1dave".into(),
                        display_name: None,
                    },
                    role: RecipientRole::Bcc,
                },
            ],
            subject: "Group email".into(),
            body: "Hi everyone".into(),
            attachments: vec![],
            thread_id: Some("thread-001".into()),
            in_reply_to: None,
            timestamp: Utc::now(),
            read: false,
        };
        let config = BridgeConfig::new("smtp")
            .with_setting("from_email", serde_json::json!("alice@example.com"))
            .with_setting(
                "address_map",
                serde_json::json!({
                    "cpub1bob": "bob@example.com",
                    "cpub1carol": "carol@example.com",
                    "cpub1dave": "dave@example.com"
                }),
            );
        let result = SmtpBridge.bridge(&msg, &config).unwrap();
        assert!(result.success);
        assert_eq!(result.response["to_count"], 1);
        assert_eq!(result.response["cc_count"], 1);
        assert_eq!(result.response["bcc_count"], 1);
    }

    #[test]
    fn body_to_plain_text_raw_string() {
        let text = body_to_plain_text("Hello world");
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn body_to_html_escaping() {
        let html = body_to_html("<script>alert('xss')</script>");
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn base64_encode_basic() {
        assert_eq!(base64_encode(b"Hello"), "SGVsbG8=");
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"ab"), "YWI=");
        assert_eq!(base64_encode(b"abc"), "YWJj");
    }

    #[test]
    fn bridge_traits() {
        let bridge = SmtpBridge;
        assert_eq!(bridge.id(), "nexus.smtp.bridge");
        assert_eq!(bridge.display_name(), "SMTP Email Bridge");
    }

    #[test]
    fn bridge_with_reply_and_thread() {
        let reply_to_id = Uuid::new_v4();
        let msg = MailMessage {
            id: Uuid::new_v4(),
            from: "cpub1alice".into(),
            recipients: vec![MailRecipientEntry {
                recipient: MailRecipient {
                    crown_id: "cpub1bob".into(),
                    display_name: None,
                },
                role: RecipientRole::To,
            }],
            subject: "Re: Hello".into(),
            body: "Reply text".into(),
            attachments: vec![],
            thread_id: Some("thread-123".into()),
            in_reply_to: Some(reply_to_id),
            timestamp: Utc::now(),
            read: false,
        };
        let config = test_config();
        let result = SmtpBridge.bridge(&msg, &config).unwrap();
        assert!(result.success);
    }
}
