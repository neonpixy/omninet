use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::validation;

/// A collection of bonds (references) to other ideas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bonds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<LocalBonds>,
    #[serde(default, rename = "private", skip_serializing_if = "Option::is_none")]
    pub private_bonds: Option<PrivateBonds>,
    #[serde(default, rename = "public", skip_serializing_if = "Option::is_none")]
    pub public_bonds: Option<PublicBonds>,
}

/// How a reference relates to the idea.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BondRelationship {
    /// Actively uses the referenced idea as a dependency.
    Uses,
    /// Mentions the referenced idea in passing.
    Mentions,
    /// Formally cites the referenced idea as a source.
    Cites,
    /// Derived from or built on the referenced idea.
    DerivesFrom,
    /// A direct response or reply.
    RespondsTo,
    /// Disagrees with or counters.
    Contradicts,
    /// Agrees with or bolsters.
    Supports,
    /// A loose association.
    Related,
    /// A live or snapshot data source for bindings.
    DataSource,
}

// ── Local Bonds ──

/// References to .idea files on the local filesystem.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalBonds {
    pub references: Vec<LocalBondReference>,
}

/// A single reference to a local .idea file. Path must be absolute.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LocalBondReference {
    pub idea_id: Uuid,
    /// Absolute filesystem path to the referenced .idea.
    pub path: String,
    pub relationship: BondRelationship,
    #[serde(default)]
    pub verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_verified: Option<DateTime<Utc>>,
}

// ── Private Bonds ──

/// References to ideas accessible through private relays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrivateBonds {
    pub references: Vec<PrivateBondReference>,
}

/// A reference to an idea on a private relay, potentially requiring an access key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PrivateBondReference {
    pub crown_id: String,
    pub idea_id: Uuid,
    pub creator: String,
    pub relationship: BondRelationship,
    pub relays: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_key: Option<String>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_fetched: Option<DateTime<Utc>>,
}

// ── Public Bonds ──

/// References to ideas accessible through public relays.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublicBonds {
    pub references: Vec<PublicBondReference>,
}

/// A reference to an idea on a public relay, optionally cached locally.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublicBondReference {
    pub crown_id: String,
    pub idea_id: Uuid,
    pub creator: String,
    pub relationship: BondRelationship,
    pub relays: Vec<String>,
    #[serde(default)]
    pub verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_fetched: Option<DateTime<Utc>>,
    #[serde(default)]
    pub cached: bool,
}

impl Bonds {
    /// Creates an empty Bonds with no references.
    pub fn new() -> Self {
        Bonds {
            local: None,
            private_bonds: None,
            public_bonds: None,
        }
    }

    /// Total number of references across all bond categories.
    pub fn count(&self) -> usize {
        self.local.as_ref().map_or(0, |b| b.references.len())
            + self.private_bonds.as_ref().map_or(0, |b| b.references.len())
            + self.public_bonds.as_ref().map_or(0, |b| b.references.len())
    }

    /// Whether there are no references in any category.
    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    /// Validates all local bond paths.
    pub fn validate(&self) -> Result<(), crate::error::IdeasError> {
        if let Some(local) = &self.local {
            for r in &local.references {
                validation::validate_local_bond_path(&r.path)?;
            }
        }
        Ok(())
    }
}

impl Default for Bonds {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bonds() {
        let b = Bonds::new();
        assert!(b.is_empty());
        assert_eq!(b.count(), 0);
    }

    #[test]
    fn bonds_with_local() {
        let b = Bonds {
            local: Some(LocalBonds {
                references: vec![LocalBondReference {
                    idea_id: Uuid::new_v4(),
                    path: "/Users/test/doc.idea".into(),
                    relationship: BondRelationship::Uses,
                    verified: false,
                    last_verified: None,
                }],
            }),
            private_bonds: None,
            public_bonds: None,
        };
        assert_eq!(b.count(), 1);
        assert!(b.validate().is_ok());
    }

    #[test]
    fn bonds_invalid_path() {
        let b = Bonds {
            local: Some(LocalBonds {
                references: vec![LocalBondReference {
                    idea_id: Uuid::new_v4(),
                    path: "relative/path".into(),
                    relationship: BondRelationship::Cites,
                    verified: false,
                    last_verified: None,
                }],
            }),
            private_bonds: None,
            public_bonds: None,
        };
        assert!(b.validate().is_err());
    }

    #[test]
    fn serde_round_trip() {
        let b = Bonds {
            local: Some(LocalBonds {
                references: vec![LocalBondReference {
                    idea_id: Uuid::new_v4(),
                    path: "/test.idea".into(),
                    relationship: BondRelationship::Related,
                    verified: true,
                    last_verified: Some(Utc::now()),
                }],
            }),
            private_bonds: None,
            public_bonds: Some(PublicBonds {
                references: vec![PublicBondReference {
                    crown_id: "note1test".into(),
                    idea_id: Uuid::new_v4(),
                    creator: "cpub1test".into(),
                    relationship: BondRelationship::Mentions,
                    relays: vec!["wss://relay.damus.io".into()],
                    verified: false,
                    last_fetched: None,
                    cached: false,
                }],
            }),
        };
        let json = serde_json::to_string_pretty(&b).unwrap();
        let rt: Bonds = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.count(), b.count());
    }
}
