use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use super::note::{CashNote, CashStatus};

/// Tracks all Cash notes in the system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashRegistry {
    notes: HashMap<String, CashNote>,
}

impl CashRegistry {
    /// Create an empty registry with no notes.
    pub fn new() -> Self {
        Self {
            notes: HashMap::new(),
        }
    }

    /// Register a new Cash note in the registry.
    pub fn register(&mut self, note: CashNote) {
        self.notes.insert(note.serial.clone(), note);
    }

    /// Look up a note by its serial number.
    pub fn note(&self, serial: &str) -> Option<&CashNote> {
        self.notes.get(serial)
    }

    /// Look up a note mutably by its serial number.
    pub fn note_mut(&mut self, serial: &str) -> Option<&mut CashNote> {
        self.notes.get_mut(serial)
    }

    /// Mark a note as redeemed by the given person.
    pub fn mark_redeemed(&mut self, serial: &str, redeemer: &str) {
        if let Some(note) = self.notes.get_mut(serial) {
            note.status = CashStatus::Redeemed;
            note.redeemer = Some(redeemer.into());
            note.redeemed_at = Some(chrono::Utc::now());
        }
    }

    /// Mark a note as expired (past its expiry date).
    pub fn mark_expired(&mut self, serial: &str) {
        if let Some(note) = self.notes.get_mut(serial) {
            note.status = CashStatus::Expired;
        }
    }

    /// Mark a note as revoked by the issuer, with a reason.
    pub fn mark_revoked(&mut self, serial: &str, reason: &str) {
        if let Some(note) = self.notes.get_mut(serial) {
            note.status = CashStatus::Revoked;
            note.revocation_reason = Some(reason.into());
        }
    }

    /// Get all active notes.
    pub fn active_notes(&self) -> Vec<&CashNote> {
        self.notes.values().filter(|n| n.is_active()).collect()
    }

    /// Get notes issued by a specific person.
    pub fn notes_by_issuer(&self, issuer: &str) -> Vec<&CashNote> {
        self.notes.values().filter(|n| n.issuer == issuer).collect()
    }

    /// Find expired-but-still-active notes (need processing).
    pub fn expired_unprocessed(&self) -> Vec<&CashNote> {
        self.notes.values().filter(|n| n.is_expired()).collect()
    }

    /// Total number of notes ever registered (all statuses).
    pub fn total_notes(&self) -> usize {
        self.notes.len()
    }

    /// Number of currently active (unredeemed, unexpired) notes.
    pub fn active_count(&self) -> usize {
        self.active_notes().len()
    }
}

impl Default for CashRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::note::CashNote;
    use chrono::Utc;

    fn test_note(serial: &str, issuer: &str, amount: i64) -> CashNote {
        CashNote {
            serial: serial.into(),
            amount,
            issuer: issuer.into(),
            issued_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::days(365),
            memo: None,
            status: CashStatus::Active,
            redeemer: None,
            redeemed_at: None,
            revocation_reason: None,
        }
    }

    #[test]
    fn register_and_lookup() {
        let mut reg = CashRegistry::new();
        reg.register(test_note("A-B-C", "alice", 100));
        assert!(reg.note("A-B-C").is_some());
        assert!(reg.note("X-Y-Z").is_none());
    }

    #[test]
    fn mark_redeemed() {
        let mut reg = CashRegistry::new();
        reg.register(test_note("A-B-C", "alice", 50));
        reg.mark_redeemed("A-B-C", "bob");
        let note = reg.note("A-B-C").unwrap();
        assert_eq!(note.status, CashStatus::Redeemed);
        assert_eq!(note.redeemer.as_deref(), Some("bob"));
    }

    #[test]
    fn active_notes_filter() {
        let mut reg = CashRegistry::new();
        reg.register(test_note("A1", "alice", 10));
        reg.register(test_note("A2", "alice", 20));
        reg.mark_redeemed("A1", "bob");

        assert_eq!(reg.active_count(), 1);
        assert_eq!(reg.total_notes(), 2);
    }

    #[test]
    fn notes_by_issuer() {
        let mut reg = CashRegistry::new();
        reg.register(test_note("A1", "alice", 10));
        reg.register(test_note("A2", "alice", 20));
        reg.register(test_note("B1", "bob", 30));

        assert_eq!(reg.notes_by_issuer("alice").len(), 2);
        assert_eq!(reg.notes_by_issuer("bob").len(), 1);
    }
}
