use sha2::{Digest, Sha256};

/// Compute the content-addressed event ID from its fields.
///
/// The ID is the SHA-256 hash (hex-encoded, 64 chars) of the canonical
/// serialization: `[0,"<author>",<created_at>,<kind>,<tags>,"<content>"]`
///
/// This function uses manual JSON construction — never serde — to guarantee
/// byte-exact determinism across all platforms.
pub fn compute_id(
    author: &str,
    created_at: i64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> String {
    let bytes = canonical_serialize(author, created_at, kind, tags, content);
    let hash = Sha256::digest(&bytes);
    hex::encode(hash)
}

/// Produce the canonical byte representation for signing and ID computation.
///
/// Format: `[0,"<author>",<created_at>,<kind>,<tags_json>,"<content>"]`
pub(crate) fn canonical_serialize(
    author: &str,
    created_at: i64,
    kind: u32,
    tags: &[Vec<String>],
    content: &str,
) -> Vec<u8> {
    let mut out = String::with_capacity(256);
    out.push_str("[0,\"");
    out.push_str(author);
    out.push_str("\",");
    out.push_str(&created_at.to_string());
    out.push(',');
    out.push_str(&kind.to_string());
    out.push(',');
    serialize_tags(tags, &mut out);
    out.push_str(",\"");
    escape_json(content, &mut out);
    out.push_str("\"]");
    out.into_bytes()
}

/// Serialize tags as a JSON array of string arrays.
fn serialize_tags(tags: &[Vec<String>], out: &mut String) {
    out.push('[');
    for (i, tag) in tags.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push('[');
        for (j, val) in tag.iter().enumerate() {
            if j > 0 {
                out.push(',');
            }
            out.push('"');
            escape_json(val, out);
            out.push('"');
        }
        out.push(']');
    }
    out.push(']');
}

/// Escape a string for JSON embedding.
///
/// Handles: `"`, `\`, `\n`, `\r`, `\t`, and control characters (< 0x20).
fn escape_json(s: &str, out: &mut String) {
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                // Control characters as \u00xx.
                let code = c as u32;
                out.push_str(&format!("\\u{code:04x}"));
            }
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_same_inputs_same_output() {
        let id1 = compute_id("abc", 1000, 1, &[], "hello");
        let id2 = compute_id("abc", 1000, 1, &[], "hello");
        assert_eq!(id1, id2);
    }

    #[test]
    fn different_content_different_id() {
        let id1 = compute_id("abc", 1000, 1, &[], "hello");
        let id2 = compute_id("abc", 1000, 1, &[], "world");
        assert_ne!(id1, id2);
    }

    #[test]
    fn id_is_64_hex_chars() {
        let id = compute_id("abc", 1000, 1, &[], "test");
        assert_eq!(id.len(), 64);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn canonical_format_no_tags() {
        let bytes = canonical_serialize("deadbeef", 12345, 1, &[], "hello");
        let s = String::from_utf8(bytes).unwrap();
        assert_eq!(s, r#"[0,"deadbeef",12345,1,[],"hello"]"#);
    }

    #[test]
    fn canonical_format_with_tags() {
        let tags = vec![
            vec!["e".to_string(), "abc123".to_string()],
            vec!["p".to_string(), "def456".to_string()],
        ];
        let bytes = canonical_serialize("aabbcc", 99, 7000, &tags, "");
        let s = String::from_utf8(bytes).unwrap();
        assert_eq!(
            s,
            r#"[0,"aabbcc",99,7000,[["e","abc123"],["p","def456"]],""]"#
        );
    }

    #[test]
    fn escape_special_characters() {
        let bytes = canonical_serialize("aa", 0, 0, &[], "line1\nline2\t\"quoted\"\\back");
        let s = String::from_utf8(bytes).unwrap();
        assert!(s.contains(r#"line1\nline2\t\"quoted\"\\back"#));
    }
}
