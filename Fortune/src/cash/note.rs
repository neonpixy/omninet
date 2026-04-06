use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A bearer cash note — whoever has the serial can redeem it.
///
/// From Consortium Art. 2 §2: "Fair compensation, consent-based agreements,
/// and transparency in value flows shall be required."
///
/// Cash notes are backed 1:1 by locked Cool in the issuer's balance.
/// They can be printed, shared physically, or transmitted digitally.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CashNote {
    pub serial: String,
    pub amount: i64,
    pub issuer: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub memo: Option<String>,
    pub status: CashStatus,
    pub redeemer: Option<String>,
    pub redeemed_at: Option<DateTime<Utc>>,
    pub revocation_reason: Option<String>,
}

impl CashNote {
    /// Whether this note is still active and within its expiry window.
    pub fn is_active(&self) -> bool {
        self.status == CashStatus::Active && Utc::now() < self.expires_at
    }

    /// Whether this note has passed its expiry date but hasn't been processed yet.
    pub fn is_expired(&self) -> bool {
        self.status == CashStatus::Active && Utc::now() >= self.expires_at
    }
}

/// Lifecycle of a cash note.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CashStatus {
    /// Note is live and can be redeemed.
    Active,
    /// Note has been redeemed by the holder.
    Redeemed,
    /// Note passed its expiry date — Cool returned to issuer.
    Expired,
    /// Note was revoked by the issuer before use.
    Revoked,
}

/// Generate a XXXX-XXXX-XXXX serial using the 31-character unambiguous alphabet.
///
/// Excludes: 0, O, I, l, 1 (visually confusing characters).
pub fn generate_serial() -> String {
    const ALPHABET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    let mut bytes = [0u8; 12];
    getrandom::fill(&mut bytes).expect("RNG failure is unrecoverable in financial context");

    let mut serial = String::with_capacity(14);
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 && i % 4 == 0 {
            serial.push('-');
        }
        serial.push(ALPHABET[(*byte as usize) % ALPHABET.len()] as char);
    }
    serial
}

/// Validate a serial matches the XXXX-XXXX-XXXX format.
pub fn validate_serial(serial: &str) -> bool {
    if serial.len() != 14 {
        return false;
    }
    let parts: Vec<&str> = serial.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    const VALID: &str = "23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    parts.iter().all(|part| {
        part.len() == 4 && part.chars().all(|c| VALID.contains(c))
    })
}

/// Normalize input to standard serial format (uppercase, add dashes).
pub fn normalize_serial(input: &str) -> String {
    let cleaned: String = input
        .to_uppercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect();

    if cleaned.len() == 12 {
        format!("{}-{}-{}", &cleaned[0..4], &cleaned[4..8], &cleaned[8..12])
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_serial_format() {
        let serial = generate_serial();
        assert_eq!(serial.len(), 14);
        assert!(validate_serial(&serial), "invalid serial: {serial}");
    }

    #[test]
    fn generate_unique_serials() {
        let a = generate_serial();
        let b = generate_serial();
        assert_ne!(a, b);
    }

    #[test]
    fn validate_serial_good() {
        assert!(validate_serial("A2B3-C4D5-E6F7"));
        assert!(validate_serial("XXXX-YYYY-ZZZZ"));
        assert!(validate_serial("2345-6789-ABCD"));
    }

    #[test]
    fn validate_serial_bad() {
        assert!(!validate_serial("INVALID"));
        assert!(!validate_serial("A2B3-C4D5"));
        assert!(!validate_serial("A2B3-C4D5-E6F7-H8J9"));
        assert!(!validate_serial("0000-OOOO-1111")); // contains excluded chars
    }

    #[test]
    fn normalize_serial_adds_dashes() {
        assert_eq!(normalize_serial("A2B3C4D5E6F7"), "A2B3-C4D5-E6F7");
    }

    #[test]
    fn normalize_serial_uppercases() {
        assert_eq!(normalize_serial("a2b3-c4d5-e6f7"), "A2B3-C4D5-E6F7");
    }

    #[test]
    fn cash_note_expiry_check() {
        let note = CashNote {
            serial: "TEST-TEST-TEST".into(),
            amount: 100,
            issuer: "alice".into(),
            issued_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::days(365),
            memo: None,
            status: CashStatus::Active,
            redeemer: None,
            redeemed_at: None,
            revocation_reason: None,
        };
        assert!(note.is_active());
        assert!(!note.is_expired());
    }

    #[test]
    fn cash_note_already_expired() {
        let note = CashNote {
            serial: "TEST-TEST-TEST".into(),
            amount: 100,
            issuer: "alice".into(),
            issued_at: Utc::now() - chrono::Duration::days(400),
            expires_at: Utc::now() - chrono::Duration::days(35),
            memo: None,
            status: CashStatus::Active,
            redeemer: None,
            redeemed_at: None,
            revocation_reason: None,
        };
        assert!(!note.is_active());
        assert!(note.is_expired());
    }
}
