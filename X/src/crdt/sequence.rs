//! Character-level sequence CRDT (RGA-light) for real-time collaborative text editing.
//!
//! Every character gets a globally unique [`SequenceId`] (replica_id + monotonic counter).
//! Concurrent insertions at the same position are deterministically ordered by
//! lexicographic `replica_id` then numeric `seq`, so all replicas converge to
//! identical text regardless of operation arrival order.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Unique identifier for each character in the sequence.
///
/// `(replica_id, seq)` is globally unique -- no two replicas produce the same pair.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SequenceId {
    pub replica_id: String,
    pub seq: u64,
}

impl SequenceId {
    /// Deterministic total order used for tie-breaking concurrent inserts.
    /// Compares `replica_id` lexicographically, then `seq` numerically.
    fn cmp_tiebreak(&self, other: &Self) -> std::cmp::Ordering {
        self.replica_id
            .cmp(&other.replica_id)
            .then(self.seq.cmp(&other.seq))
    }
}

/// A single character with its identity and tombstone state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceAtom {
    pub id: SequenceId,
    pub value: char,
    pub deleted: bool,
    /// The ID of the atom this was inserted after (None = head of document).
    /// Needed for conflict resolution when replaying concurrent inserts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<SequenceId>,
}

/// An operation on the sequence (sent over the network for sync).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SequenceOp {
    /// Insert a character after an existing atom (or at the head if `after` is None).
    Insert {
        id: SequenceId,
        value: char,
        /// Insert after this ID. `None` = insert at the beginning.
        after: Option<SequenceId>,
    },
    /// Delete a character by its ID (tombstone).
    Delete { id: SequenceId },
}

// ---------------------------------------------------------------------------
// SequenceRga
// ---------------------------------------------------------------------------

/// The replicated sequence -- an ordered list of atoms with tombstones.
///
/// This is an RGA-light implementation suitable for character-level collaborative
/// editing. Operations are idempotent: applying the same op twice is safe.
#[derive(Debug)]
pub struct SequenceRga {
    atoms: Vec<SequenceAtom>,
    /// Fast lookup: id -> index in atoms vec.
    index: HashMap<SequenceId, usize>,
    /// This replica's unique identifier.
    pub replica_id: String,
    /// Monotonically increasing counter for generating unique [`SequenceId`]s.
    seq_counter: u64,
}

impl SequenceRga {
    /// Create a new empty sequence for the given replica.
    pub fn new(replica_id: impl Into<String>) -> Self {
        Self {
            atoms: Vec::new(),
            index: HashMap::new(),
            replica_id: replica_id.into(),
            seq_counter: 0,
        }
    }

    /// Insert a character after the given position. Returns the operation to broadcast.
    ///
    /// `after` is the [`SequenceId`] of the character to insert after (`None` = beginning).
    pub fn insert_after(&mut self, after: Option<&SequenceId>, value: char) -> SequenceOp {
        self.seq_counter += 1;
        let id = SequenceId {
            replica_id: self.replica_id.clone(),
            seq: self.seq_counter,
        };
        let op = SequenceOp::Insert {
            id: id.clone(),
            value,
            after: after.cloned(),
        };
        self.apply_insert(&id, value, after.cloned());
        op
    }

    /// Delete a character by its ID. Returns the operation to broadcast.
    ///
    /// Returns `None` if the character doesn't exist or is already deleted.
    pub fn delete(&mut self, id: &SequenceId) -> Option<SequenceOp> {
        let &idx = self.index.get(id)?;
        let atom = &mut self.atoms[idx];
        if atom.deleted {
            return None;
        }
        atom.deleted = true;
        Some(SequenceOp::Delete { id: id.clone() })
    }

    /// Apply a remote operation. Returns `true` if it was new (not a duplicate).
    ///
    /// Idempotent -- applying the same op twice is safe.
    pub fn apply(&mut self, op: &SequenceOp) -> bool {
        match op {
            SequenceOp::Insert { id, value, after } => {
                if self.index.contains_key(id) {
                    return false;
                }
                self.apply_insert(id, *value, after.clone());
                true
            }
            SequenceOp::Delete { id } => {
                if let Some(&idx) = self.index.get(id) {
                    let atom = &mut self.atoms[idx];
                    if atom.deleted {
                        return false;
                    }
                    atom.deleted = true;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Get the visible text (all non-deleted characters in order).
    pub fn text(&self) -> String {
        self.atoms
            .iter()
            .filter(|a| !a.deleted)
            .map(|a| a.value)
            .collect()
    }

    /// Number of visible (non-deleted) characters.
    pub fn len(&self) -> usize {
        self.atoms.iter().filter(|a| !a.deleted).count()
    }

    /// Whether the sequence is empty (no visible characters).
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Convert a visible position (0-based index into [`text()`](Self::text)) to a [`SequenceId`].
    ///
    /// Returns `None` if position is out of bounds.
    pub fn position_to_id(&self, pos: usize) -> Option<&SequenceId> {
        self.atoms
            .iter()
            .filter(|a| !a.deleted)
            .nth(pos)
            .map(|a| &a.id)
    }

    /// Convert a [`SequenceId`] to a visible position.
    ///
    /// Returns `None` if the ID doesn't exist or is deleted.
    pub fn id_to_position(&self, id: &SequenceId) -> Option<usize> {
        let &idx = self.index.get(id)?;
        let atom = &self.atoms[idx];
        if atom.deleted {
            return None;
        }
        Some(self.atoms[..idx].iter().filter(|a| !a.deleted).count())
    }

    /// Insert a character at a visible position (convenience wrapper).
    ///
    /// Position 0 = before first char, position `len()` = after last char.
    pub fn insert_at(&mut self, position: usize, value: char) -> SequenceOp {
        if position == 0 {
            self.insert_after(None, value)
        } else {
            let after_id = self.position_to_id(position - 1).cloned();
            self.insert_after(after_id.as_ref(), value)
        }
    }

    /// Delete a character at a visible position (convenience wrapper).
    ///
    /// Returns `None` if position is out of bounds.
    pub fn delete_at(&mut self, position: usize) -> Option<SequenceOp> {
        let id = self.position_to_id(position)?.clone();
        self.delete(&id)
    }

    /// Total atoms (including tombstones).
    pub fn atom_count(&self) -> usize {
        self.atoms.len()
    }

    /// Get the raw atom index for a [`SequenceId`].
    ///
    /// Returns the position in the internal atom list (including tombstones).
    /// Used by [`FormatMap`](super::formatting::FormatMap) for range containment checks.
    pub fn atom_index(&self, id: &SequenceId) -> Option<usize> {
        self.index.get(id).copied()
    }

    /// Iterate over visible (non-deleted) atoms in document order.
    ///
    /// Yields `(SequenceId, char)` pairs. Used by
    /// [`FormatMap::to_spans`](super::formatting::FormatMap::to_spans) to walk the document.
    pub fn visible_atoms(&self) -> impl Iterator<Item = (&SequenceId, char)> {
        self.atoms
            .iter()
            .filter(|a| !a.deleted)
            .map(|a| (&a.id, a.value))
    }

    // -----------------------------------------------------------------------
    // Internal
    // -----------------------------------------------------------------------

    /// Core insert logic shared by local and remote operations.
    fn apply_insert(&mut self, id: &SequenceId, value: char, parent: Option<SequenceId>) {
        // Already present (idempotency guard).
        if self.index.contains_key(id) {
            return;
        }

        // Find the insertion point in the atoms vec.
        let insert_pos = match &parent {
            None => {
                // Insert at beginning -- but scan past any concurrent head inserts.
                self.scan_concurrent(0, &None, id)
            }
            Some(parent_id) => {
                let parent_idx = match self.index.get(parent_id) {
                    Some(&idx) => idx,
                    // Parent not yet present (out-of-order delivery). Append at end.
                    None => self.atoms.len(),
                };
                let start = parent_idx + 1;
                self.scan_concurrent(start, &Some(parent_id.clone()), id)
            }
        };

        let atom = SequenceAtom {
            id: id.clone(),
            value,
            deleted: false,
            parent,
        };

        self.atoms.insert(insert_pos, atom);

        // Rebuild index from insert_pos onward (shifted entries).
        for i in insert_pos..self.atoms.len() {
            self.index.insert(self.atoms[i].id.clone(), i);
        }
    }

    /// Starting at `start`, scan forward to find where a new atom with `new_id`
    /// (inserted after `parent`) should be placed.
    ///
    /// The key insight: when we encounter a sibling (same parent) with a greater ID,
    /// we must skip past that sibling AND its entire causal subtree (all descendants
    /// that were inserted after it). Only when we find a sibling with a lesser ID,
    /// or a non-sibling that isn't a descendant of a skipped sibling, do we stop.
    fn scan_concurrent(
        &self,
        start: usize,
        parent: &Option<SequenceId>,
        new_id: &SequenceId,
    ) -> usize {
        let mut pos = start;
        while pos < self.atoms.len() {
            let existing = &self.atoms[pos];

            if existing.parent == *parent {
                // This is a direct sibling (same parent).
                // Higher IDs go first. If existing > new, skip past it and its subtree.
                if existing.id.cmp_tiebreak(new_id) == std::cmp::Ordering::Greater {
                    pos += 1;
                } else {
                    // Found a sibling with lesser/equal ID — this is our spot.
                    break;
                }
            } else if self.is_descendant_of_any_between(existing, start, pos, parent) {
                // This atom is a descendant of a sibling we already skipped past.
                // Keep scanning — we need to get past the entire subtree.
                pos += 1;
            } else {
                // Not a sibling and not a descendant of a skipped sibling. Stop here.
                break;
            }
        }
        pos
    }

    /// Check if `atom` is a causal descendant of any atom between `start` and `end`
    /// that has the given `parent`.
    fn is_descendant_of_any_between(
        &self,
        atom: &SequenceAtom,
        start: usize,
        end: usize,
        parent: &Option<SequenceId>,
    ) -> bool {
        // Walk up the atom's parent chain and see if we hit any sibling in [start..end].
        let mut current_parent = &atom.parent;
        while let Some(pid) = current_parent {
            if let Some(&idx) = self.index.get(pid) {
                if idx >= start && idx < end && self.atoms[idx].parent == *parent {
                    return true;
                }
                current_parent = &self.atoms[idx].parent;
            } else {
                break;
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sequence_is_empty() {
        let rga = SequenceRga::new("replica-a");
        assert!(rga.is_empty());
        assert_eq!(rga.len(), 0);
        assert_eq!(rga.text(), "");
        assert_eq!(rga.atom_count(), 0);
    }

    #[test]
    fn insert_single_char() {
        let mut rga = SequenceRga::new("replica-a");
        rga.insert_after(None, 'a');
        assert_eq!(rga.text(), "a");
        assert_eq!(rga.len(), 1);
        assert_eq!(rga.atom_count(), 1);
    }

    #[test]
    fn insert_multiple_chars_in_order() {
        let mut rga = SequenceRga::new("replica-a");
        let op_a = rga.insert_after(None, 'a');
        let id_a = match &op_a {
            SequenceOp::Insert { id, .. } => id.clone(),
            _ => panic!("expected insert"),
        };
        let op_b = rga.insert_after(Some(&id_a), 'b');
        let id_b = match &op_b {
            SequenceOp::Insert { id, .. } => id.clone(),
            _ => panic!("expected insert"),
        };
        rga.insert_after(Some(&id_b), 'c');
        assert_eq!(rga.text(), "abc");
    }

    #[test]
    fn insert_at_beginning() {
        let mut rga = SequenceRga::new("replica-a");
        rga.insert_after(None, 'b');
        rga.insert_after(None, 'a');
        // 'a' was inserted at the beginning after 'b' was already there.
        // Both have parent=None. 'a' has seq=2, 'b' has seq=1.
        // Higher IDs go first in scan_concurrent, so seq=2 ('a') comes before seq=1 ('b').
        assert_eq!(rga.text(), "ab");
    }

    #[test]
    fn insert_in_middle() {
        let mut rga = SequenceRga::new("replica-a");
        let op_a = rga.insert_after(None, 'a');
        let id_a = match &op_a {
            SequenceOp::Insert { id, .. } => id.clone(),
            _ => panic!("expected insert"),
        };
        let op_c = rga.insert_after(Some(&id_a), 'c');
        let _id_c = match &op_c {
            SequenceOp::Insert { id, .. } => id.clone(),
            _ => panic!("expected insert"),
        };
        // Insert 'b' between 'a' and 'c'
        rga.insert_after(Some(&id_a), 'b');
        // 'b' has seq=3, 'c' has seq=2. Both parent=id_a.
        // Higher IDs first: seq=3 ('b') before seq=2 ('c').
        assert_eq!(rga.text(), "abc");
    }

    #[test]
    fn delete_char() {
        let mut rga = SequenceRga::new("replica-a");
        let op = rga.insert_after(None, 'x');
        let id = match &op {
            SequenceOp::Insert { id, .. } => id.clone(),
            _ => panic!("expected insert"),
        };
        let del = rga.delete(&id);
        assert!(del.is_some());
        assert_eq!(rga.text(), "");
        assert_eq!(rga.len(), 0);
        // Tombstone still exists
        assert_eq!(rga.atom_count(), 1);
    }

    #[test]
    fn delete_already_deleted_returns_none() {
        let mut rga = SequenceRga::new("replica-a");
        let op = rga.insert_after(None, 'x');
        let id = match &op {
            SequenceOp::Insert { id, .. } => id.clone(),
            _ => panic!("expected insert"),
        };
        rga.delete(&id);
        assert!(rga.delete(&id).is_none());
    }

    #[test]
    fn delete_nonexistent_returns_none() {
        let mut rga = SequenceRga::new("replica-a");
        let fake_id = SequenceId {
            replica_id: "ghost".into(),
            seq: 999,
        };
        assert!(rga.delete(&fake_id).is_none());
    }

    #[test]
    fn text_excludes_deleted() {
        let mut rga = SequenceRga::new("replica-a");
        let op_a = rga.insert_after(None, 'a');
        let id_a = match &op_a {
            SequenceOp::Insert { id, .. } => id.clone(),
            _ => panic!("expected insert"),
        };
        let op_b = rga.insert_after(Some(&id_a), 'b');
        let id_b = match &op_b {
            SequenceOp::Insert { id, .. } => id.clone(),
            _ => panic!("expected insert"),
        };
        rga.insert_after(Some(&id_b), 'c');
        rga.delete(&id_b);
        assert_eq!(rga.text(), "ac");
    }

    #[test]
    fn position_to_id_and_back() {
        let mut rga = SequenceRga::new("replica-a");
        rga.insert_at(0, 'h');
        rga.insert_at(1, 'i');
        assert_eq!(rga.text(), "hi");

        let id_0 = rga.position_to_id(0).unwrap().clone();
        let id_1 = rga.position_to_id(1).unwrap().clone();
        assert_eq!(rga.id_to_position(&id_0), Some(0));
        assert_eq!(rga.id_to_position(&id_1), Some(1));
    }

    #[test]
    fn position_out_of_bounds() {
        let mut rga = SequenceRga::new("replica-a");
        rga.insert_at(0, 'x');
        assert!(rga.position_to_id(1).is_none());
        assert!(rga.position_to_id(100).is_none());
    }

    #[test]
    fn apply_remote_insert() {
        let mut rga_a = SequenceRga::new("replica-a");
        let mut rga_b = SequenceRga::new("replica-b");

        let op = rga_a.insert_after(None, 'z');
        assert!(rga_b.apply(&op), "first apply should return true");
        assert_eq!(rga_b.text(), "z");
    }

    #[test]
    fn apply_duplicate_is_idempotent() {
        let mut rga_a = SequenceRga::new("replica-a");
        let mut rga_b = SequenceRga::new("replica-b");

        let op = rga_a.insert_after(None, 'z');
        assert!(rga_b.apply(&op));
        assert!(!rga_b.apply(&op), "duplicate apply should return false");
        assert_eq!(rga_b.text(), "z");
    }

    #[test]
    fn concurrent_insert_same_position() {
        // Two replicas insert after the same char concurrently.
        let mut rga_a = SequenceRga::new("replica-a");
        let mut rga_b = SequenceRga::new("replica-b");

        // Both start with 'x'
        let op_x = rga_a.insert_after(None, 'x');
        rga_b.apply(&op_x);
        let id_x = match &op_x {
            SequenceOp::Insert { id, .. } => id.clone(),
            _ => panic!("expected insert"),
        };

        // replica-a inserts 'a' after 'x'
        let op_a = rga_a.insert_after(Some(&id_x), 'a');
        // replica-b inserts 'b' after 'x'
        let op_b = rga_b.insert_after(Some(&id_x), 'b');

        // Cross-apply
        rga_a.apply(&op_b);
        rga_b.apply(&op_a);

        // Both must converge to the same text.
        assert_eq!(
            rga_a.text(),
            rga_b.text(),
            "replicas must converge: a={}, b={}",
            rga_a.text(),
            rga_b.text()
        );
    }

    #[test]
    fn concurrent_insert_convergence() {
        // More thorough convergence test: each replica inserts multiple chars
        // at the same position, then cross-apply all ops.
        let mut rga_a = SequenceRga::new("replica-a");
        let mut rga_b = SequenceRga::new("replica-b");

        // Shared root
        let op_root = rga_a.insert_after(None, '.');
        rga_b.apply(&op_root);
        let root_id = match &op_root {
            SequenceOp::Insert { id, .. } => id.clone(),
            _ => panic!("expected insert"),
        };

        // replica-a types "AB" after root
        let op_a1 = rga_a.insert_after(Some(&root_id), 'A');
        let id_a1 = match &op_a1 {
            SequenceOp::Insert { id, .. } => id.clone(),
            _ => panic!("expected insert"),
        };
        let op_a2 = rga_a.insert_after(Some(&id_a1), 'B');

        // replica-b types "12" after root (concurrently)
        let op_b1 = rga_b.insert_after(Some(&root_id), '1');
        let id_b1 = match &op_b1 {
            SequenceOp::Insert { id, .. } => id.clone(),
            _ => panic!("expected insert"),
        };
        let op_b2 = rga_b.insert_after(Some(&id_b1), '2');

        // Cross-apply all ops
        for op in [&op_b1, &op_b2] {
            rga_a.apply(op);
        }
        for op in [&op_a1, &op_a2] {
            rga_b.apply(op);
        }

        assert_eq!(
            rga_a.text(),
            rga_b.text(),
            "replicas must converge: a={}, b={}",
            rga_a.text(),
            rga_b.text()
        );
        // Both should contain all 5 chars
        assert_eq!(rga_a.len(), 5);
    }

    #[test]
    fn interleaved_typing() {
        // Simulate two users typing alternating characters into a shared doc.
        let mut rga_a = SequenceRga::new("replica-a");
        let mut rga_b = SequenceRga::new("replica-b");

        // User A types 'H'
        let op1 = rga_a.insert_at(0, 'H');
        rga_b.apply(&op1);

        // User B types 'e' at position 1
        let op2 = rga_b.insert_at(1, 'e');
        rga_a.apply(&op2);

        // User A types 'l' at position 2
        let op3 = rga_a.insert_at(2, 'l');
        rga_b.apply(&op3);

        // User B types 'l' at position 3
        let op4 = rga_b.insert_at(3, 'l');
        rga_a.apply(&op4);

        // User A types 'o' at position 4
        let op5 = rga_a.insert_at(4, 'o');
        rga_b.apply(&op5);

        assert_eq!(rga_a.text(), "Hello");
        assert_eq!(rga_b.text(), "Hello");
    }

    #[test]
    fn insert_at_convenience() {
        let mut rga = SequenceRga::new("replica-a");
        rga.insert_at(0, 'c');
        rga.insert_at(0, 'b');
        rga.insert_at(0, 'a');
        rga.insert_at(3, 'd');
        assert_eq!(rga.text(), "abcd");
    }

    #[test]
    fn delete_at_convenience() {
        let mut rga = SequenceRga::new("replica-a");
        rga.insert_at(0, 'a');
        rga.insert_at(1, 'b');
        rga.insert_at(2, 'c');
        let del = rga.delete_at(1);
        assert!(del.is_some());
        assert_eq!(rga.text(), "ac");
        // Out of bounds
        assert!(rga.delete_at(99).is_none());
    }

    #[test]
    fn serde_round_trip() {
        let op = SequenceOp::Insert {
            id: SequenceId {
                replica_id: "replica-a".into(),
                seq: 42,
            },
            value: 'Z',
            after: Some(SequenceId {
                replica_id: "replica-b".into(),
                seq: 7,
            }),
        };
        let json = serde_json::to_string(&op).unwrap();
        let deserialized: SequenceOp = serde_json::from_str(&json).unwrap();
        assert_eq!(op, deserialized);

        let del_op = SequenceOp::Delete {
            id: SequenceId {
                replica_id: "replica-a".into(),
                seq: 1,
            },
        };
        let json2 = serde_json::to_string(&del_op).unwrap();
        let deserialized2: SequenceOp = serde_json::from_str(&json2).unwrap();
        assert_eq!(del_op, deserialized2);
    }

    #[test]
    fn large_document() {
        let mut rga = SequenceRga::new("replica-a");
        let expected: String = (0..1000).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
        for (i, ch) in expected.chars().enumerate() {
            rga.insert_at(i, ch);
        }
        assert_eq!(rga.text(), expected);
        assert_eq!(rga.len(), 1000);
        assert_eq!(rga.atom_count(), 1000);
    }

    #[test]
    fn delete_via_apply() {
        let mut rga_a = SequenceRga::new("replica-a");
        let mut rga_b = SequenceRga::new("replica-b");

        let op_ins = rga_a.insert_after(None, 'q');
        rga_b.apply(&op_ins);

        let id = match &op_ins {
            SequenceOp::Insert { id, .. } => id.clone(),
            _ => panic!("expected insert"),
        };
        let del_op = rga_a.delete(&id).unwrap();
        assert!(rga_b.apply(&del_op));
        assert_eq!(rga_b.text(), "");
        // Applying same delete again is idempotent
        assert!(!rga_b.apply(&del_op));
    }

    #[test]
    fn id_to_position_deleted_returns_none() {
        let mut rga = SequenceRga::new("replica-a");
        let op = rga.insert_after(None, 'x');
        let id = match &op {
            SequenceOp::Insert { id, .. } => id.clone(),
            _ => panic!("expected insert"),
        };
        rga.delete(&id);
        assert_eq!(rga.id_to_position(&id), None);
    }
}
