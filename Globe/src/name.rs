use chrono::{DateTime, TimeZone, Utc};
use crown::CrownKeypair;
use serde::{Deserialize, Serialize};

use crate::error::GlobeError;
use crate::event::OmniEvent;
use crate::event_builder::{EventBuilder, UnsignedEvent};
use crate::filter::OmniFilter;
use crate::gospel::config::NamePolicy;
use crate::kind;

/// A parsed domain name record from a name event.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct NameRecord {
    /// The domain name (e.g., "sam.com").
    pub name: String,
    /// Owner's public key hex.
    pub owner: String,
    /// Optional target pubkey (if name points to a different key).
    pub target: Option<String>,
    /// Optional description.
    pub description: Option<String>,
    /// When this record was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Parsed components of a domain name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameParts {
    /// The top-level domain (e.g., "com").
    pub tld: String,
    /// The domain name (e.g., "sam").
    pub domain: String,
    /// Optional subdomain (e.g., "shop" in "shop.sam.com").
    pub subdomain: Option<String>,
}

impl std::fmt::Display for NameParts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(sub) = &self.subdomain {
            write!(f, "{}.{}.{}", sub, self.domain, self.tld)
        } else {
            write!(f, "{}.{}", self.domain, self.tld)
        }
    }
}

/// Parse a domain name string into its components.
///
/// Supports: `"sam.com"`, `"shop.sam.com"`.
/// Rejects: empty strings, single-part names, names with 4+ parts.
pub fn parse_name(name: &str) -> Result<NameParts, GlobeError> {
    let parts: Vec<&str> = name.split('.').collect();
    match parts.len() {
        2 => Ok(NameParts {
            tld: parts[1].to_string(),
            domain: parts[0].to_string(),
            subdomain: None,
        }),
        3 => Ok(NameParts {
            tld: parts[2].to_string(),
            domain: parts[1].to_string(),
            subdomain: Some(parts[0].to_string()),
        }),
        _ => Err(GlobeError::InvalidConfig(format!(
            "invalid domain name format: '{name}' (expected domain.tld or sub.domain.tld)"
        ))),
    }
}

/// Build an OmniFilter to resolve a domain name.
pub fn resolve_filter(name: &str) -> OmniFilter {
    OmniFilter::for_name(name)
}

/// Parse a NameRecord from a name claim event.
pub fn parse_name_record(event: &OmniEvent) -> Result<NameRecord, GlobeError> {
    let name = event
        .d_tag()
        .ok_or_else(|| GlobeError::InvalidMessage("name event missing d-tag".into()))?
        .to_string();

    let target = event.tag_value("target").map(|s| s.to_string());

    let description = if event.content.is_empty() {
        None
    } else {
        // Try to parse content as JSON and extract description.
        serde_json::from_str::<serde_json::Value>(&event.content)
            .ok()
            .and_then(|v| v.get("description")?.as_str().map(|s| s.to_string()))
    };

    let updated_at = Utc
        .timestamp_opt(event.created_at, 0)
        .single()
        .unwrap_or_else(Utc::now);

    Ok(NameRecord {
        name,
        owner: event.author.clone(),
        target,
        description,
        updated_at,
    })
}

/// Builds signed name events for the Globe naming system.
pub struct NameBuilder;

impl NameBuilder {
    /// Claim a domain name (kind 7000).
    pub fn claim(name: &str, keypair: &CrownKeypair) -> Result<OmniEvent, GlobeError> {
        parse_name(name)?; // Validate format.
        let unsigned = UnsignedEvent::new(kind::NAME_CLAIM, "")
            .with_d_tag(name)
            .with_tag("target", &[&keypair.public_key_hex()]);
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Update a name's target pubkey (kind 7001).
    pub fn update(
        name: &str,
        target_pubkey: &str,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        parse_name(name)?;
        let unsigned = UnsignedEvent::new(kind::NAME_UPDATE, "")
            .with_d_tag(name)
            .with_tag("target", &[target_pubkey]);
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Transfer name ownership to a new pubkey (kind 7002).
    pub fn transfer(
        name: &str,
        new_owner_pubkey: &str,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        parse_name(name)?;
        let unsigned = UnsignedEvent::new(kind::NAME_TRANSFER, "")
            .with_d_tag(name)
            .with_tag("target", &[new_owner_pubkey]);
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Delegate a subdomain to another pubkey (kind 7003).
    pub fn delegate_subdomain(
        parent: &str,
        subdomain: &str,
        delegate_pubkey: &str,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        parse_name(parent)?;
        let full_name = format!("{subdomain}.{parent}");
        let unsigned = UnsignedEvent::new(kind::NAME_DELEGATE, "")
            .with_d_tag(parent)
            .with_tag("subdomain", &[&full_name])
            .with_tag("delegate", &[delegate_pubkey]);
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Revoke a name (kind 7004).
    pub fn revoke(name: &str, keypair: &CrownKeypair) -> Result<OmniEvent, GlobeError> {
        parse_name(name)?;
        let unsigned = UnsignedEvent::new(kind::NAME_REVOKE, "").with_d_tag(name);
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Renew a name registration (kind 7005).
    ///
    /// Extends the name's expiration by another TTL period. Only the
    /// current owner can renew. Produces a NAME_RENEWAL event with
    /// d-tag = name.
    pub fn renew(name: &str, keypair: &CrownKeypair) -> Result<OmniEvent, GlobeError> {
        parse_name(name)?;
        let unsigned = UnsignedEvent::new(kind::NAME_RENEWAL, "").with_d_tag(name);
        EventBuilder::sign(&unsigned, keypair)
    }
}

/// A parsed payment proof from a name event's tags.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentProof {
    /// The Cool transaction ID.
    pub transaction_id: String,
    /// The Cool amount paid.
    pub amount: u64,
    /// The payer's Crown public key hex.
    pub payer: String,
}

/// Check whether a name event carries a valid payment proof tag.
///
/// A valid payment tag has the form:
/// `["payment", transaction_id, amount, payer]`
///
/// Validates that amount parses to a positive u64 and that payer
/// matches the event's author.
pub fn has_payment_proof(event: &OmniEvent) -> bool {
    event.tags.iter().any(|tag| {
        if tag.len() < 4 {
            return false;
        }
        if tag[0] != "payment" {
            return false;
        }
        // Amount must parse to a positive u64.
        let amount_ok = tag[2].parse::<u64>().is_ok_and(|a| a > 0);
        // Payer must match the event author.
        let payer_ok = tag[3] == event.author;
        amount_ok && payer_ok
    })
}

/// Parse the payment proof from a name event's tags.
///
/// Returns `None` if no valid payment tag is found.
pub fn payment_proof(event: &OmniEvent) -> Option<PaymentProof> {
    for tag in &event.tags {
        if tag.len() < 4 || tag[0] != "payment" {
            continue;
        }
        let amount = match tag[2].parse::<u64>() {
            Ok(a) if a > 0 => a,
            _ => continue,
        };
        if tag[3] != event.author {
            continue;
        }
        return Some(PaymentProof {
            transaction_id: tag[1].clone(),
            amount,
            payer: tag[3].clone(),
        });
    }
    None
}

/// Build a filter closure that enforces anti-squatting policy on name events.
///
/// Non-name events pass through unconditionally. Name events are checked for:
/// - Timestamp window: `|created_at - now| <= policy.timestamp_window_secs`
/// - Payment proof (if `policy.require_payment`): valid tag with amount >= min
///
/// Returns a closure suitable for use as a relay-side event filter.
pub fn name_event_filter(policy: &NamePolicy) -> impl Fn(&OmniEvent) -> bool {
    let require_payment = policy.require_payment;
    let min_amount = policy.min_payment_amount;
    let window = policy.timestamp_window_secs;

    move |event: &OmniEvent| -> bool {
        if !kind::is_name_kind(event.kind) {
            return true;
        }

        // Timestamp window check.
        let now = Utc::now().timestamp();
        let drift = (event.created_at - now).abs();
        if drift > window {
            return false;
        }

        // Payment proof check (only if required).
        if require_payment {
            if !has_payment_proof(event) {
                return false;
            }
            // Verify amount meets minimum.
            if let Some(proof) = payment_proof(event) {
                if proof.amount < min_amount {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_name_two_parts() {
        let parts = parse_name("sam.com").unwrap();
        assert_eq!(parts.domain, "sam");
        assert_eq!(parts.tld, "com");
        assert!(parts.subdomain.is_none());
        assert_eq!(parts.to_string(), "sam.com");
    }

    #[test]
    fn parse_name_three_parts() {
        let parts = parse_name("shop.sam.com").unwrap();
        assert_eq!(parts.domain, "sam");
        assert_eq!(parts.tld, "com");
        assert_eq!(parts.subdomain, Some("shop".into()));
        assert_eq!(parts.to_string(), "shop.sam.com");
    }

    #[test]
    fn parse_name_invalid() {
        assert!(parse_name("sam").is_err());
        assert!(parse_name("a.b.c.d").is_err());
        assert!(parse_name("").is_err());
    }

    #[test]
    fn claim_and_parse_round_trip() {
        let kp = CrownKeypair::generate();
        let event = NameBuilder::claim("sam.com", &kp).unwrap();

        assert_eq!(event.kind, kind::NAME_CLAIM);
        assert_eq!(event.d_tag(), Some("sam.com"));

        let record = parse_name_record(&event).unwrap();
        assert_eq!(record.name, "sam.com");
        assert_eq!(record.owner, kp.public_key_hex());
        assert_eq!(record.target, Some(kp.public_key_hex()));
    }

    #[test]
    fn transfer_event() {
        let owner = CrownKeypair::generate();
        let new_owner = CrownKeypair::generate();

        let event =
            NameBuilder::transfer("sam.com", &new_owner.public_key_hex(), &owner).unwrap();
        assert_eq!(event.kind, kind::NAME_TRANSFER);
        assert_eq!(event.d_tag(), Some("sam.com"));
        assert!(event.has_tag("target", &new_owner.public_key_hex()));
    }

    #[test]
    fn delegate_subdomain_event() {
        let kp = CrownKeypair::generate();
        let delegate = CrownKeypair::generate();

        let event = NameBuilder::delegate_subdomain(
            "sam.com",
            "shop",
            &delegate.public_key_hex(),
            &kp,
        )
        .unwrap();

        assert_eq!(event.kind, kind::NAME_DELEGATE);
        assert_eq!(event.d_tag(), Some("sam.com"));
        assert!(event.has_tag("subdomain", "shop.sam.com"));
    }

    #[test]
    fn revoke_event() {
        let kp = CrownKeypair::generate();
        let event = NameBuilder::revoke("sam.com", &kp).unwrap();
        assert_eq!(event.kind, kind::NAME_REVOKE);
        assert_eq!(event.d_tag(), Some("sam.com"));
    }

    #[test]
    fn resolve_filter_creates_correct_filter() {
        let filter = resolve_filter("sam.com");
        assert_eq!(filter.kinds, Some(vec![kind::NAME_CLAIM]));
        assert_eq!(
            filter.tag_filters.get(&'d'),
            Some(&vec!["sam.com".to_string()])
        );
    }

    #[test]
    fn name_record_serde_round_trip() {
        let record = NameRecord {
            name: "sam.com".into(),
            owner: "a".repeat(64),
            target: Some("b".repeat(64)),
            description: Some("My domain".into()),
            updated_at: Utc::now(),
        };
        let json = serde_json::to_string(&record).unwrap();
        let loaded: NameRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(record.name, loaded.name);
        assert_eq!(record.owner, loaded.owner);
    }

    #[test]
    fn renew_event() {
        let kp = CrownKeypair::generate();
        let event = NameBuilder::renew("sam.com", &kp).unwrap();
        assert_eq!(event.kind, kind::NAME_RENEWAL);
        assert_eq!(event.d_tag(), Some("sam.com"));
        assert_eq!(event.author, kp.public_key_hex());
    }

    #[test]
    fn renew_invalid_name_rejected() {
        let kp = CrownKeypair::generate();
        assert!(NameBuilder::renew("sam", &kp).is_err());
    }

    #[test]
    fn has_payment_proof_valid() {
        let author = "a".repeat(64);
        let event = OmniEvent {
            id: "test-id".into(),
            author: author.clone(),
            created_at: Utc::now().timestamp(),
            kind: kind::NAME_CLAIM,
            tags: vec![
                vec!["d".into(), "sam.com".into()],
                vec!["payment".into(), "tx123".into(), "200".into(), author],
            ],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(has_payment_proof(&event));
    }

    #[test]
    fn has_payment_proof_missing_tag() {
        let event = OmniEvent {
            id: "test-id".into(),
            author: "a".repeat(64),
            created_at: Utc::now().timestamp(),
            kind: kind::NAME_CLAIM,
            tags: vec![vec!["d".into(), "sam.com".into()]],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(!has_payment_proof(&event));
    }

    #[test]
    fn has_payment_proof_invalid_amount() {
        let author = "a".repeat(64);
        let event = OmniEvent {
            id: "test-id".into(),
            author: author.clone(),
            created_at: Utc::now().timestamp(),
            kind: kind::NAME_CLAIM,
            tags: vec![
                vec!["d".into(), "sam.com".into()],
                vec!["payment".into(), "tx123".into(), "zero".into(), author],
            ],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(!has_payment_proof(&event));
    }

    #[test]
    fn has_payment_proof_zero_amount() {
        let author = "a".repeat(64);
        let event = OmniEvent {
            id: "test-id".into(),
            author: author.clone(),
            created_at: Utc::now().timestamp(),
            kind: kind::NAME_CLAIM,
            tags: vec![
                vec!["d".into(), "sam.com".into()],
                vec!["payment".into(), "tx123".into(), "0".into(), author],
            ],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(!has_payment_proof(&event));
    }

    #[test]
    fn has_payment_proof_wrong_payer() {
        let event = OmniEvent {
            id: "test-id".into(),
            author: "a".repeat(64),
            created_at: Utc::now().timestamp(),
            kind: kind::NAME_CLAIM,
            tags: vec![
                vec!["d".into(), "sam.com".into()],
                vec![
                    "payment".into(),
                    "tx123".into(),
                    "200".into(),
                    "b".repeat(64),
                ],
            ],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(!has_payment_proof(&event));
    }

    #[test]
    fn has_payment_proof_too_few_values() {
        let author = "a".repeat(64);
        let event = OmniEvent {
            id: "test-id".into(),
            author: author.clone(),
            created_at: Utc::now().timestamp(),
            kind: kind::NAME_CLAIM,
            tags: vec![
                vec!["d".into(), "sam.com".into()],
                vec!["payment".into(), "tx123".into()], // Missing amount and payer.
            ],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(!has_payment_proof(&event));
    }

    #[test]
    fn payment_proof_parses_valid() {
        let author = "a".repeat(64);
        let event = OmniEvent {
            id: "test-id".into(),
            author: author.clone(),
            created_at: Utc::now().timestamp(),
            kind: kind::NAME_CLAIM,
            tags: vec![
                vec!["d".into(), "sam.com".into()],
                vec!["payment".into(), "tx456".into(), "500".into(), author.clone()],
            ],
            content: String::new(),
            sig: "c".repeat(128),
        };
        let proof = payment_proof(&event).unwrap();
        assert_eq!(proof.transaction_id, "tx456");
        assert_eq!(proof.amount, 500);
        assert_eq!(proof.payer, author);
    }

    #[test]
    fn payment_proof_returns_none_for_invalid() {
        let event = OmniEvent {
            id: "test-id".into(),
            author: "a".repeat(64),
            created_at: Utc::now().timestamp(),
            kind: kind::NAME_CLAIM,
            tags: vec![vec!["d".into(), "sam.com".into()]],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(payment_proof(&event).is_none());
    }

    #[test]
    fn name_event_filter_passes_non_name_events() {
        let policy = NamePolicy {
            require_payment: true,
            min_payment_amount: 100,
            timestamp_window_secs: 10,
            name_ttl_secs: 86_400,
        };
        let filter = name_event_filter(&policy);
        let event = OmniEvent {
            id: "test-id".into(),
            author: "a".repeat(64),
            created_at: 0, // Way outside window, but not a name event.
            kind: kind::TEXT_NOTE,
            tags: vec![],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(filter(&event));
    }

    #[test]
    fn name_event_filter_rejects_far_future_timestamp() {
        let policy = NamePolicy {
            require_payment: false,
            min_payment_amount: 100,
            timestamp_window_secs: 60,
            name_ttl_secs: 86_400,
        };
        let filter = name_event_filter(&policy);
        let event = OmniEvent {
            id: "test-id".into(),
            author: "a".repeat(64),
            created_at: Utc::now().timestamp() + 3600, // 1 hour in the future.
            kind: kind::NAME_CLAIM,
            tags: vec![vec!["d".into(), "sam.com".into()]],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(!filter(&event));
    }

    #[test]
    fn name_event_filter_accepts_within_window() {
        let policy = NamePolicy {
            require_payment: false,
            min_payment_amount: 100,
            timestamp_window_secs: 600,
            name_ttl_secs: 86_400,
        };
        let filter = name_event_filter(&policy);
        let event = OmniEvent {
            id: "test-id".into(),
            author: "a".repeat(64),
            created_at: Utc::now().timestamp(),
            kind: kind::NAME_CLAIM,
            tags: vec![vec!["d".into(), "sam.com".into()]],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(filter(&event));
    }

    #[test]
    fn name_event_filter_rejects_without_payment_when_required() {
        let policy = NamePolicy {
            require_payment: true,
            min_payment_amount: 100,
            timestamp_window_secs: 600,
            name_ttl_secs: 86_400,
        };
        let filter = name_event_filter(&policy);
        let event = OmniEvent {
            id: "test-id".into(),
            author: "a".repeat(64),
            created_at: Utc::now().timestamp(),
            kind: kind::NAME_CLAIM,
            tags: vec![vec!["d".into(), "sam.com".into()]],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(!filter(&event));
    }

    #[test]
    fn name_event_filter_rejects_insufficient_payment() {
        let author = "a".repeat(64);
        let policy = NamePolicy {
            require_payment: true,
            min_payment_amount: 200,
            timestamp_window_secs: 600,
            name_ttl_secs: 86_400,
        };
        let filter = name_event_filter(&policy);
        let event = OmniEvent {
            id: "test-id".into(),
            author: author.clone(),
            created_at: Utc::now().timestamp(),
            kind: kind::NAME_CLAIM,
            tags: vec![
                vec!["d".into(), "sam.com".into()],
                vec!["payment".into(), "tx1".into(), "100".into(), author],
            ],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(!filter(&event));
    }

    #[test]
    fn name_event_filter_accepts_with_sufficient_payment() {
        let author = "a".repeat(64);
        let policy = NamePolicy {
            require_payment: true,
            min_payment_amount: 100,
            timestamp_window_secs: 600,
            name_ttl_secs: 86_400,
        };
        let filter = name_event_filter(&policy);
        let event = OmniEvent {
            id: "test-id".into(),
            author: author.clone(),
            created_at: Utc::now().timestamp(),
            kind: kind::NAME_CLAIM,
            tags: vec![
                vec!["d".into(), "sam.com".into()],
                vec!["payment".into(), "tx1".into(), "200".into(), author],
            ],
            content: String::new(),
            sig: "c".repeat(128),
        };
        assert!(filter(&event));
    }

    #[test]
    fn payment_proof_serde_round_trip() {
        let proof = PaymentProof {
            transaction_id: "tx789".into(),
            amount: 1000,
            payer: "a".repeat(64),
        };
        let json = serde_json::to_string(&proof).unwrap();
        let loaded: PaymentProof = serde_json::from_str(&json).unwrap();
        assert_eq!(proof, loaded);
    }
}
