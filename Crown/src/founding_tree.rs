//! Founding Verification Tree — the root defense against identity rebirth.
//!
//! Every Crown identity on Omninet traces back through a chain of physical
//! verifications to Crown #1 (the founding identity). This chain is public
//! and forms a tree rooted at the first identity.
//!
//! # Concepts
//!
//! - **Crown #1** is the root. The first identity on Omninet.
//! - Every subsequent identity is verified in-person by someone already in the tree.
//! - Lineage is public — verification is a public trust act.
//! - Depth indicates generation, NOT trust level. Depth 50 is as valid as depth 2.
//! - Anomaly detection identifies suspicious patterns (identity rebirth attempts).
//!
//! # Integration
//!
//! - **KidsSphere (R2B):** Parents can view a person's VerificationLineage when
//!   making approval decisions.
//! - **Identity rebirth detection:** When an Immutable Exclusion is active,
//!   `anomaly_check()` runs on new identities near the excluded identity's verifiers.

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::CrownError;

// ── Types ──────────────────────────────────────────────────────────────────

/// A single link in the verification chain — one identity's proof of physical verification.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct VerificationLineage {
    /// The public key of this identity.
    pub pubkey: String,
    /// The public key of the identity that verified this one.
    /// `None` for the root (Crown #1).
    pub verified_by: Option<String>,
    /// When the verification took place.
    pub verified_at: DateTime<Utc>,
    /// Hash of the proximity proof (opaque to this module — produced by the
    /// physical verification protocol).
    pub proximity_proof: Option<String>,
    /// Distance from Crown #1 in the tree. Root is depth 0.
    pub depth: usize,
    /// Public keys from this identity back to Crown #1, inclusive.
    pub branch_path: Vec<String>,
}

/// The complete founding verification tree, rooted at Crown #1.
///
/// Every identity on Omninet exists as a node in this tree. The tree
/// is append-only — verifications cannot be revoked (but identities
/// can be excluded from the network via other mechanisms).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FoundingTree {
    /// The public key of Crown #1 (the root identity).
    pub root_pubkey: String,
    /// Total number of verified identities in the tree (including root).
    pub total_verified: usize,
    /// Maximum depth in the tree.
    pub max_depth: usize,
    /// Map from pubkey to verification lineage.
    tree: HashMap<String, VerificationLineage>,
    /// Known excluded identities with their exclusion timestamps.
    excluded: HashMap<String, DateTime<Utc>>,
}

/// Categories of suspicious patterns detected in the verification tree.
///
/// These anomalies help identify potential identity rebirth — someone who was
/// excluded attempting to rejoin under a new identity.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TreeAnomaly {
    /// A new identity appeared shortly after another identity was excluded.
    /// Detected by comparing exclusion timestamp of identity A vs.
    /// verification timestamp of identity B, within the same verifier cluster.
    TimingCorrelation {
        /// How many seconds between exclusion and new verification.
        gap_seconds: i64,
    },

    /// The verifiers of a new identity share connections with an excluded
    /// identity's verifiers. Detected by graph proximity analysis.
    VerifierOverlap {
        /// Pubkeys of verifiers shared between the new and excluded identities.
        shared_verifiers: Vec<String>,
    },

    /// New identity's activity patterns resemble an excluded identity.
    /// This is a placeholder — actual comparison is performed externally
    /// by Yoke's behavioral baseline system (R2F).
    BehavioralSimilarity {
        /// Similarity score from external analysis (0.0–1.0).
        similarity_score: f64,
    },

    /// New identity verified in the same geographic region as an excluded
    /// identity, by geographically proximate verifiers.
    GeographicProximity {
        /// Region identifier (opaque string from the verification protocol).
        region: String,
    },

    /// A single verifier has verified an unusually high number of identities
    /// in a short period. Possible verification farm.
    RapidBranching {
        /// The verifier responsible.
        verifier_pubkey: String,
        /// Number of verifications in the time window.
        count: usize,
        /// The time window in seconds.
        window_seconds: i64,
    },
}

/// An alert produced by anomaly detection.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AnomalyAlert {
    /// The type of anomaly detected.
    pub anomaly: TreeAnomaly,
    /// The new identity that triggered the alert.
    pub new_pubkey: String,
    /// The excluded identity this may be related to (if applicable).
    pub related_excluded_pubkey: Option<String>,
    /// Confidence score (0.0–1.0). Higher means more suspicious.
    pub confidence: f64,
    /// When the anomaly was detected.
    pub detected_at: DateTime<Utc>,
}

// ── Configuration ──────────────────────────────────────────────────────────

/// Thresholds for anomaly detection. Tunable per deployment.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AnomalyThresholds {
    /// Maximum seconds between an exclusion and a new verification to
    /// consider it a timing correlation. Default: 30 days.
    pub timing_window_seconds: i64,
    /// Maximum number of verifications by one verifier within
    /// `rapid_branch_window_seconds` before it counts as rapid branching.
    /// Default: 10.
    pub rapid_branch_max: usize,
    /// Time window for rapid branching detection, in seconds.
    /// Default: 7 days.
    pub rapid_branch_window_seconds: i64,
}

impl Default for AnomalyThresholds {
    fn default() -> Self {
        Self {
            timing_window_seconds: 30 * 24 * 3600,       // 30 days
            rapid_branch_max: 10,
            rapid_branch_window_seconds: 7 * 24 * 3600,  // 7 days
        }
    }
}

// ── FoundingTree implementation ────────────────────────────────────────────

impl FoundingTree {
    /// Create a new founding tree with the given root pubkey (Crown #1).
    ///
    /// The root identity is inserted at depth 0 with no verifier.
    pub fn new(root_pubkey: impl Into<String>) -> Self {
        let root = root_pubkey.into();
        let lineage = VerificationLineage {
            pubkey: root.clone(),
            verified_by: None,
            verified_at: Utc::now(),
            proximity_proof: None,
            depth: 0,
            branch_path: vec![root.clone()],
        };

        let mut tree = HashMap::new();
        tree.insert(root.clone(), lineage);

        Self {
            root_pubkey: root,
            total_verified: 1,
            max_depth: 0,
            tree,
            excluded: HashMap::new(),
        }
    }

    /// Add an identity to the tree via physical verification.
    ///
    /// # Errors
    ///
    /// Returns `CrownError::VerificationFailed` if:
    /// - The verifier is not in the tree
    /// - The pubkey is already verified
    /// - The pubkey is empty
    pub fn verify(
        &mut self,
        pubkey: impl Into<String>,
        verified_by: impl Into<String>,
        proximity_proof: impl Into<String>,
    ) -> Result<&VerificationLineage, CrownError> {
        let pubkey = pubkey.into();
        let verified_by = verified_by.into();
        let proximity_proof = proximity_proof.into();

        if pubkey.is_empty() {
            return Err(CrownError::VerificationFailed);
        }

        if self.tree.contains_key(&pubkey) {
            return Err(CrownError::VerificationFailed);
        }

        let verifier = self
            .tree
            .get(&verified_by)
            .ok_or(CrownError::VerificationFailed)?;

        let depth = verifier.depth + 1;
        let mut branch_path = verifier.branch_path.clone();
        branch_path.push(pubkey.clone());

        let lineage = VerificationLineage {
            pubkey: pubkey.clone(),
            verified_by: Some(verified_by),
            verified_at: Utc::now(),
            proximity_proof: Some(proximity_proof),
            depth,
            branch_path,
        };

        self.tree.insert(pubkey.clone(), lineage);
        self.total_verified += 1;
        if depth > self.max_depth {
            self.max_depth = depth;
        }

        // Safe: we just inserted this key.
        Ok(self.tree.get(&pubkey).expect("just inserted"))
    }

    /// Add an identity with a specific timestamp (for testing and replaying events).
    ///
    /// Same as `verify()` but allows setting the verification time.
    pub fn verify_at(
        &mut self,
        pubkey: impl Into<String>,
        verified_by: impl Into<String>,
        proximity_proof: impl Into<String>,
        verified_at: DateTime<Utc>,
    ) -> Result<&VerificationLineage, CrownError> {
        let pubkey = pubkey.into();
        let verified_by = verified_by.into();
        let proximity_proof = proximity_proof.into();

        if pubkey.is_empty() {
            return Err(CrownError::VerificationFailed);
        }

        if self.tree.contains_key(&pubkey) {
            return Err(CrownError::VerificationFailed);
        }

        let verifier = self
            .tree
            .get(&verified_by)
            .ok_or(CrownError::VerificationFailed)?;

        let depth = verifier.depth + 1;
        let mut branch_path = verifier.branch_path.clone();
        branch_path.push(pubkey.clone());

        let lineage = VerificationLineage {
            pubkey: pubkey.clone(),
            verified_by: Some(verified_by),
            verified_at,
            proximity_proof: Some(proximity_proof),
            depth,
            branch_path,
        };

        self.tree.insert(pubkey.clone(), lineage);
        self.total_verified += 1;
        if depth > self.max_depth {
            self.max_depth = depth;
        }

        Ok(self.tree.get(&pubkey).expect("just inserted"))
    }

    /// Look up the verification lineage for a pubkey.
    #[must_use]
    pub fn lineage(&self, pubkey: &str) -> Option<&VerificationLineage> {
        self.tree.get(pubkey)
    }

    /// Find the most recent common ancestor of two identities in the tree.
    ///
    /// Returns `None` if either pubkey is not in the tree.
    #[must_use]
    pub fn common_ancestor(&self, pubkey_a: &str, pubkey_b: &str) -> Option<String> {
        let lineage_a = self.tree.get(pubkey_a)?;
        let lineage_b = self.tree.get(pubkey_b)?;

        // Walk both branch paths from root, find where they diverge.
        // The last common element is the common ancestor.
        let path_a = &lineage_a.branch_path;
        let path_b = &lineage_b.branch_path;

        let mut ancestor = None;
        for (a, b) in path_a.iter().zip(path_b.iter()) {
            if a == b {
                ancestor = Some(a.clone());
            } else {
                break;
            }
        }

        ancestor
    }

    /// Find other identities verified by the same verifier (siblings).
    ///
    /// Does not include the queried pubkey itself.
    #[must_use]
    pub fn siblings(&self, pubkey: &str) -> Vec<String> {
        let lineage = match self.tree.get(pubkey) {
            Some(l) => l,
            None => return Vec::new(),
        };

        let verifier = match &lineage.verified_by {
            Some(v) => v,
            None => return Vec::new(), // root has no siblings
        };

        self.tree
            .values()
            .filter(|l| l.verified_by.as_deref() == Some(verifier) && l.pubkey != pubkey)
            .map(|l| l.pubkey.clone())
            .collect()
    }

    /// Recursively collect everyone this identity has verified, and their
    /// verifiees, and so on down the tree.
    #[must_use]
    pub fn subtree(&self, pubkey: &str) -> Vec<String> {
        let mut result = Vec::new();
        let mut stack = vec![pubkey.to_string()];

        while let Some(current) = stack.pop() {
            // Find everyone verified by `current`.
            for lineage in self.tree.values() {
                if lineage.verified_by.as_deref() == Some(&current) {
                    result.push(lineage.pubkey.clone());
                    stack.push(lineage.pubkey.clone());
                }
            }
        }

        result
    }

    /// Record that an identity has been excluded from the network.
    ///
    /// This is used by anomaly detection to identify timing correlations
    /// between exclusions and new verifications.
    pub fn record_exclusion(&mut self, pubkey: impl Into<String>) {
        self.excluded.insert(pubkey.into(), Utc::now());
    }

    /// Record an exclusion with a specific timestamp.
    pub fn record_exclusion_at(
        &mut self,
        pubkey: impl Into<String>,
        excluded_at: DateTime<Utc>,
    ) {
        self.excluded.insert(pubkey.into(), excluded_at);
    }

    /// Check a pubkey for suspicious patterns that may indicate identity rebirth.
    ///
    /// Uses default anomaly thresholds.
    #[must_use]
    pub fn anomaly_check(&self, pubkey: &str) -> Vec<AnomalyAlert> {
        self.anomaly_check_with_thresholds(pubkey, &AnomalyThresholds::default())
    }

    /// Check a pubkey for suspicious patterns with custom thresholds.
    #[must_use]
    pub fn anomaly_check_with_thresholds(
        &self,
        pubkey: &str,
        thresholds: &AnomalyThresholds,
    ) -> Vec<AnomalyAlert> {
        let mut alerts = Vec::new();

        let lineage = match self.tree.get(pubkey) {
            Some(l) => l,
            None => return alerts,
        };

        // 1. Timing correlation — was this identity verified shortly after
        //    someone was excluded?
        self.check_timing_correlation(lineage, thresholds, &mut alerts);

        // 2. Verifier overlap — does this identity's verification chain
        //    share verifiers with excluded identities?
        self.check_verifier_overlap(lineage, &mut alerts);

        // 3. Rapid branching — did the verifier verify too many people too fast?
        self.check_rapid_branching(lineage, thresholds, &mut alerts);

        alerts
    }

    /// Detect timing correlation between new verifications and recent exclusions.
    fn check_timing_correlation(
        &self,
        lineage: &VerificationLineage,
        thresholds: &AnomalyThresholds,
        alerts: &mut Vec<AnomalyAlert>,
    ) {
        let verified_at = lineage.verified_at;

        for (excluded_pubkey, excluded_at) in &self.excluded {
            let gap = verified_at
                .signed_duration_since(*excluded_at)
                .num_seconds();

            // Only flag if the verification happened AFTER the exclusion
            // and within the timing window.
            if gap >= 0 && gap <= thresholds.timing_window_seconds {
                // Confidence scales inversely with the gap — closer in time = more suspicious.
                let confidence = 1.0
                    - (gap as f64 / thresholds.timing_window_seconds as f64);
                let confidence = confidence.clamp(0.0, 1.0);

                alerts.push(AnomalyAlert {
                    anomaly: TreeAnomaly::TimingCorrelation {
                        gap_seconds: gap,
                    },
                    new_pubkey: lineage.pubkey.clone(),
                    related_excluded_pubkey: Some(excluded_pubkey.clone()),
                    confidence,
                    detected_at: Utc::now(),
                });
            }
        }
    }

    /// Detect verifier overlap with excluded identities.
    fn check_verifier_overlap(
        &self,
        lineage: &VerificationLineage,
        alerts: &mut Vec<AnomalyAlert>,
    ) {
        let new_verifiers: HashSet<&str> = lineage
            .branch_path
            .iter()
            .filter(|p| p.as_str() != lineage.pubkey)
            .map(|s| s.as_str())
            .collect();

        for excluded_pubkey in self.excluded.keys() {
            if let Some(excluded_lineage) = self.tree.get(excluded_pubkey) {
                let excluded_verifiers: HashSet<&str> = excluded_lineage
                    .branch_path
                    .iter()
                    .filter(|p| p.as_str() != excluded_pubkey)
                    .map(|s| s.as_str())
                    .collect();

                let shared: Vec<String> = new_verifiers
                    .intersection(&excluded_verifiers)
                    .map(|s| s.to_string())
                    .collect();

                if !shared.is_empty() {
                    // Confidence scales with how many verifiers overlap.
                    let max_possible = new_verifiers.len().max(1);
                    let confidence =
                        (shared.len() as f64 / max_possible as f64).clamp(0.0, 1.0);

                    alerts.push(AnomalyAlert {
                        anomaly: TreeAnomaly::VerifierOverlap {
                            shared_verifiers: shared,
                        },
                        new_pubkey: lineage.pubkey.clone(),
                        related_excluded_pubkey: Some(excluded_pubkey.clone()),
                        confidence,
                        detected_at: Utc::now(),
                    });
                }
            }
        }
    }

    /// Detect rapid branching — a single verifier verifying too many identities
    /// in a short time window.
    fn check_rapid_branching(
        &self,
        lineage: &VerificationLineage,
        thresholds: &AnomalyThresholds,
        alerts: &mut Vec<AnomalyAlert>,
    ) {
        let verifier = match &lineage.verified_by {
            Some(v) => v,
            None => return, // root
        };

        // Count how many identities this verifier has verified within the window.
        let window_start = lineage
            .verified_at
            .checked_sub_signed(chrono::Duration::seconds(
                thresholds.rapid_branch_window_seconds,
            ));

        let window_start = match window_start {
            Some(ws) => ws,
            None => return,
        };

        let count = self
            .tree
            .values()
            .filter(|l| {
                l.verified_by.as_deref() == Some(verifier)
                    && l.verified_at >= window_start
                    && l.verified_at <= lineage.verified_at
            })
            .count();

        if count > thresholds.rapid_branch_max {
            let confidence =
                ((count as f64 - thresholds.rapid_branch_max as f64)
                    / thresholds.rapid_branch_max as f64)
                    .clamp(0.0, 1.0);

            alerts.push(AnomalyAlert {
                anomaly: TreeAnomaly::RapidBranching {
                    verifier_pubkey: verifier.clone(),
                    count,
                    window_seconds: thresholds.rapid_branch_window_seconds,
                },
                new_pubkey: lineage.pubkey.clone(),
                related_excluded_pubkey: None,
                confidence,
                detected_at: Utc::now(),
            });
        }
    }

    /// Register an externally-computed behavioral similarity anomaly.
    ///
    /// Behavioral analysis is performed by Yoke (R2F). This method lets
    /// the caller inject the result into an anomaly alert.
    #[must_use]
    pub fn behavioral_similarity_alert(
        new_pubkey: &str,
        excluded_pubkey: &str,
        similarity_score: f64,
    ) -> AnomalyAlert {
        AnomalyAlert {
            anomaly: TreeAnomaly::BehavioralSimilarity {
                similarity_score: similarity_score.clamp(0.0, 1.0),
            },
            new_pubkey: new_pubkey.to_string(),
            related_excluded_pubkey: Some(excluded_pubkey.to_string()),
            confidence: similarity_score.clamp(0.0, 1.0),
            detected_at: Utc::now(),
        }
    }

    /// Register an externally-computed geographic proximity anomaly.
    ///
    /// Geographic analysis is performed by World. This method lets the
    /// caller inject the result into an anomaly alert.
    #[must_use]
    pub fn geographic_proximity_alert(
        new_pubkey: &str,
        excluded_pubkey: &str,
        region: &str,
        confidence: f64,
    ) -> AnomalyAlert {
        AnomalyAlert {
            anomaly: TreeAnomaly::GeographicProximity {
                region: region.to_string(),
            },
            new_pubkey: new_pubkey.to_string(),
            related_excluded_pubkey: Some(excluded_pubkey.to_string()),
            confidence: confidence.clamp(0.0, 1.0),
            detected_at: Utc::now(),
        }
    }

    /// Whether a pubkey is in the tree.
    #[must_use]
    pub fn contains(&self, pubkey: &str) -> bool {
        self.tree.contains_key(pubkey)
    }

    /// Whether a pubkey has been excluded.
    #[must_use]
    pub fn is_excluded(&self, pubkey: &str) -> bool {
        self.excluded.contains_key(pubkey)
    }

    /// The depth of a pubkey in the tree, or `None` if not present.
    #[must_use]
    pub fn depth(&self, pubkey: &str) -> Option<usize> {
        self.tree.get(pubkey).map(|l| l.depth)
    }

    /// Number of direct verifiees of a pubkey (not recursive).
    #[must_use]
    pub fn direct_verifiee_count(&self, pubkey: &str) -> usize {
        self.tree
            .values()
            .filter(|l| l.verified_by.as_deref() == Some(pubkey))
            .count()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    // -- Bootstrap & basic structure --

    #[test]
    fn bootstrap_from_single_root() {
        let tree = FoundingTree::new("crown1");
        assert_eq!(tree.total_verified, 1);
        assert_eq!(tree.max_depth, 0);
        assert_eq!(tree.root_pubkey, "crown1");

        let root = tree.lineage("crown1").unwrap();
        assert_eq!(root.depth, 0);
        assert!(root.verified_by.is_none());
        assert_eq!(root.branch_path, vec!["crown1"]);
    }

    #[test]
    fn root_has_no_siblings() {
        let tree = FoundingTree::new("crown1");
        assert!(tree.siblings("crown1").is_empty());
    }

    #[test]
    fn root_contains_check() {
        let tree = FoundingTree::new("crown1");
        assert!(tree.contains("crown1"));
        assert!(!tree.contains("unknown"));
    }

    // -- Verification chain --

    #[test]
    fn verify_single_identity() {
        let mut tree = FoundingTree::new("crown1");
        let lineage = tree.verify("alice", "crown1", "proof_a").unwrap();

        assert_eq!(lineage.depth, 1);
        assert_eq!(lineage.verified_by.as_deref(), Some("crown1"));
        assert_eq!(lineage.branch_path, vec!["crown1", "alice"]);
        assert_eq!(tree.total_verified, 2);
        assert_eq!(tree.max_depth, 1);
    }

    #[test]
    fn verify_chain_three_deep() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("bob", "alice", "proof_b").unwrap();
        tree.verify("carol", "bob", "proof_c").unwrap();

        let carol = tree.lineage("carol").unwrap();
        assert_eq!(carol.depth, 3);
        assert_eq!(
            carol.branch_path,
            vec!["crown1", "alice", "bob", "carol"]
        );
        assert_eq!(tree.max_depth, 3);
        assert_eq!(tree.total_verified, 4);
    }

    #[test]
    fn verify_branching_tree() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("bob", "crown1", "proof_b").unwrap();
        tree.verify("carol", "alice", "proof_c").unwrap();
        tree.verify("dave", "alice", "proof_d").unwrap();

        assert_eq!(tree.total_verified, 5);
        assert_eq!(tree.max_depth, 2);

        let carol = tree.lineage("carol").unwrap();
        assert_eq!(carol.branch_path, vec!["crown1", "alice", "carol"]);

        let bob = tree.lineage("bob").unwrap();
        assert_eq!(bob.branch_path, vec!["crown1", "bob"]);
    }

    #[test]
    fn verify_duplicate_fails() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();

        let result = tree.verify("alice", "crown1", "proof_a2");
        assert!(result.is_err());
    }

    #[test]
    fn verify_unknown_verifier_fails() {
        let mut tree = FoundingTree::new("crown1");

        let result = tree.verify("alice", "unknown_verifier", "proof_a");
        assert!(result.is_err());
    }

    #[test]
    fn verify_empty_pubkey_fails() {
        let mut tree = FoundingTree::new("crown1");

        let result = tree.verify("", "crown1", "proof");
        assert!(result.is_err());
    }

    #[test]
    fn verify_root_as_new_identity_fails() {
        let mut tree = FoundingTree::new("crown1");

        let result = tree.verify("crown1", "crown1", "proof");
        assert!(result.is_err());
    }

    // -- Lineage tracing --

    #[test]
    fn lineage_traces_back_to_root() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("bob", "alice", "proof_b").unwrap();
        tree.verify("carol", "bob", "proof_c").unwrap();

        let carol = tree.lineage("carol").unwrap();
        assert_eq!(carol.branch_path.first().unwrap(), "crown1");
        assert_eq!(carol.branch_path.last().unwrap(), "carol");
        assert_eq!(carol.branch_path.len(), 4);
    }

    #[test]
    fn lineage_unknown_returns_none() {
        let tree = FoundingTree::new("crown1");
        assert!(tree.lineage("unknown").is_none());
    }

    #[test]
    fn lineage_root() {
        let tree = FoundingTree::new("crown1");
        let root = tree.lineage("crown1").unwrap();
        assert_eq!(root.branch_path, vec!["crown1"]);
        assert_eq!(root.depth, 0);
    }

    // -- Common ancestor --

    #[test]
    fn common_ancestor_same_verifier() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("bob", "crown1", "proof_b").unwrap();

        let ancestor = tree.common_ancestor("alice", "bob").unwrap();
        assert_eq!(ancestor, "crown1");
    }

    #[test]
    fn common_ancestor_different_branches() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("bob", "crown1", "proof_b").unwrap();
        tree.verify("carol", "alice", "proof_c").unwrap();
        tree.verify("dave", "bob", "proof_d").unwrap();

        let ancestor = tree.common_ancestor("carol", "dave").unwrap();
        assert_eq!(ancestor, "crown1");
    }

    #[test]
    fn common_ancestor_one_is_ancestor_of_other() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("bob", "alice", "proof_b").unwrap();

        let ancestor = tree.common_ancestor("alice", "bob").unwrap();
        assert_eq!(ancestor, "alice");
    }

    #[test]
    fn common_ancestor_same_identity() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();

        let ancestor = tree.common_ancestor("alice", "alice").unwrap();
        assert_eq!(ancestor, "alice");
    }

    #[test]
    fn common_ancestor_unknown_returns_none() {
        let tree = FoundingTree::new("crown1");
        assert!(tree.common_ancestor("alice", "bob").is_none());
        assert!(tree.common_ancestor("crown1", "unknown").is_none());
    }

    #[test]
    fn common_ancestor_deep_branches() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("bob", "alice", "proof_b").unwrap();
        tree.verify("carol", "bob", "proof_c").unwrap();
        tree.verify("dave", "alice", "proof_d").unwrap();
        tree.verify("eve", "dave", "proof_e").unwrap();

        // carol: crown1 -> alice -> bob -> carol
        // eve:   crown1 -> alice -> dave -> eve
        let ancestor = tree.common_ancestor("carol", "eve").unwrap();
        assert_eq!(ancestor, "alice");
    }

    // -- Siblings --

    #[test]
    fn siblings_basic() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("bob", "crown1", "proof_b").unwrap();
        tree.verify("carol", "crown1", "proof_c").unwrap();

        let mut sibs = tree.siblings("alice");
        sibs.sort();
        assert_eq!(sibs, vec!["bob", "carol"]);
    }

    #[test]
    fn siblings_different_verifier() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("bob", "alice", "proof_b").unwrap();

        // alice was verified by crown1, bob by alice — not siblings.
        assert!(tree.siblings("bob").is_empty());
    }

    #[test]
    fn siblings_unknown_returns_empty() {
        let tree = FoundingTree::new("crown1");
        assert!(tree.siblings("unknown").is_empty());
    }

    // -- Subtree --

    #[test]
    fn subtree_basic() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("bob", "alice", "proof_b").unwrap();
        tree.verify("carol", "alice", "proof_c").unwrap();
        tree.verify("dave", "bob", "proof_d").unwrap();

        let mut sub = tree.subtree("alice");
        sub.sort();
        assert_eq!(sub, vec!["bob", "carol", "dave"]);
    }

    #[test]
    fn subtree_leaf_is_empty() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();

        assert!(tree.subtree("alice").is_empty());
    }

    #[test]
    fn subtree_root_is_everyone() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("bob", "crown1", "proof_b").unwrap();
        tree.verify("carol", "alice", "proof_c").unwrap();

        let mut sub = tree.subtree("crown1");
        sub.sort();
        assert_eq!(sub, vec!["alice", "bob", "carol"]);
    }

    #[test]
    fn subtree_unknown_returns_empty() {
        let tree = FoundingTree::new("crown1");
        assert!(tree.subtree("nonexistent").is_empty());
    }

    // -- Anomaly detection: Timing correlation --

    #[test]
    fn anomaly_timing_correlation() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();

        let exclusion_time = Utc::now() - Duration::hours(1);
        tree.record_exclusion_at("alice", exclusion_time);

        // Bob is verified 30 minutes after Alice's exclusion.
        let bob_time = exclusion_time + Duration::minutes(30);
        tree.verify_at("bob", "crown1", "proof_b", bob_time)
            .unwrap();

        let alerts = tree.anomaly_check("bob");
        let timing_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| matches!(a.anomaly, TreeAnomaly::TimingCorrelation { .. }))
            .collect();

        assert!(!timing_alerts.is_empty());
        assert_eq!(
            timing_alerts[0].related_excluded_pubkey.as_deref(),
            Some("alice")
        );
        assert!(timing_alerts[0].confidence > 0.9); // 30 min out of 30 days = very suspicious
    }

    #[test]
    fn anomaly_timing_no_correlation_when_gap_too_large() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();

        let exclusion_time = Utc::now() - Duration::days(60);
        tree.record_exclusion_at("alice", exclusion_time);

        // Bob verified 60 days after exclusion — outside the 30-day window.
        tree.verify("bob", "crown1", "proof_b").unwrap();

        let alerts = tree.anomaly_check("bob");
        let timing_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| matches!(a.anomaly, TreeAnomaly::TimingCorrelation { .. }))
            .collect();

        assert!(timing_alerts.is_empty());
    }

    #[test]
    fn anomaly_timing_before_exclusion_not_flagged() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();

        // Bob was verified BEFORE alice was excluded.
        let bob_time = Utc::now() - Duration::days(10);
        tree.verify_at("bob", "crown1", "proof_b", bob_time)
            .unwrap();

        let exclusion_time = Utc::now() - Duration::days(5);
        tree.record_exclusion_at("alice", exclusion_time);

        let alerts = tree.anomaly_check("bob");
        let timing_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| matches!(a.anomaly, TreeAnomaly::TimingCorrelation { .. }))
            .collect();

        assert!(timing_alerts.is_empty());
    }

    // -- Anomaly detection: Verifier overlap --

    #[test]
    fn anomaly_verifier_overlap() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("eve", "alice", "proof_e").unwrap();

        // Eve gets excluded.
        tree.record_exclusion("eve");

        // New identity "newguy" is also verified by alice (same as excluded eve).
        tree.verify("newguy", "alice", "proof_n").unwrap();

        let alerts = tree.anomaly_check("newguy");
        let overlap_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| matches!(a.anomaly, TreeAnomaly::VerifierOverlap { .. }))
            .collect();

        assert!(!overlap_alerts.is_empty());

        if let TreeAnomaly::VerifierOverlap {
            ref shared_verifiers,
        } = overlap_alerts[0].anomaly
        {
            // crown1 and alice are shared in both branch paths.
            assert!(!shared_verifiers.is_empty());
        } else {
            panic!("expected VerifierOverlap");
        }
    }

    #[test]
    fn anomaly_no_verifier_overlap_with_unrelated() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("bob", "crown1", "proof_b").unwrap();
        tree.verify("eve", "alice", "proof_e").unwrap();
        tree.verify("carol", "bob", "proof_c").unwrap();

        // Eve excluded. Carol verified through Bob — different branch.
        tree.record_exclusion("eve");

        let alerts = tree.anomaly_check("carol");
        let overlap_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| {
                if let TreeAnomaly::VerifierOverlap {
                    ref shared_verifiers,
                } = a.anomaly
                {
                    // Only crown1 might overlap — that's the root, shared by everyone.
                    // Check that alice is NOT in shared verifiers.
                    shared_verifiers.contains(&"alice".to_string())
                } else {
                    false
                }
            })
            .collect();

        assert!(overlap_alerts.is_empty());
    }

    // -- Anomaly detection: Rapid branching --

    #[test]
    fn anomaly_rapid_branching() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();

        let base_time = Utc::now();

        // Alice verifies 15 people in one day — well above the default threshold of 10/week.
        for i in 0..15 {
            let t = base_time + Duration::minutes(i * 10);
            tree.verify_at(
                format!("person_{i}"),
                "alice",
                format!("proof_{i}"),
                t,
            )
            .unwrap();
        }

        // Check the last person verified.
        let alerts = tree.anomaly_check("person_14");
        let rapid_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| matches!(a.anomaly, TreeAnomaly::RapidBranching { .. }))
            .collect();

        assert!(!rapid_alerts.is_empty());

        if let TreeAnomaly::RapidBranching {
            ref verifier_pubkey,
            count,
            ..
        } = rapid_alerts[0].anomaly
        {
            assert_eq!(verifier_pubkey, "alice");
            assert!(count > 10);
        } else {
            panic!("expected RapidBranching");
        }
    }

    #[test]
    fn anomaly_no_rapid_branching_normal_rate() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();

        let base_time = Utc::now();

        // Alice verifies 3 people over a week — normal rate.
        for i in 0..3 {
            let t = base_time + Duration::days(i * 2);
            tree.verify_at(
                format!("person_{i}"),
                "alice",
                format!("proof_{i}"),
                t,
            )
            .unwrap();
        }

        let alerts = tree.anomaly_check("person_2");
        let rapid_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| matches!(a.anomaly, TreeAnomaly::RapidBranching { .. }))
            .collect();

        assert!(rapid_alerts.is_empty());
    }

    #[test]
    fn anomaly_rapid_branching_custom_thresholds() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();

        let base_time = Utc::now();

        // Alice verifies 5 people in one day.
        for i in 0..5 {
            let t = base_time + Duration::hours(i);
            tree.verify_at(
                format!("person_{i}"),
                "alice",
                format!("proof_{i}"),
                t,
            )
            .unwrap();
        }

        // With strict thresholds (max 3 per day), this should trigger.
        let strict = AnomalyThresholds {
            timing_window_seconds: 30 * 24 * 3600,
            rapid_branch_max: 3,
            rapid_branch_window_seconds: 24 * 3600,
        };

        let alerts = tree.anomaly_check_with_thresholds("person_4", &strict);
        let rapid_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| matches!(a.anomaly, TreeAnomaly::RapidBranching { .. }))
            .collect();

        assert!(!rapid_alerts.is_empty());
    }

    // -- Anomaly detection: Behavioral similarity (external) --

    #[test]
    fn behavioral_similarity_alert() {
        let alert =
            FoundingTree::behavioral_similarity_alert("newguy", "excluded_eve", 0.85);

        assert_eq!(alert.new_pubkey, "newguy");
        assert_eq!(
            alert.related_excluded_pubkey.as_deref(),
            Some("excluded_eve")
        );
        assert!((alert.confidence - 0.85).abs() < f64::EPSILON);

        if let TreeAnomaly::BehavioralSimilarity { similarity_score } = alert.anomaly {
            assert!((similarity_score - 0.85).abs() < f64::EPSILON);
        } else {
            panic!("expected BehavioralSimilarity");
        }
    }

    #[test]
    fn behavioral_similarity_clamped() {
        let alert = FoundingTree::behavioral_similarity_alert("a", "b", 1.5);
        assert!((alert.confidence - 1.0).abs() < f64::EPSILON);

        let alert = FoundingTree::behavioral_similarity_alert("a", "b", -0.3);
        assert!(alert.confidence.abs() < f64::EPSILON);
    }

    // -- Anomaly detection: Geographic proximity (external) --

    #[test]
    fn geographic_proximity_alert() {
        let alert = FoundingTree::geographic_proximity_alert(
            "newguy",
            "excluded_eve",
            "portland_or",
            0.7,
        );

        assert_eq!(alert.new_pubkey, "newguy");

        if let TreeAnomaly::GeographicProximity { ref region } = alert.anomaly {
            assert_eq!(region, "portland_or");
        } else {
            panic!("expected GeographicProximity");
        }
    }

    // -- Exclusion tracking --

    #[test]
    fn exclusion_tracking() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();

        assert!(!tree.is_excluded("alice"));
        tree.record_exclusion("alice");
        assert!(tree.is_excluded("alice"));
    }

    // -- Depth --

    #[test]
    fn depth_tracking() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("bob", "alice", "proof_b").unwrap();

        assert_eq!(tree.depth("crown1"), Some(0));
        assert_eq!(tree.depth("alice"), Some(1));
        assert_eq!(tree.depth("bob"), Some(2));
        assert_eq!(tree.depth("unknown"), None);
    }

    // -- Direct verifiee count --

    #[test]
    fn direct_verifiee_count() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("bob", "crown1", "proof_b").unwrap();
        tree.verify("carol", "alice", "proof_c").unwrap();

        assert_eq!(tree.direct_verifiee_count("crown1"), 2);
        assert_eq!(tree.direct_verifiee_count("alice"), 1);
        assert_eq!(tree.direct_verifiee_count("bob"), 0);
    }

    // -- Serde round-trip --

    #[test]
    fn serde_round_trip() {
        let mut tree = FoundingTree::new("crown1");
        tree.verify("alice", "crown1", "proof_a").unwrap();
        tree.verify("bob", "alice", "proof_b").unwrap();
        tree.record_exclusion("alice");

        let json = serde_json::to_string(&tree).unwrap();
        let loaded: FoundingTree = serde_json::from_str(&json).unwrap();

        assert_eq!(tree.total_verified, loaded.total_verified);
        assert_eq!(tree.max_depth, loaded.max_depth);
        assert_eq!(tree.root_pubkey, loaded.root_pubkey);
        assert!(loaded.is_excluded("alice"));
        assert!(loaded.contains("bob"));
    }

    #[test]
    fn anomaly_alert_serde_round_trip() {
        let alert = AnomalyAlert {
            anomaly: TreeAnomaly::TimingCorrelation { gap_seconds: 3600 },
            new_pubkey: "bob".to_string(),
            related_excluded_pubkey: Some("alice".to_string()),
            confidence: 0.95,
            detected_at: Utc::now(),
        };

        let json = serde_json::to_string(&alert).unwrap();
        let loaded: AnomalyAlert = serde_json::from_str(&json).unwrap();

        assert_eq!(alert.new_pubkey, loaded.new_pubkey);
        assert!((alert.confidence - loaded.confidence).abs() < f64::EPSILON);
    }

    // -- Integration-style: full tree lifecycle --

    #[test]
    fn full_tree_lifecycle() {
        // Crown #1 bootstraps the tree.
        let mut tree = FoundingTree::new("sam");

        // First generation: Sam verifies 3 people in person.
        tree.verify("alice", "sam", "handshake_alice").unwrap();
        tree.verify("bob", "sam", "handshake_bob").unwrap();
        tree.verify("carol", "sam", "handshake_carol").unwrap();

        // Second generation: Alice verifies Dave, Bob verifies Eve.
        tree.verify("dave", "alice", "handshake_dave").unwrap();
        tree.verify("eve", "bob", "handshake_eve").unwrap();

        // Third generation: Dave verifies Frank.
        tree.verify("frank", "dave", "handshake_frank").unwrap();

        assert_eq!(tree.total_verified, 7);
        assert_eq!(tree.max_depth, 3);

        // Frank's lineage traces all the way back.
        let frank = tree.lineage("frank").unwrap();
        assert_eq!(
            frank.branch_path,
            vec!["sam", "alice", "dave", "frank"]
        );

        // Dave and Eve share Sam as their common ancestor (via alice and bob).
        // Actually: dave = sam -> alice -> dave, eve = sam -> bob -> eve
        let ancestor = tree.common_ancestor("dave", "eve").unwrap();
        assert_eq!(ancestor, "sam");

        // Alice and Bob are siblings (both verified by Sam).
        let mut alice_sibs = tree.siblings("alice");
        alice_sibs.sort();
        assert_eq!(alice_sibs, vec!["bob", "carol"]);

        // Alice's subtree: dave, frank.
        let mut alice_sub = tree.subtree("alice");
        alice_sub.sort();
        assert_eq!(alice_sub, vec!["dave", "frank"]);

        // Sam's subtree is everyone.
        let mut all = tree.subtree("sam");
        all.sort();
        assert_eq!(all, vec!["alice", "bob", "carol", "dave", "eve", "frank"]);
    }
}
