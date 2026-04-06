use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::IdeasError;

// ── Cool (Currency) ──

/// The economic value of an .idea file, denominated in cents cool.
///
/// The Covenant (founding document) = 1 Googolplex (10^10^100).
/// Everything else is priced as a fraction of that reserve.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cool {
    #[serde(default = "default_version")]
    pub version: String,
    pub cool: i64,
    pub initial_cool: i64,
    pub valuation_history: Vec<Valuation>,
    pub splits: Splits,
    pub last_valuation: DateTime<Utc>,
}

/// A single recorded valuation event in the idea's economic history.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Valuation {
    /// The new value in Cool cents.
    pub cool: i64,
    pub timestamp: DateTime<Utc>,
    pub reason: ValuationReason,
}

/// Why a valuation changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValuationReason {
    /// Set at creation.
    Initial,
    /// Sold for more than the asking price.
    TradeAboveAsk,
    /// Sold for less than the asking price.
    TradeBelowAsk,
    /// Received an endorsement, increasing perceived value.
    Endorsement,
    /// A derivative was created, increasing demand.
    DerivativeCreated,
    /// Natural market-driven decrease in value.
    MarketDepreciation,
    /// The creator manually adjusted the price.
    CreatorAdjustment,
}

/// How revenue from this idea is split between the creator and root ideas.
///
/// `self_percent` plus all root percentages must sum to exactly 100.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Splits {
    /// Percentage the creator keeps.
    pub self_percent: i32,
    /// Revenue shares owed to parent ideas.
    #[serde(default)]
    pub roots: Vec<RootSplit>,
}

/// A revenue share owed to a parent idea in the provenance tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RootSplit {
    /// The parent idea that receives this share.
    pub idea_id: Uuid,
    /// Percentage of revenue (1-100).
    pub percentage: i32,
}

impl Cool {
    /// Suggested Cool value for a major creative work (c1M).
    pub const MAJOR_WORK: i64 = 1_000_000;
    /// Suggested Cool value for typical content (c100K).
    pub const TYPICAL_WORK: i64 = 100_000;
    /// Suggested Cool value for a simple service (c50K).
    pub const SIMPLE_SERVICE: i64 = 50_000;
    /// Suggested Cool value for a small idea (c10K).
    pub const SMALL_IDEA: i64 = 10_000;
    /// Suggested Cool value for a micro idea (c1K).
    pub const MICRO_IDEA: i64 = 1_000;

    /// Creates a new Cool with the given initial value in cents.
    pub fn new(value: i64) -> Self {
        let now = Utc::now();
        Cool {
            version: "1.0".to_string(),
            cool: value,
            initial_cool: value,
            valuation_history: vec![Valuation {
                cool: value,
                timestamp: now,
                reason: ValuationReason::Initial,
            }],
            splits: Splits {
                self_percent: 100,
                roots: Vec::new(),
            },
            last_valuation: now,
        }
    }

    /// Returns a new Cool with an updated valuation recorded.
    pub fn with_valuation(&self, new_cool: i64, reason: ValuationReason) -> Self {
        let mut copy = self.clone();
        let now = Utc::now();
        copy.cool = new_cool;
        copy.valuation_history.push(Valuation {
            cool: new_cool,
            timestamp: now,
            reason,
        });
        copy.last_valuation = now;
        copy
    }

    /// Returns a new Cool with updated revenue splits.
    pub fn with_splits(&self, splits: Splits) -> Self {
        let mut copy = self.clone();
        copy.splits = splits;
        copy
    }

    /// Validates that revenue splits sum to exactly 100%.
    pub fn validate_splits(&self) -> Result<(), IdeasError> {
        self.splits.validate()
    }

    /// Formats the cool value with symbol (e.g., "c100K").
    pub fn formatted(&self) -> String {
        Self::format(self.cool)
    }

    /// Formats a Cool value with abbreviated suffix (e.g., `"c100K"`, `"c1M"`).
    pub fn format(value: i64) -> String {
        match value {
            v if v >= 1_000_000_000_000 => format!("c{}T", v / 1_000_000_000_000),
            v if v >= 1_000_000_000 => format!("c{}B", v / 1_000_000_000),
            v if v >= 1_000_000 => format!("c{}M", v / 1_000_000),
            v if v >= 1_000 => format!("c{}K", v / 1_000),
            v => format!("c{v}"),
        }
    }
}

impl Splits {
    /// Sum of all root split percentages.
    pub fn roots_percent(&self) -> i32 {
        self.roots.iter().map(|r| r.percentage).sum()
    }

    /// Validates that splits sum to 100% and all percentages are in range.
    pub fn validate(&self) -> Result<(), IdeasError> {
        let total = self.self_percent + self.roots_percent();
        if total != 100 {
            return Err(IdeasError::SplitsNotEqual100(total));
        }
        if !(0..=100).contains(&self.self_percent) {
            return Err(IdeasError::InvalidSplitPercentage(self.self_percent));
        }
        for root in &self.roots {
            if root.percentage <= 0 || root.percentage > 100 {
                return Err(IdeasError::InvalidSplitPercentage(root.percentage));
            }
        }
        Ok(())
    }
}

// ── Redemption (Service/Goods Fulfillment) ──

/// Tracks the fulfillment lifecycle for a redeemable idea (service or physical good).
///
/// Used when an .idea represents something that can be claimed in the real world
/// -- like a concert ticket, consulting session, or physical product.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Redemption {
    #[serde(default = "default_version")]
    pub version: String,
    pub redeemable: bool,
    #[serde(rename = "type")]
    pub redeemable_type: RedeemableType,
    pub status: RedemptionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redeemed_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redeemed_by: Option<String>,
    pub provider: Provider,
    pub terms: Terms,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_use: Option<MultiUse>,
    #[serde(default)]
    pub redemption_history: Vec<RedemptionRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancellation: Option<Cancellation>,
}

/// Whether the redeemable is a service or a physical good.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RedeemableType {
    /// A service (e.g., consultation, lesson, repair).
    Service,
    /// A physical good (e.g., handmade item, print).
    Physical,
}

/// Current state of a redeemable idea's fulfillment lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RedemptionStatus {
    /// Not yet claimed.
    Unredeemed,
    /// Fully claimed and fulfilled.
    Redeemed,
    /// Past its validity window.
    Expired,
    /// Cancelled by the provider or redeemer.
    Cancelled,
    /// Some uses remaining (multi-use redeemables).
    PartiallyRedeemed,
}

/// The person or entity who fulfills a redeemable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Provider {
    pub public_key: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact: Option<String>,
}

/// The conditions under which a redeemable can be claimed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Terms {
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_until: Option<DateTime<Utc>>,
    #[serde(default)]
    pub conditions: Vec<String>,
}

/// Configuration for a redeemable that can be used more than once.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultiUse {
    /// Total number of uses allowed.
    pub total: u32,
    /// Uses still available.
    pub remaining: u32,
    pub description: String,
}

/// A single record of a redemption event, with optional proof.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedemptionRecord {
    pub timestamp: DateTime<Utc>,
    pub redeemer: String,
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redeemer_signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_signature: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(default)]
    pub proof: Vec<Proof>,
}

/// Evidence that a redemption occurred (photo, video, signature, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proof {
    #[serde(rename = "type")]
    pub proof_type: ProofType,
    pub asset_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// The kind of evidence attached to a redemption record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProofType {
    /// A photograph.
    Photo,
    /// A video recording.
    Video,
    /// A written document.
    Document,
    /// A cryptographic or handwritten signature.
    Signature,
    /// A timestamped record.
    Timestamp,
}

/// Details of a redeemable that was cancelled before fulfillment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cancellation {
    pub cancelled_at: DateTime<Utc>,
    pub cancelled_by: String,
    pub reason: String,
    #[serde(default)]
    pub refund_offered: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refund_details: Option<String>,
}

impl Redemption {
    /// Creates a new unredeemed Redemption with the given type, provider, and terms.
    pub fn new(
        redeemable_type: RedeemableType,
        provider: Provider,
        terms: Terms,
    ) -> Self {
        Redemption {
            version: "1.0".to_string(),
            redeemable: true,
            redeemable_type,
            status: RedemptionStatus::Unredeemed,
            redeemed_at: None,
            redeemed_by: None,
            provider,
            terms,
            multi_use: None,
            redemption_history: Vec::new(),
            cancellation: None,
        }
    }

    /// Whether this redeemable is currently eligible for redemption.
    pub fn can_redeem(&self) -> bool {
        matches!(
            self.status,
            RedemptionStatus::Unredeemed | RedemptionStatus::PartiallyRedeemed
        ) && !self.terms.is_expired()
    }
}

impl Terms {
    /// Whether the validity window has passed (returns false if no expiry set).
    pub fn is_expired(&self) -> bool {
        self.valid_until
            .map(|d| Utc::now() > d)
            .unwrap_or(false)
    }
}

fn default_version() -> String {
    "1.0".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_cool() {
        let c = Cool::new(100_000);
        assert_eq!(c.cool, 100_000);
        assert_eq!(c.initial_cool, 100_000);
        assert_eq!(c.valuation_history.len(), 1);
        assert!(c.validate_splits().is_ok());
    }

    #[test]
    fn splits_valid() {
        let s = Splits {
            self_percent: 70,
            roots: vec![
                RootSplit {
                    idea_id: Uuid::new_v4(),
                    percentage: 20,
                },
                RootSplit {
                    idea_id: Uuid::new_v4(),
                    percentage: 10,
                },
            ],
        };
        assert!(s.validate().is_ok());
    }

    #[test]
    fn splits_not_100() {
        let s = Splits {
            self_percent: 50,
            roots: vec![RootSplit {
                idea_id: Uuid::new_v4(),
                percentage: 30,
            }],
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn format_values() {
        assert_eq!(Cool::format(500), "c500");
        assert_eq!(Cool::format(1_000), "c1K");
        assert_eq!(Cool::format(100_000), "c100K");
        assert_eq!(Cool::format(1_000_000), "c1M");
        assert_eq!(Cool::format(1_000_000_000), "c1B");
        assert_eq!(Cool::format(1_000_000_000_000), "c1T");
    }

    #[test]
    fn cool_serde_round_trip() {
        let c = Cool::new(50_000);
        let json = serde_json::to_string_pretty(&c).unwrap();
        let rt: Cool = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.cool, c.cool);
        assert_eq!(rt.initial_cool, c.initial_cool);
    }

    #[test]
    fn redemption_serde_round_trip() {
        let r = Redemption::new(
            RedeemableType::Service,
            Provider {
                public_key: "cpub1provider".into(),
                name: "Test Service".into(),
                contact: None,
            },
            Terms {
                description: "One hour consultation".into(),
                location: None,
                valid_until: None,
                conditions: vec!["Must book in advance".into()],
            },
        );
        assert!(r.can_redeem());
        let json = serde_json::to_string_pretty(&r).unwrap();
        let rt: Redemption = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.status, RedemptionStatus::Unredeemed);
        assert!(rt.can_redeem());
    }
}
