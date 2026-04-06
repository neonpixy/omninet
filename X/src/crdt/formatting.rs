//! Peritext-inspired formatting CRDT for collaborative rich text editing.
//!
//! Formatting marks reference stable [`SequenceId`] positions from the sibling
//! [`SequenceRga`] character CRDT. This means formatting ranges survive
//! concurrent text insertions and deletions because they reference character
//! *identities*, not indices.
//!
//! ## Design
//!
//! Each [`FormatMark`] anchors its start and end to a [`SequenceId`] with an
//! [`AnchorSide`] that controls Peritext boundary expansion behavior:
//!
//! - `After` on the start anchor → expand to include insertions at the start
//! - `Before` on the end anchor → expand to include insertions at the end
//!
//! Conflict resolution uses last-writer-wins per attribute: the mark with the
//! latest timestamp determines whether an attribute is active and what its
//! value is.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::sequence::{SequenceId, SequenceRga};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Which side of a character the anchor is on.
///
/// Used for Peritext boundary behavior:
/// - `After` on start anchor = expand to include insertions at start
/// - `Before` on end anchor = expand to include insertions at end
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnchorSide {
    Before,
    After,
}

/// A position anchored to a specific character in the RGA.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MarkAnchor {
    pub id: SequenceId,
    pub side: AnchorSide,
}

/// Whether a mark adds or removes formatting.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MarkAction {
    Add,
    Remove,
}

/// A formatting mark spanning a range of characters.
///
/// References [`SequenceId`] positions for stability across concurrent edits.
/// A mark with [`MarkAction::Remove`] acts as a tombstone — it persists in
/// the map but suppresses the attribute when its timestamp is later than any
/// corresponding `Add` mark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormatMark {
    pub id: Uuid,
    pub start: MarkAnchor,
    pub end: MarkAnchor,
    pub attribute: String,
    pub value: serde_json::Value,
    pub action: MarkAction,
    pub author: String,
    pub timestamp: DateTime<Utc>,
}

/// An operation on the format map (for broadcasting).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FormatOp {
    /// Add a new formatting mark.
    AddMark(FormatMark),
    /// Remove a mark by ID (tombstone approach).
    RemoveMark {
        id: Uuid,
        author: String,
        timestamp: DateTime<Utc>,
    },
}

/// Inline text span with formatting attributes.
///
/// Matches the Ideas crate's `TextSpan` for compatibility. The daemon converts
/// between this and Ideas' `TextAttribute` when needed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextSpan {
    pub text: String,
    pub attributes: HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// FormatMap
// ---------------------------------------------------------------------------

/// Container for formatting marks alongside a [`SequenceRga`].
///
/// The `FormatMap` + `SequenceRga` together represent a formatted text field.
/// Operations are idempotent: applying the same op twice is safe.
pub struct FormatMap {
    marks: Vec<FormatMark>,
    /// Track applied mark IDs for idempotent apply.
    applied_ids: HashSet<Uuid>,
}

impl FormatMap {
    /// Create a new empty format map.
    pub fn new() -> Self {
        Self {
            marks: Vec::new(),
            applied_ids: HashSet::new(),
        }
    }

    /// Create and apply an Add mark. Returns the [`FormatOp`] to broadcast.
    pub fn add_mark(
        &mut self,
        start: MarkAnchor,
        end: MarkAnchor,
        attribute: impl Into<String>,
        value: serde_json::Value,
        author: impl Into<String>,
    ) -> FormatOp {
        let mark = FormatMark {
            id: Uuid::new_v4(),
            start,
            end,
            attribute: attribute.into(),
            value,
            action: MarkAction::Add,
            author: author.into(),
            timestamp: Utc::now(),
        };
        let op = FormatOp::AddMark(mark.clone());
        self.marks.push(mark.clone());
        self.applied_ids.insert(mark.id);
        op
    }

    /// Create and apply a Remove mark. Returns the [`FormatOp`] to broadcast.
    ///
    /// The original mark's `action` is set to [`MarkAction::Remove`] (tombstone
    /// approach — the mark persists but with Remove action and a later timestamp).
    /// Returns `None` if `mark_id` is not found.
    pub fn remove_mark(
        &mut self,
        mark_id: Uuid,
        author: impl Into<String>,
    ) -> Option<FormatOp> {
        // Find the original mark to copy its range.
        let original = self.marks.iter().find(|m| m.id == mark_id)?;

        let author_str = author.into();
        let timestamp = Utc::now();

        let removal = FormatMark {
            id: Uuid::new_v4(),
            start: original.start.clone(),
            end: original.end.clone(),
            attribute: original.attribute.clone(),
            value: original.value.clone(),
            action: MarkAction::Remove,
            author: author_str.clone(),
            timestamp,
        };

        let op = FormatOp::RemoveMark {
            id: mark_id,
            author: author_str,
            timestamp,
        };

        self.marks.push(removal.clone());
        self.applied_ids.insert(removal.id);
        Some(op)
    }

    /// Apply a remote [`FormatOp`]. Returns `true` if new (not a duplicate).
    ///
    /// Idempotent — applying the same op twice is safe.
    pub fn apply(&mut self, op: &FormatOp) -> bool {
        match op {
            FormatOp::AddMark(mark) => {
                if self.applied_ids.contains(&mark.id) {
                    return false;
                }
                self.applied_ids.insert(mark.id);
                self.marks.push(mark.clone());
                true
            }
            FormatOp::RemoveMark {
                id: mark_id,
                author,
                timestamp,
            } => {
                // Find the original mark to copy its range.
                let original = match self.marks.iter().find(|m| m.id == *mark_id) {
                    Some(m) => m.clone(),
                    None => return false,
                };

                let removal = FormatMark {
                    id: Uuid::new_v4(),
                    start: original.start.clone(),
                    end: original.end.clone(),
                    attribute: original.attribute.clone(),
                    value: original.value.clone(),
                    action: MarkAction::Remove,
                    author: author.clone(),
                    timestamp: *timestamp,
                };

                if self.applied_ids.contains(&removal.id) {
                    return false;
                }
                self.applied_ids.insert(removal.id);
                self.marks.push(removal);
                true
            }
        }
    }

    /// Resolve the active attributes at a given [`SequenceId`] position.
    ///
    /// Walks all marks covering that position. For each attribute: if any
    /// `Remove` mark is later than all `Add` marks, the attribute is off.
    /// Otherwise the latest `Add` mark's value wins.
    pub fn attributes_at(
        &self,
        target_id: &SequenceId,
        rga: &SequenceRga,
    ) -> HashMap<String, serde_json::Value> {
        // Group marks by attribute, keeping only those that cover the target.
        let mut by_attr: HashMap<&str, Vec<&FormatMark>> = HashMap::new();

        for mark in &self.marks {
            if self.is_in_range(target_id, mark, rga) {
                by_attr
                    .entry(&mark.attribute)
                    .or_default()
                    .push(mark);
            }
        }

        let mut result = HashMap::new();
        for (attr, marks) in by_attr {
            // Find the latest mark for this attribute.
            if let Some(winner) = marks.iter().max_by_key(|m| m.timestamp) {
                match winner.action {
                    MarkAction::Add => {
                        result.insert(attr.to_string(), winner.value.clone());
                    }
                    MarkAction::Remove => {
                        // Latest is a Remove — attribute is off.
                    }
                }
            }
        }

        result
    }

    /// Check if a [`SequenceId`] position falls within a mark's range.
    ///
    /// Uses the RGA's atom ordering to determine containment. Tombstoned
    /// atoms are part of the range (they still define positions).
    fn is_in_range(
        &self,
        target_id: &SequenceId,
        mark: &FormatMark,
        rga: &SequenceRga,
    ) -> bool {
        let target_idx = match rga.atom_index(target_id) {
            Some(idx) => idx,
            None => return false,
        };
        let start_idx = match rga.atom_index(&mark.start.id) {
            Some(idx) => idx,
            None => return false,
        };
        let end_idx = match rga.atom_index(&mark.end.id) {
            Some(idx) => idx,
            None => return false,
        };

        // Outside the atom index range entirely.
        if target_idx < start_idx || target_idx > end_idx {
            return false;
        }

        // AnchorSide boundary checks.
        if target_id == &mark.start.id && mark.start.side == AnchorSide::Before {
            return false;
        }
        if target_id == &mark.end.id && mark.end.side == AnchorSide::After {
            return false;
        }

        true
    }

    /// Convert all marks to [`TextSpan`] format for rendering.
    ///
    /// Walks through the RGA's visible characters, computes active attributes
    /// at each position, and groups consecutive characters with identical
    /// attributes into spans.
    pub fn to_spans(&self, rga: &SequenceRga) -> Vec<TextSpan> {
        let mut spans: Vec<TextSpan> = Vec::new();

        for (id, ch) in rga.visible_atoms() {
            let attrs = self.attributes_at(id, rga);

            match spans.last_mut() {
                Some(last) if last.attributes == attrs => {
                    last.text.push(ch);
                }
                _ => {
                    spans.push(TextSpan {
                        text: ch.to_string(),
                        attributes: attrs,
                    });
                }
            }
        }

        spans
    }

    /// Number of marks (including Remove marks).
    pub fn len(&self) -> usize {
        self.marks.len()
    }

    /// Whether the map has no marks.
    pub fn is_empty(&self) -> bool {
        self.marks.is_empty()
    }

    /// All marks (for snapshot/sync).
    pub fn marks(&self) -> &[FormatMark] {
        &self.marks
    }
}

impl Default for FormatMap {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    /// Helper: build an RGA with the given text, returning the RGA.
    fn rga_with_text(text: &str) -> SequenceRga {
        let mut rga = SequenceRga::new("test-replica");
        for (i, ch) in text.chars().enumerate() {
            rga.insert_at(i, ch);
        }
        rga
    }

    /// Helper: get the SequenceId at a visible position.
    fn id_at(rga: &SequenceRga, pos: usize) -> SequenceId {
        rga.position_to_id(pos).unwrap().clone()
    }

    /// Helper: create an expanding start anchor (After side).
    fn start_anchor(rga: &SequenceRga, pos: usize) -> MarkAnchor {
        MarkAnchor {
            id: id_at(rga, pos),
            side: AnchorSide::After,
        }
    }

    /// Helper: create an expanding end anchor (Before side).
    fn end_anchor(rga: &SequenceRga, pos: usize) -> MarkAnchor {
        MarkAnchor {
            id: id_at(rga, pos),
            side: AnchorSide::Before,
        }
    }

    /// Helper: create a mark directly with specific timestamps for testing.
    fn mark_with_timestamp(
        start: MarkAnchor,
        end: MarkAnchor,
        attribute: &str,
        value: serde_json::Value,
        action: MarkAction,
        author: &str,
        timestamp: DateTime<Utc>,
    ) -> FormatMark {
        FormatMark {
            id: Uuid::new_v4(),
            start,
            end,
            attribute: attribute.to_string(),
            value,
            action,
            author: author.to_string(),
            timestamp,
        }
    }

    // -----------------------------------------------------------------------
    // Basic operations
    // -----------------------------------------------------------------------

    #[test]
    fn new_format_map_is_empty() {
        let fm = FormatMap::new();
        assert!(fm.is_empty());
        assert_eq!(fm.len(), 0);
        assert!(fm.marks().is_empty());
    }

    #[test]
    fn add_single_mark() {
        let rga = rga_with_text("hello");
        let mut fm = FormatMap::new();

        let start = start_anchor(&rga, 0);
        let end = end_anchor(&rga, 4);
        fm.add_mark(start, end, "bold", serde_json::Value::Bool(true), "alice");

        assert_eq!(fm.len(), 1);
        assert!(!fm.is_empty());

        // Every character should be bold.
        for pos in 0..5 {
            let attrs = fm.attributes_at(&id_at(&rga, pos), &rga);
            assert_eq!(
                attrs.get("bold"),
                Some(&serde_json::Value::Bool(true)),
                "position {} should be bold",
                pos
            );
        }
    }

    #[test]
    fn add_mark_idempotent() {
        let rga = rga_with_text("abc");
        let mut fm = FormatMap::new();

        let start = start_anchor(&rga, 0);
        let end = end_anchor(&rga, 2);
        let op = fm.add_mark(start, end, "bold", serde_json::Value::Bool(true), "alice");

        // Apply the same op again — should return false.
        assert!(!fm.apply(&op), "duplicate apply should return false");
        // Still only one mark.
        assert_eq!(fm.len(), 1);
    }

    #[test]
    fn remove_mark() {
        let rga = rga_with_text("hello");
        let mut fm = FormatMap::new();

        let start = start_anchor(&rga, 0);
        let end = end_anchor(&rga, 4);
        let op = fm.add_mark(start, end, "bold", serde_json::Value::Bool(true), "alice");
        let mark_id = match &op {
            FormatOp::AddMark(m) => m.id,
            _ => panic!("expected AddMark"),
        };

        // Verify bold is active.
        let attrs = fm.attributes_at(&id_at(&rga, 2), &rga);
        assert!(attrs.contains_key("bold"));

        // Remove it.
        let remove_op = fm.remove_mark(mark_id, "alice");
        assert!(remove_op.is_some());

        // Now bold should be gone.
        let attrs = fm.attributes_at(&id_at(&rga, 2), &rga);
        assert!(!attrs.contains_key("bold"), "bold should be removed");
    }

    #[test]
    fn remove_nonexistent_returns_none() {
        let mut fm = FormatMap::new();
        assert!(fm.remove_mark(Uuid::new_v4(), "alice").is_none());
    }

    // -----------------------------------------------------------------------
    // Overlapping and composition
    // -----------------------------------------------------------------------

    #[test]
    fn overlapping_marks_compose() {
        let rga = rga_with_text("abcde");
        let mut fm = FormatMap::new();

        // Bold on [0..3] (a, b, c, d)
        fm.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 3),
            "bold",
            serde_json::Value::Bool(true),
            "alice",
        );

        // Italic on [2..4] (c, d, e)
        fm.add_mark(
            start_anchor(&rga, 2),
            end_anchor(&rga, 4),
            "italic",
            serde_json::Value::Bool(true),
            "bob",
        );

        // 'a' (pos 0): bold only
        let a = fm.attributes_at(&id_at(&rga, 0), &rga);
        assert!(a.contains_key("bold"));
        assert!(!a.contains_key("italic"));

        // 'c' (pos 2): bold + italic
        let c = fm.attributes_at(&id_at(&rga, 2), &rga);
        assert!(c.contains_key("bold"));
        assert!(c.contains_key("italic"));

        // 'e' (pos 4): italic only
        let e = fm.attributes_at(&id_at(&rga, 4), &rga);
        assert!(!e.contains_key("bold"));
        assert!(e.contains_key("italic"));
    }

    #[test]
    fn non_overlapping_marks_independent() {
        let rga = rga_with_text("abcdef");
        let mut fm = FormatMap::new();

        // Bold on [0..2] (a, b, c)
        fm.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "bold",
            serde_json::Value::Bool(true),
            "alice",
        );

        // Italic on [3..5] (d, e, f)
        fm.add_mark(
            start_anchor(&rga, 3),
            end_anchor(&rga, 5),
            "italic",
            serde_json::Value::Bool(true),
            "bob",
        );

        // 'a' → bold only
        let a = fm.attributes_at(&id_at(&rga, 0), &rga);
        assert!(a.contains_key("bold"));
        assert!(!a.contains_key("italic"));

        // 'd' → italic only
        let d = fm.attributes_at(&id_at(&rga, 3), &rga);
        assert!(!d.contains_key("bold"));
        assert!(d.contains_key("italic"));
    }

    // -----------------------------------------------------------------------
    // Timestamp-based conflict resolution
    // -----------------------------------------------------------------------

    #[test]
    fn remove_wins_over_add_by_timestamp() {
        let rga = rga_with_text("abc");
        let mut fm = FormatMap::new();

        let now = Utc::now();
        let later = now + Duration::seconds(1);

        let add = mark_with_timestamp(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "bold",
            serde_json::Value::Bool(true),
            MarkAction::Add,
            "alice",
            now,
        );
        let remove = mark_with_timestamp(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "bold",
            serde_json::Value::Bool(true),
            MarkAction::Remove,
            "bob",
            later,
        );

        fm.apply(&FormatOp::AddMark(add));
        fm.apply(&FormatOp::AddMark(remove));

        let attrs = fm.attributes_at(&id_at(&rga, 1), &rga);
        assert!(
            !attrs.contains_key("bold"),
            "Remove with later timestamp should win"
        );
    }

    #[test]
    fn add_wins_when_later_than_remove() {
        let rga = rga_with_text("abc");
        let mut fm = FormatMap::new();

        let now = Utc::now();
        let later = now + Duration::seconds(1);

        let remove = mark_with_timestamp(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "bold",
            serde_json::Value::Bool(true),
            MarkAction::Remove,
            "bob",
            now,
        );
        let add = mark_with_timestamp(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "bold",
            serde_json::Value::Bool(true),
            MarkAction::Add,
            "alice",
            later,
        );

        fm.apply(&FormatOp::AddMark(remove));
        fm.apply(&FormatOp::AddMark(add));

        let attrs = fm.attributes_at(&id_at(&rga, 1), &rga);
        assert_eq!(
            attrs.get("bold"),
            Some(&serde_json::Value::Bool(true)),
            "Add with later timestamp should win"
        );
    }

    #[test]
    fn concurrent_different_attributes() {
        let rga = rga_with_text("abc");
        let mut fm = FormatMap::new();

        fm.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "bold",
            serde_json::Value::Bool(true),
            "alice",
        );
        fm.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "italic",
            serde_json::Value::Bool(true),
            "bob",
        );

        let attrs = fm.attributes_at(&id_at(&rga, 1), &rga);
        assert!(attrs.contains_key("bold"));
        assert!(attrs.contains_key("italic"));
    }

    #[test]
    fn concurrent_same_attribute_latest_wins() {
        let rga = rga_with_text("abc");
        let mut fm = FormatMap::new();

        let now = Utc::now();
        let later = now + Duration::seconds(1);

        let red = mark_with_timestamp(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "color",
            serde_json::Value::String("#ff0000".into()),
            MarkAction::Add,
            "alice",
            now,
        );
        let blue = mark_with_timestamp(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "color",
            serde_json::Value::String("#0000ff".into()),
            MarkAction::Add,
            "bob",
            later,
        );

        fm.apply(&FormatOp::AddMark(red));
        fm.apply(&FormatOp::AddMark(blue));

        let attrs = fm.attributes_at(&id_at(&rga, 1), &rga);
        assert_eq!(
            attrs.get("color"),
            Some(&serde_json::Value::String("#0000ff".into())),
            "Latest color (blue) should win"
        );
    }

    // -----------------------------------------------------------------------
    // Boundary behavior (Peritext expansion)
    // -----------------------------------------------------------------------

    #[test]
    fn boundary_insert_at_start() {
        // Bold range starts with After anchor at position 0.
        // Inserting at position 0 (before the anchor char) should NOT be bold.
        // But inserting after the anchor char's position should be bold.
        let mut rga = SequenceRga::new("test-replica");
        // Build "abc"
        rga.insert_at(0, 'a');
        rga.insert_at(1, 'b');
        rga.insert_at(2, 'c');

        let mut fm = FormatMap::new();

        // Bold [a..c] with After/Before anchors (expanding).
        let start = MarkAnchor {
            id: id_at(&rga, 0),
            side: AnchorSide::After,
        };
        let end = MarkAnchor {
            id: id_at(&rga, 2),
            side: AnchorSide::Before,
        };
        fm.add_mark(start, end, "bold", serde_json::Value::Bool(true), "alice");

        // Insert 'x' at position 0 (before 'a', with parent=None).
        // This goes before the start anchor character.
        rga.insert_after(None, 'x');

        // 'x' is now at position 0. The bold start is anchored After 'a'.
        // Since 'x' is before 'a' in the atom list, it is outside the bold range.
        // Find 'x' — it's the newest insert.
        let x_id = rga.position_to_id(0).unwrap();
        let attrs = fm.attributes_at(x_id, &rga);
        assert!(
            !attrs.contains_key("bold"),
            "char inserted before start anchor should not be bold"
        );
    }

    #[test]
    fn boundary_insert_at_end() {
        // Bold range ends with Before anchor at position 2.
        // Inserting after position 2 should expand into the bold range
        // because Before means "include me".
        let mut rga = SequenceRga::new("test-replica");
        rga.insert_at(0, 'a');
        rga.insert_at(1, 'b');
        rga.insert_at(2, 'c');

        let mut fm = FormatMap::new();

        let start = MarkAnchor {
            id: id_at(&rga, 0),
            side: AnchorSide::After,
        };
        let end = MarkAnchor {
            id: id_at(&rga, 2),
            side: AnchorSide::Before,
        };
        fm.add_mark(start, end, "bold", serde_json::Value::Bool(true), "alice");

        // Insert 'z' after 'c' (position 2).
        let id_c = id_at(&rga, 2);
        rga.insert_after(Some(&id_c), 'z');

        // 'z' is after 'c' in the atom list. The end anchor is Before 'c',
        // so 'c' is included but 'z' (which comes after 'c') is outside.
        let z_id = rga.position_to_id(3).unwrap();
        let z_attrs = fm.attributes_at(z_id, &rga);
        assert!(
            !z_attrs.contains_key("bold"),
            "char inserted after end anchor char should not be bold"
        );

        // 'c' itself should still be bold (Before means include).
        let c_attrs = fm.attributes_at(&id_c, &rga);
        assert!(
            c_attrs.contains_key("bold"),
            "end anchor char with Before side should be bold"
        );
    }

    #[test]
    fn boundary_insert_before_start() {
        // Insert a char before the start of the bold range.
        let mut rga = SequenceRga::new("test-replica");
        rga.insert_at(0, 'a');
        rga.insert_at(1, 'b');
        rga.insert_at(2, 'c');

        let mut fm = FormatMap::new();

        // Bold on [b..c] — start at pos 1 (After), end at pos 2 (Before).
        let start = MarkAnchor {
            id: id_at(&rga, 1),
            side: AnchorSide::After,
        };
        let end = MarkAnchor {
            id: id_at(&rga, 2),
            side: AnchorSide::Before,
        };
        fm.add_mark(start, end, "bold", serde_json::Value::Bool(true), "alice");

        // Insert 'x' between 'a' and 'b' (after 'a', before bold start).
        let id_a = id_at(&rga, 0);
        rga.insert_after(Some(&id_a), 'x');

        // 'x' is between 'a' and 'b' in the atom list, before the bold start.
        let x_id = rga.position_to_id(1).unwrap();
        let attrs = fm.attributes_at(x_id, &rga);
        assert!(
            !attrs.contains_key("bold"),
            "char inserted before bold start should not be formatted"
        );
    }

    // -----------------------------------------------------------------------
    // to_spans
    // -----------------------------------------------------------------------

    #[test]
    fn to_spans_empty_document() {
        let rga = SequenceRga::new("test-replica");
        let fm = FormatMap::new();
        let spans = fm.to_spans(&rga);
        assert!(spans.is_empty());
    }

    #[test]
    fn to_spans_no_formatting() {
        let rga = rga_with_text("hello");
        let fm = FormatMap::new();
        let spans = fm.to_spans(&rga);

        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "hello");
        assert!(spans[0].attributes.is_empty());
    }

    #[test]
    fn to_spans_single_bold_range() {
        // "Hello world" with "world" bold.
        let rga = rga_with_text("Hello world");
        let mut fm = FormatMap::new();

        // Bold on positions 6..10 ("world").
        fm.add_mark(
            start_anchor(&rga, 6),
            end_anchor(&rga, 10),
            "bold",
            serde_json::Value::Bool(true),
            "alice",
        );

        let spans = fm.to_spans(&rga);
        assert_eq!(spans.len(), 2, "expected 2 spans, got {:?}", spans);

        assert_eq!(spans[0].text, "Hello ");
        assert!(spans[0].attributes.is_empty());

        assert_eq!(spans[1].text, "world");
        assert_eq!(
            spans[1].attributes.get("bold"),
            Some(&serde_json::Value::Bool(true))
        );
    }

    #[test]
    fn to_spans_multiple_attributes() {
        let rga = rga_with_text("abc");
        let mut fm = FormatMap::new();

        // Bold + italic on whole range.
        fm.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "bold",
            serde_json::Value::Bool(true),
            "alice",
        );
        fm.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "italic",
            serde_json::Value::Bool(true),
            "bob",
        );

        let spans = fm.to_spans(&rga);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "abc");
        assert!(spans[0].attributes.contains_key("bold"));
        assert!(spans[0].attributes.contains_key("italic"));
    }

    #[test]
    fn to_spans_adjacent_different() {
        // "abc" with 'a' bold and 'c' italic.
        let rga = rga_with_text("abc");
        let mut fm = FormatMap::new();

        // Bold on [0..0] (just 'a').
        fm.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 0),
            "bold",
            serde_json::Value::Bool(true),
            "alice",
        );

        // Italic on [2..2] (just 'c').
        fm.add_mark(
            start_anchor(&rga, 2),
            end_anchor(&rga, 2),
            "italic",
            serde_json::Value::Bool(true),
            "bob",
        );

        let spans = fm.to_spans(&rga);
        assert_eq!(spans.len(), 3, "expected 3 spans: bold, plain, italic");

        assert_eq!(spans[0].text, "a");
        assert!(spans[0].attributes.contains_key("bold"));

        assert_eq!(spans[1].text, "b");
        assert!(spans[1].attributes.is_empty());

        assert_eq!(spans[2].text, "c");
        assert!(spans[2].attributes.contains_key("italic"));
    }

    #[test]
    fn to_spans_whole_document_formatted() {
        let rga = rga_with_text("hello");
        let mut fm = FormatMap::new();

        fm.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 4),
            "bold",
            serde_json::Value::Bool(true),
            "alice",
        );

        let spans = fm.to_spans(&rga);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "hello");
        assert!(spans[0].attributes.contains_key("bold"));
    }

    // -----------------------------------------------------------------------
    // Serialization
    // -----------------------------------------------------------------------

    #[test]
    fn format_op_serde_round_trip() {
        let mark = FormatMark {
            id: Uuid::new_v4(),
            start: MarkAnchor {
                id: SequenceId {
                    replica_id: "r1".into(),
                    seq: 1,
                },
                side: AnchorSide::After,
            },
            end: MarkAnchor {
                id: SequenceId {
                    replica_id: "r1".into(),
                    seq: 5,
                },
                side: AnchorSide::Before,
            },
            attribute: "bold".into(),
            value: serde_json::Value::Bool(true),
            action: MarkAction::Add,
            author: "alice".into(),
            timestamp: Utc::now(),
        };
        let op = FormatOp::AddMark(mark);
        let json = serde_json::to_string(&op).unwrap();
        let deserialized: FormatOp = serde_json::from_str(&json).unwrap();

        // Verify round-trip produces valid JSON.
        let re_json = serde_json::to_string(&deserialized).unwrap();
        assert_eq!(json, re_json);
    }

    #[test]
    fn format_mark_serde_round_trip() {
        let mark = FormatMark {
            id: Uuid::new_v4(),
            start: MarkAnchor {
                id: SequenceId {
                    replica_id: "r1".into(),
                    seq: 1,
                },
                side: AnchorSide::After,
            },
            end: MarkAnchor {
                id: SequenceId {
                    replica_id: "r1".into(),
                    seq: 3,
                },
                side: AnchorSide::Before,
            },
            attribute: "italic".into(),
            value: serde_json::Value::Bool(true),
            action: MarkAction::Remove,
            author: "bob".into(),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&mark).unwrap();
        let deserialized: FormatMark = serde_json::from_str(&json).unwrap();
        assert_eq!(mark.id, deserialized.id);
        assert_eq!(mark.attribute, deserialized.attribute);
        assert_eq!(mark.action, deserialized.action);
    }

    #[test]
    fn mark_anchor_serde() {
        let anchor = MarkAnchor {
            id: SequenceId {
                replica_id: "r1".into(),
                seq: 42,
            },
            side: AnchorSide::Before,
        };
        let json = serde_json::to_string(&anchor).unwrap();
        let deserialized: MarkAnchor = serde_json::from_str(&json).unwrap();
        assert_eq!(anchor, deserialized);
    }

    // -----------------------------------------------------------------------
    // is_in_range
    // -----------------------------------------------------------------------

    #[test]
    fn is_in_range_basic() {
        let rga = rga_with_text("abcde");
        let fm = FormatMap::new();

        let mark = FormatMark {
            id: Uuid::new_v4(),
            start: start_anchor(&rga, 1),
            end: end_anchor(&rga, 3),
            attribute: "bold".into(),
            value: serde_json::Value::Bool(true),
            action: MarkAction::Add,
            author: "alice".into(),
            timestamp: Utc::now(),
        };

        // 'b' (1), 'c' (2), 'd' (3) should be in range.
        assert!(fm.is_in_range(&id_at(&rga, 1), &mark, &rga));
        assert!(fm.is_in_range(&id_at(&rga, 2), &mark, &rga));
        assert!(fm.is_in_range(&id_at(&rga, 3), &mark, &rga));
    }

    #[test]
    fn is_in_range_outside() {
        let rga = rga_with_text("abcde");
        let fm = FormatMap::new();

        let mark = FormatMark {
            id: Uuid::new_v4(),
            start: start_anchor(&rga, 1),
            end: end_anchor(&rga, 3),
            attribute: "bold".into(),
            value: serde_json::Value::Bool(true),
            action: MarkAction::Add,
            author: "alice".into(),
            timestamp: Utc::now(),
        };

        // 'a' (0) and 'e' (4) should be outside.
        assert!(!fm.is_in_range(&id_at(&rga, 0), &mark, &rga));
        assert!(!fm.is_in_range(&id_at(&rga, 4), &mark, &rga));
    }

    #[test]
    fn is_in_range_at_boundaries() {
        let rga = rga_with_text("abcde");
        let fm = FormatMap::new();

        // Start: After at pos 1, End: Before at pos 3.
        // The anchor chars themselves are included (After = include self, Before = include self).
        let mark = FormatMark {
            id: Uuid::new_v4(),
            start: MarkAnchor {
                id: id_at(&rga, 1),
                side: AnchorSide::After,
            },
            end: MarkAnchor {
                id: id_at(&rga, 3),
                side: AnchorSide::Before,
            },
            attribute: "bold".into(),
            value: serde_json::Value::Bool(true),
            action: MarkAction::Add,
            author: "alice".into(),
            timestamp: Utc::now(),
        };

        // Start char (After side) is included.
        assert!(fm.is_in_range(&id_at(&rga, 1), &mark, &rga));
        // End char (Before side) is included.
        assert!(fm.is_in_range(&id_at(&rga, 3), &mark, &rga));

        // Now test with Before on start (excludes start char) and After on end (excludes end char).
        let mark2 = FormatMark {
            id: Uuid::new_v4(),
            start: MarkAnchor {
                id: id_at(&rga, 1),
                side: AnchorSide::Before,
            },
            end: MarkAnchor {
                id: id_at(&rga, 3),
                side: AnchorSide::After,
            },
            attribute: "italic".into(),
            value: serde_json::Value::Bool(true),
            action: MarkAction::Add,
            author: "bob".into(),
            timestamp: Utc::now(),
        };

        // Start char (Before side) is excluded.
        assert!(!fm.is_in_range(&id_at(&rga, 1), &mark2, &rga));
        // End char (After side) is excluded.
        assert!(!fm.is_in_range(&id_at(&rga, 3), &mark2, &rga));
        // Middle char is still included.
        assert!(fm.is_in_range(&id_at(&rga, 2), &mark2, &rga));
    }

    // -----------------------------------------------------------------------
    // Tombstones and deletion
    // -----------------------------------------------------------------------

    #[test]
    fn deleted_chars_excluded_from_spans() {
        let mut rga = SequenceRga::new("test-replica");
        rga.insert_at(0, 'a');
        rga.insert_at(1, 'b');
        rga.insert_at(2, 'c');
        rga.insert_at(3, 'd');

        let mut fm = FormatMap::new();

        // Bold on [0..3] (all chars).
        fm.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 3),
            "bold",
            serde_json::Value::Bool(true),
            "alice",
        );

        // Delete 'b' (position 1).
        rga.delete_at(1);

        let spans = fm.to_spans(&rga);
        // Should be "acd" as one bold span.
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "acd");
        assert!(spans[0].attributes.contains_key("bold"));
    }

    // -----------------------------------------------------------------------
    // Multiple marks same attribute
    // -----------------------------------------------------------------------

    #[test]
    fn multiple_marks_same_attribute_merge() {
        // Two overlapping bold marks — union should be bold.
        let rga = rga_with_text("abcde");
        let mut fm = FormatMap::new();

        // Bold on [0..2] (a, b, c).
        fm.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "bold",
            serde_json::Value::Bool(true),
            "alice",
        );

        // Bold on [2..4] (c, d, e).
        fm.add_mark(
            start_anchor(&rga, 2),
            end_anchor(&rga, 4),
            "bold",
            serde_json::Value::Bool(true),
            "bob",
        );

        // All should be bold — the union of both ranges.
        for pos in 0..5 {
            let attrs = fm.attributes_at(&id_at(&rga, pos), &rga);
            assert!(
                attrs.contains_key("bold"),
                "position {} should be bold",
                pos
            );
        }
    }

    // -----------------------------------------------------------------------
    // Rich attributes
    // -----------------------------------------------------------------------

    #[test]
    fn link_attribute_with_url_value() {
        let rga = rga_with_text("click here");
        let mut fm = FormatMap::new();

        fm.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 9),
            "link",
            serde_json::Value::String("https://omnidea.net".into()),
            "alice",
        );

        let attrs = fm.attributes_at(&id_at(&rga, 5), &rga);
        assert_eq!(
            attrs.get("link"),
            Some(&serde_json::Value::String("https://omnidea.net".into()))
        );
    }

    // -----------------------------------------------------------------------
    // Snapshot
    // -----------------------------------------------------------------------

    #[test]
    fn format_map_snapshot_marks() {
        let rga = rga_with_text("abc");
        let mut fm = FormatMap::new();

        fm.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "bold",
            serde_json::Value::Bool(true),
            "alice",
        );
        fm.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "italic",
            serde_json::Value::Bool(true),
            "bob",
        );

        let marks = fm.marks();
        assert_eq!(marks.len(), 2);
        assert_eq!(marks[0].attribute, "bold");
        assert_eq!(marks[1].attribute, "italic");
    }

    // -----------------------------------------------------------------------
    // Scale
    // -----------------------------------------------------------------------

    #[test]
    fn large_document_formatting() {
        let text: String = (0..1000).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
        let rga = rga_with_text(&text);
        let mut fm = FormatMap::new();

        // Bold on [100..199].
        fm.add_mark(
            start_anchor(&rga, 100),
            end_anchor(&rga, 199),
            "bold",
            serde_json::Value::Bool(true),
            "alice",
        );

        // Italic on [500..599].
        fm.add_mark(
            start_anchor(&rga, 500),
            end_anchor(&rga, 599),
            "italic",
            serde_json::Value::Bool(true),
            "bob",
        );

        let spans = fm.to_spans(&rga);
        // Should have 5 spans: plain, bold, plain, italic, plain.
        assert_eq!(spans.len(), 5, "expected 5 spans, got {}", spans.len());

        // Verify total text length matches.
        let total_len: usize = spans.iter().map(|s| s.text.len()).sum();
        assert_eq!(total_len, 1000);

        // Spot-check: position 150 should be bold.
        let attrs = fm.attributes_at(&id_at(&rga, 150), &rga);
        assert!(attrs.contains_key("bold"));

        // Position 550 should be italic.
        let attrs = fm.attributes_at(&id_at(&rga, 550), &rga);
        assert!(attrs.contains_key("italic"));

        // Position 300 should be plain.
        let attrs = fm.attributes_at(&id_at(&rga, 300), &rga);
        assert!(attrs.is_empty());
    }

    // -----------------------------------------------------------------------
    // Concurrent text + format edits
    // -----------------------------------------------------------------------

    #[test]
    fn concurrent_format_during_text_edit() {
        // Format a range, then insert a char inside the range.
        // The mark should cover the new char because it references IDs, not indices.
        let mut rga = SequenceRga::new("test-replica");
        rga.insert_at(0, 'a');
        rga.insert_at(1, 'b');
        rga.insert_at(2, 'c');

        let mut fm = FormatMap::new();

        // Bold on [a..c] with expanding anchors.
        fm.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "bold",
            serde_json::Value::Bool(true),
            "alice",
        );

        // Insert 'x' between 'a' and 'b' (after 'a').
        let id_a = id_at(&rga, 0);
        rga.insert_after(Some(&id_a), 'x');

        // 'x' should be in the bold range since it's between 'a' and 'c'.
        let x_id = rga.position_to_id(1).unwrap();
        let attrs = fm.attributes_at(x_id, &rga);
        assert!(
            attrs.contains_key("bold"),
            "char inserted inside bold range should be bold"
        );

        // Verify the spans include the new char.
        let spans = fm.to_spans(&rga);
        let total_text: String = spans.iter().map(|s| s.text.clone()).collect();
        assert_eq!(total_text, "axbc");
    }

    // -----------------------------------------------------------------------
    // Remote apply
    // -----------------------------------------------------------------------

    #[test]
    fn apply_remote_add_mark() {
        let rga = rga_with_text("abc");
        let mut fm1 = FormatMap::new();
        let mut fm2 = FormatMap::new();

        let op = fm1.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "bold",
            serde_json::Value::Bool(true),
            "alice",
        );

        // Apply on replica 2.
        assert!(fm2.apply(&op));
        let attrs = fm2.attributes_at(&id_at(&rga, 1), &rga);
        assert!(attrs.contains_key("bold"));
    }

    #[test]
    fn apply_remote_remove_mark() {
        let rga = rga_with_text("abc");
        let mut fm1 = FormatMap::new();
        let mut fm2 = FormatMap::new();

        let add_op = fm1.add_mark(
            start_anchor(&rga, 0),
            end_anchor(&rga, 2),
            "bold",
            serde_json::Value::Bool(true),
            "alice",
        );
        let mark_id = match &add_op {
            FormatOp::AddMark(m) => m.id,
            _ => panic!("expected AddMark"),
        };

        // Sync the add to replica 2.
        fm2.apply(&add_op);

        // Remove on replica 1.
        let remove_op = fm1.remove_mark(mark_id, "alice").unwrap();

        // Apply remove on replica 2.
        assert!(fm2.apply(&remove_op));

        // Bold should be gone on replica 2.
        let attrs = fm2.attributes_at(&id_at(&rga, 1), &rga);
        assert!(!attrs.contains_key("bold"));
    }

    #[test]
    fn default_trait_impl() {
        let fm: FormatMap = Default::default();
        assert!(fm.is_empty());
    }
}
