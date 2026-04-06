//! Multi-Jurisdiction Relay Mesh — relay diversity as a health metric.
//!
//! A healthy relay set is spread across multiple legal jurisdictions so that
//! no single government can compel all your relays simultaneously. This module
//! provides the types and scoring logic to measure and improve jurisdiction
//! diversity.
//!
//! # Diversity Score
//!
//! The diversity score uses Simpson's diversity index (1 - D), where D is the
//! probability that two randomly chosen relays are in the same jurisdiction.
//! A score of 0.0 means all relays are in a single jurisdiction; a score
//! approaching 1.0 means relays are evenly spread across many jurisdictions.
//!
//! # Example
//!
//! ```
//! use globe::jurisdiction::{
//!     RelayJurisdiction, LegalFramework, JurisdictionAnalyzer,
//! };
//!
//! let relays = vec![
//!     RelayJurisdiction::new("wss://ch.relay.example", "CH", "Europe")
//!         .with_framework(LegalFramework::StrongPrivacy),
//!     RelayJurisdiction::new("wss://de.relay.example", "DE", "Europe")
//!         .with_framework(LegalFramework::Moderate),
//!     RelayJurisdiction::new("wss://jp.relay.example", "JP", "Asia")
//!         .with_framework(LegalFramework::Moderate),
//! ];
//!
//! let diversity = JurisdictionAnalyzer::analyze(&relays);
//! assert_eq!(diversity.connected_relays, 3);
//! assert_eq!(diversity.unique_jurisdictions, 3);
//! assert!(!diversity.adversarial_only);
//! ```

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Relay Jurisdiction
// ---------------------------------------------------------------------------

/// Jurisdiction metadata for a single relay.
///
/// Relays optionally self-declare their jurisdiction in their info document
/// (NIP-11 equivalent). This struct holds that metadata for diversity analysis.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayJurisdiction {
    /// The relay's WebSocket URL.
    pub relay_url: String,
    /// ISO 3166-1 alpha-2 country code (e.g., "CH", "US", "DE").
    /// `None` if the relay did not declare its jurisdiction.
    pub country_code: Option<String>,
    /// Geographic region (e.g., "Europe", "North America", "Asia").
    /// `None` if unknown.
    pub region: Option<String>,
    /// Classification of the country's legal environment for privacy.
    /// `None` if unknown — treated as [`LegalFramework::Unknown`] for scoring.
    pub legal_framework: Option<LegalFramework>,
}

impl RelayJurisdiction {
    /// Create a new relay jurisdiction with known country and region.
    pub fn new(
        relay_url: impl Into<String>,
        country_code: impl Into<String>,
        region: impl Into<String>,
    ) -> Self {
        Self {
            relay_url: relay_url.into(),
            country_code: Some(country_code.into()),
            region: Some(region.into()),
            legal_framework: None,
        }
    }

    /// Create a relay jurisdiction with unknown location.
    pub fn unknown(relay_url: impl Into<String>) -> Self {
        Self {
            relay_url: relay_url.into(),
            country_code: None,
            region: None,
            legal_framework: None,
        }
    }

    /// Set the legal framework classification.
    #[must_use]
    pub fn with_framework(mut self, framework: LegalFramework) -> Self {
        self.legal_framework = Some(framework);
        self
    }

    /// The effective legal framework, defaulting to `Unknown` when not set.
    #[must_use]
    pub fn effective_framework(&self) -> LegalFramework {
        self.legal_framework.clone().unwrap_or(LegalFramework::Unknown)
    }

    /// The jurisdiction key used for diversity grouping.
    /// Uses country code if available, otherwise the relay URL as a unique key.
    fn jurisdiction_key(&self) -> String {
        self.country_code
            .clone()
            .unwrap_or_else(|| format!("unknown:{}", self.relay_url))
    }
}

// ---------------------------------------------------------------------------
// Legal Framework
// ---------------------------------------------------------------------------

/// Classification of a country's legal environment for privacy and data protection.
///
/// These are broad categories — the real legal landscape is more nuanced,
/// but for relay diversity scoring, broad strokes are sufficient.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LegalFramework {
    /// Strong constitutional privacy protections (e.g., Switzerland, Iceland).
    StrongPrivacy,
    /// Moderate privacy protections with some surveillance authority
    /// (e.g., EU GDPR countries).
    Moderate,
    /// Weak privacy protections or broad surveillance powers (e.g., US).
    Weak,
    /// Active censorship, compelled backdoors, or hostile to privacy tools.
    Adversarial,
    /// Jurisdiction is unknown or the relay did not declare it.
    Unknown,
}

impl LegalFramework {
    /// Whether this framework is classified as adversarial.
    #[must_use]
    pub fn is_adversarial(&self) -> bool {
        matches!(self, LegalFramework::Adversarial)
    }
}

// ---------------------------------------------------------------------------
// Diversity Analysis
// ---------------------------------------------------------------------------

/// The result of analyzing jurisdiction diversity across a set of relays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JurisdictionDiversity {
    /// Total number of relays analyzed.
    pub connected_relays: usize,
    /// Number of unique jurisdictions represented.
    pub unique_jurisdictions: usize,
    /// Simpson's diversity index (1 - D), ranging from 0.0 to 1.0.
    /// Higher values indicate more even distribution across jurisdictions.
    pub diversity_score: f64,
    /// Warning: all relays with known frameworks are in adversarial jurisdictions.
    pub adversarial_only: bool,
    /// Actionable recommendation based on the diversity analysis.
    pub recommendation: DiversityRecommendation,
}

/// Recommendation based on jurisdiction diversity analysis.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiversityRecommendation {
    /// Relay diversity is healthy — at least 3 jurisdictions, score > 0.5,
    /// and not all adversarial.
    Healthy,
    /// Diversity could be improved. Contains a human-readable suggestion.
    AddDiversity(String),
    /// Dangerously low diversity. Contains a human-readable warning.
    CriticallyHomogeneous(String),
}

/// Analyzes jurisdiction diversity across a set of relays.
///
/// This is a stateless analyzer — pass it a snapshot of your current relay
/// set and it returns a [`JurisdictionDiversity`] result.
pub struct JurisdictionAnalyzer;

impl JurisdictionAnalyzer {
    /// Analyze the jurisdiction diversity of a set of relays.
    ///
    /// Returns a [`JurisdictionDiversity`] with the score, warnings, and
    /// recommendations.
    #[must_use]
    pub fn analyze(relays: &[RelayJurisdiction]) -> JurisdictionDiversity {
        if relays.is_empty() {
            return JurisdictionDiversity {
                connected_relays: 0,
                unique_jurisdictions: 0,
                diversity_score: 0.0,
                adversarial_only: false,
                recommendation: DiversityRecommendation::CriticallyHomogeneous(
                    "No relays connected. Add relays in diverse jurisdictions.".into(),
                ),
            };
        }

        // Count relays per jurisdiction
        let mut jurisdiction_counts: HashMap<String, usize> = HashMap::new();
        for relay in relays {
            *jurisdiction_counts
                .entry(relay.jurisdiction_key())
                .or_insert(0) += 1;
        }

        let unique_jurisdictions = jurisdiction_counts.len();
        let total = relays.len();

        // Simpson's diversity index: 1 - sum(n_i * (n_i - 1)) / (N * (N - 1))
        let diversity_score = Self::simpsons_index(&jurisdiction_counts, total);

        // Check if all relays with known frameworks are adversarial
        let adversarial_only = Self::is_adversarial_only(relays);

        let recommendation =
            Self::compute_recommendation(unique_jurisdictions, diversity_score, adversarial_only);

        JurisdictionDiversity {
            connected_relays: total,
            unique_jurisdictions,
            diversity_score,
            adversarial_only,
            recommendation,
        }
    }

    /// Compute Simpson's diversity index (1 - D).
    ///
    /// For a single relay or all relays in one jurisdiction, returns 0.0.
    /// For perfectly even distribution, approaches 1 - 1/k where k is the
    /// number of jurisdictions.
    fn simpsons_index(counts: &HashMap<String, usize>, total: usize) -> f64 {
        if total <= 1 {
            return 0.0;
        }

        let denominator = (total * (total - 1)) as f64;
        let numerator: usize = counts.values().map(|&n| n * n.saturating_sub(1)).sum();

        1.0 - (numerator as f64 / denominator)
    }

    /// Check whether all relays with known frameworks are adversarial.
    ///
    /// Returns `false` if there are no relays with known frameworks
    /// (all unknown does not trigger the adversarial warning).
    fn is_adversarial_only(relays: &[RelayJurisdiction]) -> bool {
        let known: Vec<_> = relays
            .iter()
            .filter(|r| {
                r.legal_framework.is_some()
                    && r.legal_framework.as_ref() != Some(&LegalFramework::Unknown)
            })
            .collect();

        if known.is_empty() {
            return false;
        }

        known
            .iter()
            .all(|r| r.effective_framework().is_adversarial())
    }

    /// Generate a recommendation based on the diversity metrics.
    fn compute_recommendation(
        unique_jurisdictions: usize,
        diversity_score: f64,
        adversarial_only: bool,
    ) -> DiversityRecommendation {
        if adversarial_only {
            return DiversityRecommendation::CriticallyHomogeneous(
                "All relays are in adversarial jurisdictions. \
                 Add relays in privacy-respecting countries."
                    .into(),
            );
        }

        if unique_jurisdictions <= 1 {
            return DiversityRecommendation::CriticallyHomogeneous(
                "All relays are in a single jurisdiction. \
                 A single government could compel all your relays."
                    .into(),
            );
        }

        if diversity_score < 0.3 {
            return DiversityRecommendation::CriticallyHomogeneous(format!(
                "Jurisdiction diversity is very low ({diversity_score:.2}). \
                 Add relays in different countries."
            ));
        }

        if unique_jurisdictions < 3 || diversity_score < 0.5 {
            return DiversityRecommendation::AddDiversity(format!(
                "Consider adding relays in more jurisdictions \
                 (currently {unique_jurisdictions}, score {diversity_score:.2}). \
                 Aim for 3+ countries with a score above 0.5."
            ));
        }

        DiversityRecommendation::Healthy
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn relay(url: &str, country: &str, region: &str, fw: LegalFramework) -> RelayJurisdiction {
        RelayJurisdiction::new(url, country, region).with_framework(fw)
    }

    // -- RelayJurisdiction tests --

    #[test]
    fn relay_jurisdiction_new() {
        let rj = RelayJurisdiction::new("wss://relay.ch", "CH", "Europe");
        assert_eq!(rj.relay_url, "wss://relay.ch");
        assert_eq!(rj.country_code.as_deref(), Some("CH"));
        assert_eq!(rj.region.as_deref(), Some("Europe"));
        assert!(rj.legal_framework.is_none());
    }

    #[test]
    fn relay_jurisdiction_unknown() {
        let rj = RelayJurisdiction::unknown("wss://mystery.relay");
        assert!(rj.country_code.is_none());
        assert!(rj.region.is_none());
        assert!(rj.legal_framework.is_none());
    }

    #[test]
    fn relay_jurisdiction_with_framework() {
        let rj = RelayJurisdiction::new("wss://relay.ch", "CH", "Europe")
            .with_framework(LegalFramework::StrongPrivacy);
        assert_eq!(rj.legal_framework, Some(LegalFramework::StrongPrivacy));
    }

    #[test]
    fn effective_framework_defaults_to_unknown() {
        let rj = RelayJurisdiction::unknown("wss://relay");
        assert_eq!(rj.effective_framework(), LegalFramework::Unknown);
    }

    #[test]
    fn relay_jurisdiction_serde_roundtrip() {
        let rj = relay("wss://relay.ch", "CH", "Europe", LegalFramework::StrongPrivacy);
        let json = serde_json::to_string(&rj).unwrap();
        let loaded: RelayJurisdiction = serde_json::from_str(&json).unwrap();
        assert_eq!(rj, loaded);
    }

    // -- LegalFramework tests --

    #[test]
    fn legal_framework_adversarial_check() {
        assert!(LegalFramework::Adversarial.is_adversarial());
        assert!(!LegalFramework::StrongPrivacy.is_adversarial());
        assert!(!LegalFramework::Moderate.is_adversarial());
        assert!(!LegalFramework::Weak.is_adversarial());
        assert!(!LegalFramework::Unknown.is_adversarial());
    }

    #[test]
    fn legal_framework_serde_roundtrip() {
        for fw in &[
            LegalFramework::StrongPrivacy,
            LegalFramework::Moderate,
            LegalFramework::Weak,
            LegalFramework::Adversarial,
            LegalFramework::Unknown,
        ] {
            let json = serde_json::to_string(fw).unwrap();
            let loaded: LegalFramework = serde_json::from_str(&json).unwrap();
            assert_eq!(*fw, loaded);
        }
    }

    // -- JurisdictionAnalyzer tests --

    #[test]
    fn analyze_empty_set() {
        let diversity = JurisdictionAnalyzer::analyze(&[]);
        assert_eq!(diversity.connected_relays, 0);
        assert_eq!(diversity.unique_jurisdictions, 0);
        assert_eq!(diversity.diversity_score, 0.0);
        assert!(!diversity.adversarial_only);
        assert!(matches!(
            diversity.recommendation,
            DiversityRecommendation::CriticallyHomogeneous(_)
        ));
    }

    #[test]
    fn analyze_single_relay() {
        let relays = vec![relay(
            "wss://relay.ch",
            "CH",
            "Europe",
            LegalFramework::StrongPrivacy,
        )];
        let diversity = JurisdictionAnalyzer::analyze(&relays);
        assert_eq!(diversity.connected_relays, 1);
        assert_eq!(diversity.unique_jurisdictions, 1);
        assert_eq!(diversity.diversity_score, 0.0);
        assert!(!diversity.adversarial_only);
    }

    #[test]
    fn analyze_all_same_jurisdiction() {
        let relays = vec![
            relay("wss://r1.us", "US", "North America", LegalFramework::Weak),
            relay("wss://r2.us", "US", "North America", LegalFramework::Weak),
            relay("wss://r3.us", "US", "North America", LegalFramework::Weak),
        ];
        let diversity = JurisdictionAnalyzer::analyze(&relays);
        assert_eq!(diversity.unique_jurisdictions, 1);
        assert_eq!(diversity.diversity_score, 0.0);
        assert!(matches!(
            diversity.recommendation,
            DiversityRecommendation::CriticallyHomogeneous(_)
        ));
    }

    #[test]
    fn analyze_perfectly_diverse() {
        let relays = vec![
            relay("wss://ch.relay", "CH", "Europe", LegalFramework::StrongPrivacy),
            relay("wss://de.relay", "DE", "Europe", LegalFramework::Moderate),
            relay("wss://jp.relay", "JP", "Asia", LegalFramework::Moderate),
            relay("wss://br.relay", "BR", "South America", LegalFramework::Moderate),
        ];
        let diversity = JurisdictionAnalyzer::analyze(&relays);
        assert_eq!(diversity.connected_relays, 4);
        assert_eq!(diversity.unique_jurisdictions, 4);
        // 4 countries, 1 each: D = 0, score = 1.0
        assert!((diversity.diversity_score - 1.0).abs() < f64::EPSILON);
        assert!(!diversity.adversarial_only);
        assert_eq!(diversity.recommendation, DiversityRecommendation::Healthy);
    }

    #[test]
    fn analyze_two_jurisdictions_uneven() {
        // 3 relays in US, 1 in CH
        let relays = vec![
            relay("wss://r1.us", "US", "North America", LegalFramework::Weak),
            relay("wss://r2.us", "US", "North America", LegalFramework::Weak),
            relay("wss://r3.us", "US", "North America", LegalFramework::Weak),
            relay("wss://r1.ch", "CH", "Europe", LegalFramework::StrongPrivacy),
        ];
        let diversity = JurisdictionAnalyzer::analyze(&relays);
        assert_eq!(diversity.unique_jurisdictions, 2);
        // D = (3*2 + 1*0) / (4*3) = 6/12 = 0.5, score = 0.5
        assert!((diversity.diversity_score - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn analyze_adversarial_only() {
        let relays = vec![
            relay("wss://r1.cn", "CN", "Asia", LegalFramework::Adversarial),
            relay("wss://r2.ru", "RU", "Europe", LegalFramework::Adversarial),
        ];
        let diversity = JurisdictionAnalyzer::analyze(&relays);
        assert!(diversity.adversarial_only);
        assert!(matches!(
            diversity.recommendation,
            DiversityRecommendation::CriticallyHomogeneous(_)
        ));
    }

    #[test]
    fn analyze_mixed_adversarial_not_only() {
        let relays = vec![
            relay("wss://r1.cn", "CN", "Asia", LegalFramework::Adversarial),
            relay("wss://r1.ch", "CH", "Europe", LegalFramework::StrongPrivacy),
            relay("wss://r1.de", "DE", "Europe", LegalFramework::Moderate),
        ];
        let diversity = JurisdictionAnalyzer::analyze(&relays);
        assert!(!diversity.adversarial_only);
    }

    #[test]
    fn analyze_all_unknown_not_adversarial() {
        let relays = vec![
            RelayJurisdiction::unknown("wss://r1"),
            RelayJurisdiction::unknown("wss://r2"),
        ];
        let diversity = JurisdictionAnalyzer::analyze(&relays);
        // All unknown = adversarial_only should be false (no known frameworks)
        assert!(!diversity.adversarial_only);
    }

    #[test]
    fn analyze_unknown_with_adversarial() {
        // One unknown (no framework set) + one adversarial
        let relays = vec![
            RelayJurisdiction::unknown("wss://r1"),
            relay("wss://r2.cn", "CN", "Asia", LegalFramework::Adversarial),
        ];
        let diversity = JurisdictionAnalyzer::analyze(&relays);
        // Only one known framework and it's adversarial
        assert!(diversity.adversarial_only);
    }

    #[test]
    fn analyze_three_even_jurisdictions_healthy() {
        let relays = vec![
            relay("wss://ch.relay", "CH", "Europe", LegalFramework::StrongPrivacy),
            relay("wss://de.relay", "DE", "Europe", LegalFramework::Moderate),
            relay("wss://jp.relay", "JP", "Asia", LegalFramework::Moderate),
        ];
        let diversity = JurisdictionAnalyzer::analyze(&relays);
        assert_eq!(diversity.unique_jurisdictions, 3);
        // 3 countries, 1 each: D = 0, score = 1.0
        assert!((diversity.diversity_score - 1.0).abs() < f64::EPSILON);
        assert_eq!(diversity.recommendation, DiversityRecommendation::Healthy);
    }

    #[test]
    fn analyze_two_jurisdictions_even_suggests_more() {
        let relays = vec![
            relay("wss://ch.relay", "CH", "Europe", LegalFramework::StrongPrivacy),
            relay("wss://de.relay", "DE", "Europe", LegalFramework::Moderate),
        ];
        let diversity = JurisdictionAnalyzer::analyze(&relays);
        assert_eq!(diversity.unique_jurisdictions, 2);
        // 2 countries, 1 each: D = 0, score = 1.0 — but only 2 jurisdictions
        assert!(matches!(
            diversity.recommendation,
            DiversityRecommendation::AddDiversity(_)
        ));
    }

    #[test]
    fn analyze_low_diversity_score_critical() {
        // 10 relays in US, 1 in CH
        let mut relays: Vec<RelayJurisdiction> = (0..10)
            .map(|i| {
                relay(
                    &format!("wss://r{i}.us"),
                    "US",
                    "North America",
                    LegalFramework::Weak,
                )
            })
            .collect();
        relays.push(relay(
            "wss://r1.ch",
            "CH",
            "Europe",
            LegalFramework::StrongPrivacy,
        ));

        let diversity = JurisdictionAnalyzer::analyze(&relays);
        // D = (10*9)/(11*10) = 90/110 ≈ 0.818, score ≈ 0.182
        assert!(diversity.diversity_score < 0.3);
        assert!(matches!(
            diversity.recommendation,
            DiversityRecommendation::CriticallyHomogeneous(_)
        ));
    }

    #[test]
    fn diversity_score_serde_roundtrip() {
        let relays = vec![
            relay("wss://ch.relay", "CH", "Europe", LegalFramework::StrongPrivacy),
            relay("wss://us.relay", "US", "North America", LegalFramework::Weak),
        ];
        let diversity = JurisdictionAnalyzer::analyze(&relays);
        let json = serde_json::to_string(&diversity).unwrap();
        let loaded: JurisdictionDiversity = serde_json::from_str(&json).unwrap();
        assert_eq!(diversity.connected_relays, loaded.connected_relays);
        assert_eq!(diversity.unique_jurisdictions, loaded.unique_jurisdictions);
        assert!((diversity.diversity_score - loaded.diversity_score).abs() < f64::EPSILON);
        assert_eq!(diversity.adversarial_only, loaded.adversarial_only);
        assert_eq!(diversity.recommendation, loaded.recommendation);
    }

    #[test]
    fn recommendation_serde_roundtrip() {
        let recs = vec![
            DiversityRecommendation::Healthy,
            DiversityRecommendation::AddDiversity("add more".into()),
            DiversityRecommendation::CriticallyHomogeneous("bad".into()),
        ];
        for rec in &recs {
            let json = serde_json::to_string(rec).unwrap();
            let loaded: DiversityRecommendation = serde_json::from_str(&json).unwrap();
            assert_eq!(*rec, loaded);
        }
    }

    #[test]
    fn simpsons_index_known_values() {
        // 2 species, equal distribution: 1 - 0 = 1.0
        let mut counts = HashMap::new();
        counts.insert("A".to_string(), 1);
        counts.insert("B".to_string(), 1);
        let score = JurisdictionAnalyzer::simpsons_index(&counts, 2);
        assert!((score - 1.0).abs() < f64::EPSILON);

        // 1 species, all same: 1 - 1 = 0.0
        let mut counts2 = HashMap::new();
        counts2.insert("A".to_string(), 5);
        let score2 = JurisdictionAnalyzer::simpsons_index(&counts2, 5);
        assert!((score2 - 0.0).abs() < f64::EPSILON);
    }
}
