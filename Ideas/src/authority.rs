use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::IdeasError;

// ── Book (Ownership Ledger) ──

/// A Book records ownership history for an .idea file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Book {
    #[serde(default = "default_version")]
    pub version: String,
    pub creator: BookCreator,
    pub current_owner: Owner,
    #[serde(default)]
    pub transfers: Vec<Transfer>,
    #[serde(default)]
    pub endorsements: Vec<Endorsement>,
}

/// The original creator (immutable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BookCreator {
    pub public_key: String,
    pub timestamp: DateTime<Utc>,
    pub signature: String,
}

/// The current owner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Owner {
    pub public_key: String,
    pub since: DateTime<Utc>,
}

/// A record of ownership transfer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Transfer {
    pub from: String,
    pub to: String,
    pub timestamp: DateTime<Utc>,
    pub reason: TransferReason,
    pub signature: String,
    #[serde(default)]
    pub witnesses: Vec<Witness>,
}

/// Why an ownership transfer happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TransferReason {
    /// A mutual exchange of value.
    Trade,
    /// Freely given, no consideration.
    Gift,
    /// Sold for Cool.
    Sale,
    /// Passed down after the original owner is gone.
    Inheritance,
    /// Fixing a mistake or resolving a dispute.
    Correction,
}

/// A third party who attested to an ownership transfer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Witness {
    pub public_key: String,
    pub signature: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A public endorsement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Endorsement {
    pub endorser: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub signature: String,
}

impl Book {
    /// Creates a new Book where the creator is also the initial owner.
    pub fn new(creator_key: String, signature: String) -> Self {
        let now = Utc::now();
        Book {
            version: "1.0".to_string(),
            creator: BookCreator {
                public_key: creator_key.clone(),
                timestamp: now,
                signature,
            },
            current_owner: Owner {
                public_key: creator_key,
                since: now,
            },
            transfers: Vec::new(),
            endorsements: Vec::new(),
        }
    }

    /// Returns a new Book with the transfer recorded and ownership updated.
    pub fn with_transfer(&self, transfer: Transfer) -> Self {
        let mut copy = self.clone();
        copy.current_owner = Owner {
            public_key: transfer.to.clone(),
            since: transfer.timestamp,
        };
        copy.transfers.push(transfer);
        copy
    }

    /// Returns a new Book with the endorsement appended.
    pub fn with_endorsement(&self, endorsement: Endorsement) -> Self {
        let mut copy = self.clone();
        copy.endorsements.push(endorsement);
        copy
    }

    /// Whether the given public key is the current owner.
    pub fn is_owner(&self, public_key: &str) -> bool {
        self.current_owner.public_key == public_key
    }

    /// Whether the given public key is the original creator.
    pub fn is_creator(&self, public_key: &str) -> bool {
        self.creator.public_key == public_key
    }

    /// Whether this idea has ever changed hands.
    pub fn has_been_transferred(&self) -> bool {
        !self.transfers.is_empty()
    }
}

// ── Tree (Provenance Graph) ──

/// A Tree records the creative lineage of an idea.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tree {
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub roots: Vec<Root>,
    #[serde(default)]
    pub branches: Vec<Branch>,
    #[serde(default)]
    pub references: Vec<TreeReference>,
}

/// An idea this was built on or derived from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Root {
    pub idea_id: Uuid,
    pub creator: String,
    pub relationship: Relationship,
    pub contribution_weight: i32,
    pub timestamp: DateTime<Utc>,
    pub signature: String,
}

/// An idea that was built using this one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    pub idea_id: Uuid,
    pub creator: String,
    pub relationship: Relationship,
    pub timestamp: DateTime<Utc>,
    pub signature: String,
}

/// How a child idea relates to its parent in the provenance tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Relationship {
    /// A creative reworking of the original.
    Remix,
    /// Built directly on the original's content.
    Derivative,
    /// Adapted for a different medium or audience.
    Adaptation,
    /// Translated into another language.
    Translation,
    /// Loosely inspired by, but substantially original.
    Inspiration,
    /// A complete divergent copy.
    Fork,
    /// A sequel or next chapter.
    Continuation,
}

/// A non-derivation link to another idea.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TreeReference {
    /// A reference to a local .idea file on disk.
    Local {
        path: String,
        idea_id: Uuid,
        relationship: ReferenceRelationship,
    },
    /// A reference to an idea accessible through private relays.
    PrivateCrown {
        crown_id: String,
        creator: String,
        relationship: ReferenceRelationship,
        relays: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        access_key: Option<String>,
    },
    /// A reference to an idea accessible through public relays.
    PublicCrown {
        crown_id: String,
        creator: String,
        relationship: ReferenceRelationship,
        relays: Vec<String>,
    },
}

/// How a tree reference (non-derivation link) relates to another idea.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReferenceRelationship {
    /// Actively uses the referenced idea as a dependency.
    Uses,
    /// Mentions the referenced idea in passing.
    Mentions,
    /// Formally cites the referenced idea as a source.
    Cites,
    /// A direct response or reply to the referenced idea.
    RespondsTo,
    /// Disagrees with or counters the referenced idea.
    Contradicts,
    /// Agrees with or bolsters the referenced idea.
    Supports,
}

impl Tree {
    /// Creates a new empty Tree with no roots, branches, or references.
    pub fn new() -> Self {
        Tree {
            version: "1.0".to_string(),
            roots: Vec::new(),
            branches: Vec::new(),
            references: Vec::new(),
        }
    }

    /// Returns a new Tree with a root (parent idea) added.
    pub fn with_root(&self, root: Root) -> Self {
        let mut copy = self.clone();
        copy.roots.push(root);
        copy
    }

    /// Returns a new Tree with a branch (child idea) added.
    pub fn with_branch(&self, branch: Branch) -> Self {
        let mut copy = self.clone();
        copy.branches.push(branch);
        copy
    }

    /// Sum of all root contribution weights. Must not exceed 100.
    pub fn total_root_contribution(&self) -> i32 {
        self.roots.iter().map(|r| r.contribution_weight).sum()
    }

    /// The creator's own contribution percentage (100 minus root contributions).
    pub fn self_contribution(&self) -> i32 {
        (100 - self.total_root_contribution()).max(0)
    }

    /// Validates that root contributions don't exceed 100% and are all positive.
    pub fn validate(&self) -> Result<(), IdeasError> {
        let total = self.total_root_contribution();
        if total > 100 {
            return Err(IdeasError::ContributionExceeds100(total));
        }
        for root in &self.roots {
            if root.contribution_weight <= 0 {
                return Err(IdeasError::InvalidContribution(
                    root.idea_id,
                    root.contribution_weight,
                ));
            }
        }
        Ok(())
    }
}

impl Default for Tree {
    fn default() -> Self {
        Self::new()
    }
}

fn default_version() -> String {
    "1.0".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_book() {
        let book = Book::new("cpub1alice".into(), "sig_alice".into());
        assert!(book.is_owner("cpub1alice"));
        assert!(book.is_creator("cpub1alice"));
        assert!(!book.has_been_transferred());
    }

    #[test]
    fn book_transfer() {
        let book = Book::new("cpub1alice".into(), "sig_alice".into());
        let transfer = Transfer {
            from: "cpub1alice".into(),
            to: "cpub1bob".into(),
            timestamp: Utc::now(),
            reason: TransferReason::Gift,
            signature: "sig_transfer".into(),
            witnesses: Vec::new(),
        };
        let book2 = book.with_transfer(transfer);
        assert!(book2.is_owner("cpub1bob"));
        assert!(!book2.is_owner("cpub1alice"));
        assert!(book2.is_creator("cpub1alice")); // Creator never changes
        assert!(book2.has_been_transferred());
    }

    #[test]
    fn book_serde_round_trip() {
        let book = Book::new("cpub1test".into(), "sig".into());
        let json = serde_json::to_string_pretty(&book).unwrap();
        let rt: Book = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.creator.public_key, book.creator.public_key);
    }

    #[test]
    fn tree_contribution_valid() {
        let tree = Tree::new().with_root(Root {
            idea_id: Uuid::new_v4(),
            creator: "cpub1other".into(),
            relationship: Relationship::Remix,
            contribution_weight: 30,
            timestamp: Utc::now(),
            signature: "sig".into(),
        });
        assert!(tree.validate().is_ok());
        assert_eq!(tree.self_contribution(), 70);
    }

    #[test]
    fn tree_contribution_exceeds_100() {
        let tree = Tree::new()
            .with_root(Root {
                idea_id: Uuid::new_v4(),
                creator: "a".into(),
                relationship: Relationship::Derivative,
                contribution_weight: 60,
                timestamp: Utc::now(),
                signature: "s".into(),
            })
            .with_root(Root {
                idea_id: Uuid::new_v4(),
                creator: "b".into(),
                relationship: Relationship::Fork,
                contribution_weight: 50,
                timestamp: Utc::now(),
                signature: "s".into(),
            });
        assert!(tree.validate().is_err());
    }

    #[test]
    fn tree_serde_round_trip() {
        let tree = Tree::new().with_root(Root {
            idea_id: Uuid::new_v4(),
            creator: "cpub1test".into(),
            relationship: Relationship::Inspiration,
            contribution_weight: 10,
            timestamp: Utc::now(),
            signature: "sig".into(),
        });
        let json = serde_json::to_string_pretty(&tree).unwrap();
        let rt: Tree = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.roots.len(), 1);
        assert_eq!(rt.roots[0].contribution_weight, 10);
    }
}
