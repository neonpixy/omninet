//! # Minimum AI Capability Guarantee (R6A)
//!
//! Defines the minimum capabilities that local (on-device) AI must provide
//! to participate as an Advisor. This is about what the model can DO, not
//! which model it is. Any model that passes the benchmarks qualifies.
//!
//! When no available provider meets the floor, Advisor falls back to
//! `DeferToHuman` mode for governance — better to abstain than to vote poorly.
//!
//! # Covenant Alignment
//!
//! **Dignity** — AI equity means every participant gets meaningful AI assistance,
//! regardless of their hardware. The floor ensures a baseline.
//! **Sovereignty** — users choose their model. The floor tests capability, not brand.
//! **Consent** — DeferToHuman is consent-respecting: silence over bad advice.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── MinimumCapabilities ──────────────────────────────────────────────

bitflags::bitflags! {
    /// The minimum capabilities an AI provider must demonstrate to serve
    /// as an Advisor. Each flag represents a domain of competence.
    ///
    /// # Example
    ///
    /// ```
    /// use advisor::capability_floor::MinimumCapabilities;
    ///
    /// let required = MinimumCapabilities::TEXT_EDITING | MinimumCapabilities::GOVERNANCE_REASONING;
    /// let provider_has = MinimumCapabilities::all();
    /// assert!(provider_has.contains(required));
    /// ```
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct MinimumCapabilities: u32 {
        /// Grammar, tone, clarity suggestions for Quill/Tome.
        const TEXT_EDITING          = 0b0000_0001;
        /// Layout, color, typography recommendations for Studio.
        const DESIGN_SUGGESTION     = 0b0000_0010;
        /// Verify .idea content meets accessibility requirements
        /// (alt text, contrast, focus order).
        const ACCESSIBILITY_CHECK   = 0b0000_0100;
        /// Basic spreadsheet analysis for Abacus (trends, outliers, summaries).
        const DATA_ANALYSIS         = 0b0000_1000;
        /// Lingo-powered translation assistance.
        const TRANSLATION           = 0b0001_0000;
        /// Proposal analysis for Advisor delegation (R1D). The critical one:
        /// governance participation must work well on local models, or AI equity fails.
        const GOVERNANCE_REASONING  = 0b0010_0000;
        /// Help formulate MagicalIndex queries.
        const SEARCH_ASSISTANCE     = 0b0100_0000;
    }
}

impl MinimumCapabilities {
    /// Check if all required capabilities are met.
    pub fn satisfies(&self, required: MinimumCapabilities) -> bool {
        self.contains(required)
    }

    /// Returns the capabilities present in `required` but missing from `self`.
    pub fn missing_from(&self, required: MinimumCapabilities) -> MinimumCapabilities {
        required & !*self
    }

    /// The full floor — all capabilities required for unrestricted Advisor operation.
    pub fn full_floor() -> MinimumCapabilities {
        MinimumCapabilities::all()
    }

    /// The governance floor — minimum for participating in governance votes.
    pub fn governance_floor() -> MinimumCapabilities {
        MinimumCapabilities::GOVERNANCE_REASONING | MinimumCapabilities::TEXT_EDITING
    }

    /// Human-readable names for each capability flag.
    pub fn capability_names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.contains(MinimumCapabilities::TEXT_EDITING) {
            names.push("text_editing");
        }
        if self.contains(MinimumCapabilities::DESIGN_SUGGESTION) {
            names.push("design_suggestion");
        }
        if self.contains(MinimumCapabilities::ACCESSIBILITY_CHECK) {
            names.push("accessibility_check");
        }
        if self.contains(MinimumCapabilities::DATA_ANALYSIS) {
            names.push("data_analysis");
        }
        if self.contains(MinimumCapabilities::TRANSLATION) {
            names.push("translation");
        }
        if self.contains(MinimumCapabilities::GOVERNANCE_REASONING) {
            names.push("governance_reasoning");
        }
        if self.contains(MinimumCapabilities::SEARCH_ASSISTANCE) {
            names.push("search_assistance");
        }
        names
    }
}

impl std::fmt::Display for MinimumCapabilities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names = self.capability_names();
        if names.is_empty() {
            write!(f, "(none)")
        } else {
            write!(f, "{}", names.join(", "))
        }
    }
}

// ── CapabilityAssessment ─────────────────────────────────────────────

/// The result of assessing a provider against the minimum capability floor.
///
/// Captures what a provider can and cannot do, so the system can route
/// appropriately (e.g., defer governance to human if GOVERNANCE_REASONING
/// is missing).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityAssessment {
    /// Which provider was assessed.
    pub provider_id: String,
    /// Capabilities that met the floor.
    pub capabilities_met: MinimumCapabilities,
    /// Capabilities that did not meet the floor.
    pub capabilities_missing: MinimumCapabilities,
    /// When this assessment was performed.
    pub assessment_date: DateTime<Utc>,
    /// Human-readable model info (e.g., "llama-3.2-3b", "claude-3-haiku").
    pub model_info: String,
}

impl CapabilityAssessment {
    /// Create a new assessment.
    pub fn new(
        provider_id: impl Into<String>,
        capabilities_met: MinimumCapabilities,
        capabilities_missing: MinimumCapabilities,
        model_info: impl Into<String>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            capabilities_met,
            capabilities_missing,
            assessment_date: Utc::now(),
            model_info: model_info.into(),
        }
    }

    /// Whether this provider meets the full capability floor.
    pub fn meets_full_floor(&self) -> bool {
        self.capabilities_missing.is_empty()
    }

    /// Whether this provider can participate in governance.
    pub fn meets_governance_floor(&self) -> bool {
        self.capabilities_met
            .satisfies(MinimumCapabilities::governance_floor())
    }

    /// Whether a specific capability is met.
    pub fn has_capability(&self, cap: MinimumCapabilities) -> bool {
        self.capabilities_met.contains(cap)
    }
}

// ── BenchmarkResult ──────────────────────────────────────────────────

/// The result of running a single capability benchmark against a provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BenchmarkResult {
    /// Whether the provider passed this benchmark.
    pub passed: bool,
    /// Numeric score (0.0 to 1.0). Passing threshold varies by benchmark.
    pub score: f64,
    /// Human-readable details about the benchmark outcome.
    pub details: String,
}

impl BenchmarkResult {
    /// Create a passing result.
    pub fn pass(score: f64, details: impl Into<String>) -> Self {
        Self {
            passed: true,
            score: score.clamp(0.0, 1.0),
            details: details.into(),
        }
    }

    /// Create a failing result.
    pub fn fail(score: f64, details: impl Into<String>) -> Self {
        Self {
            passed: false,
            score: score.clamp(0.0, 1.0),
            details: details.into(),
        }
    }
}

// ── CapabilityBenchmark ──────────────────────────────────────────────

/// Standardized benchmarks for assessing provider capabilities.
///
/// Each benchmark method accepts a provider's responses to standardized
/// test cases and returns a `BenchmarkResult`. The passing threshold
/// varies by capability (governance reasoning is stricter than search).
///
/// These benchmarks run offline against pre-generated test data — they
/// do NOT call the provider at benchmark time. The platform layer
/// generates the provider's responses; this module scores them.
pub struct CapabilityBenchmark;

/// Default passing thresholds per capability.
const THRESHOLD_TEXT_EDITING: f64 = 0.6;
const THRESHOLD_DESIGN_SUGGESTION: f64 = 0.5;
const THRESHOLD_ACCESSIBILITY_CHECK: f64 = 0.7;
const THRESHOLD_DATA_ANALYSIS: f64 = 0.6;
const THRESHOLD_TRANSLATION: f64 = 0.6;
const THRESHOLD_GOVERNANCE_REASONING: f64 = 0.75;
const THRESHOLD_SEARCH_ASSISTANCE: f64 = 0.5;

impl CapabilityBenchmark {
    /// Benchmark text editing capability.
    ///
    /// Tests grammar correction, tone adjustment, and clarity improvement.
    /// Score is the fraction of test cases where the provider produced
    /// acceptable edits.
    pub fn benchmark_text_editing(correct: usize, total: usize) -> BenchmarkResult {
        Self::score(correct, total, THRESHOLD_TEXT_EDITING, "text_editing")
    }

    /// Benchmark design suggestion capability.
    ///
    /// Tests layout, color, and typography recommendations.
    pub fn benchmark_design_suggestion(correct: usize, total: usize) -> BenchmarkResult {
        Self::score(correct, total, THRESHOLD_DESIGN_SUGGESTION, "design_suggestion")
    }

    /// Benchmark accessibility checking capability.
    ///
    /// Tests alt text detection, contrast verification, focus order analysis.
    /// Higher threshold — accessibility errors have real impact.
    pub fn benchmark_accessibility_check(correct: usize, total: usize) -> BenchmarkResult {
        Self::score(
            correct,
            total,
            THRESHOLD_ACCESSIBILITY_CHECK,
            "accessibility_check",
        )
    }

    /// Benchmark data analysis capability.
    ///
    /// Tests trend detection, outlier identification, summary generation.
    pub fn benchmark_data_analysis(correct: usize, total: usize) -> BenchmarkResult {
        Self::score(correct, total, THRESHOLD_DATA_ANALYSIS, "data_analysis")
    }

    /// Benchmark translation capability.
    pub fn benchmark_translation(correct: usize, total: usize) -> BenchmarkResult {
        Self::score(correct, total, THRESHOLD_TRANSLATION, "translation")
    }

    /// Benchmark governance reasoning capability.
    ///
    /// Tests proposal analysis, value alignment detection, tradeoff articulation.
    /// Strictest threshold — governance votes affect real communities.
    pub fn benchmark_governance_reasoning(correct: usize, total: usize) -> BenchmarkResult {
        Self::score(
            correct,
            total,
            THRESHOLD_GOVERNANCE_REASONING,
            "governance_reasoning",
        )
    }

    /// Benchmark search assistance capability.
    ///
    /// Tests query formulation and refinement.
    pub fn benchmark_search_assistance(correct: usize, total: usize) -> BenchmarkResult {
        Self::score(
            correct,
            total,
            THRESHOLD_SEARCH_ASSISTANCE,
            "search_assistance",
        )
    }

    /// Run all benchmarks from a set of per-capability (correct, total) pairs.
    ///
    /// Returns a `CapabilityAssessment` summarizing which capabilities passed.
    pub fn assess_provider(
        provider_id: impl Into<String>,
        model_info: impl Into<String>,
        results: &[(MinimumCapabilities, usize, usize)],
    ) -> CapabilityAssessment {
        let mut met = MinimumCapabilities::empty();
        let mut missing = MinimumCapabilities::empty();

        for &(cap, correct, total) in results {
            let result = match cap {
                MinimumCapabilities::TEXT_EDITING => Self::benchmark_text_editing(correct, total),
                MinimumCapabilities::DESIGN_SUGGESTION => {
                    Self::benchmark_design_suggestion(correct, total)
                }
                MinimumCapabilities::ACCESSIBILITY_CHECK => {
                    Self::benchmark_accessibility_check(correct, total)
                }
                MinimumCapabilities::DATA_ANALYSIS => Self::benchmark_data_analysis(correct, total),
                MinimumCapabilities::TRANSLATION => Self::benchmark_translation(correct, total),
                MinimumCapabilities::GOVERNANCE_REASONING => {
                    Self::benchmark_governance_reasoning(correct, total)
                }
                MinimumCapabilities::SEARCH_ASSISTANCE => {
                    Self::benchmark_search_assistance(correct, total)
                }
                _ => continue,
            };

            if result.passed {
                met |= cap;
            } else {
                missing |= cap;
            }
        }

        // Any capabilities not tested are considered missing.
        let untested = MinimumCapabilities::all() & !(met | missing);
        missing |= untested;

        CapabilityAssessment::new(provider_id, met, missing, model_info)
    }

    /// Internal scoring helper.
    fn score(correct: usize, total: usize, threshold: f64, name: &str) -> BenchmarkResult {
        if total == 0 {
            return BenchmarkResult::fail(0.0, format!("{name}: no test cases provided"));
        }

        let score = correct as f64 / total as f64;
        if score >= threshold {
            BenchmarkResult::pass(
                score,
                format!("{name}: {correct}/{total} ({:.1}%) >= {:.0}% threshold", score * 100.0, threshold * 100.0),
            )
        } else {
            BenchmarkResult::fail(
                score,
                format!("{name}: {correct}/{total} ({:.1}%) < {:.0}% threshold", score * 100.0, threshold * 100.0),
            )
        }
    }
}

// ── DeferToHuman ─────────────────────────────────────────────────────

/// Fallback mode when no provider meets the capability floor.
///
/// When `should_defer_governance` returns true, the Advisor presents
/// the proposal summary but does NOT vote. Better to abstain than
/// to vote poorly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeferToHuman {
    /// Why the Advisor is deferring.
    pub reason: String,
    /// Which capabilities were missing.
    pub missing_capabilities: MinimumCapabilities,
}

impl DeferToHuman {
    /// Create a governance deferral.
    pub fn governance(missing: MinimumCapabilities) -> Self {
        Self {
            reason: format!(
                "No provider meets governance floor. Missing: {}",
                missing
            ),
            missing_capabilities: missing,
        }
    }
}

/// Check whether governance should be deferred to the human.
///
/// Returns `Some(DeferToHuman)` if no assessment in the list meets
/// the governance floor.
pub fn should_defer_governance(assessments: &[CapabilityAssessment]) -> Option<DeferToHuman> {
    if assessments.iter().any(|a| a.meets_governance_floor()) {
        None
    } else {
        // Report the best provider's missing capabilities.
        let best = assessments.iter().max_by_key(|a| a.capabilities_met.bits());
        let missing = best
            .map(|a| a.capabilities_met.missing_from(MinimumCapabilities::governance_floor()))
            .unwrap_or(MinimumCapabilities::governance_floor());
        Some(DeferToHuman::governance(missing))
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // --- MinimumCapabilities flag operations ---

    #[test]
    fn capability_flags_basic() {
        let caps =
            MinimumCapabilities::TEXT_EDITING | MinimumCapabilities::GOVERNANCE_REASONING;
        assert!(caps.contains(MinimumCapabilities::TEXT_EDITING));
        assert!(caps.contains(MinimumCapabilities::GOVERNANCE_REASONING));
        assert!(!caps.contains(MinimumCapabilities::TRANSLATION));
    }

    #[test]
    fn satisfies_check() {
        let has = MinimumCapabilities::all();
        let needs = MinimumCapabilities::TEXT_EDITING | MinimumCapabilities::TRANSLATION;
        assert!(has.satisfies(needs));
    }

    #[test]
    fn satisfies_partial_fails() {
        let has = MinimumCapabilities::TEXT_EDITING;
        let needs = MinimumCapabilities::TEXT_EDITING | MinimumCapabilities::TRANSLATION;
        assert!(!has.satisfies(needs));
    }

    #[test]
    fn missing_from_computes_diff() {
        let has = MinimumCapabilities::TEXT_EDITING | MinimumCapabilities::DATA_ANALYSIS;
        let required = MinimumCapabilities::TEXT_EDITING
            | MinimumCapabilities::DATA_ANALYSIS
            | MinimumCapabilities::TRANSLATION;
        let missing = has.missing_from(required);
        assert_eq!(missing, MinimumCapabilities::TRANSLATION);
    }

    #[test]
    fn missing_from_empty_when_all_met() {
        let has = MinimumCapabilities::all();
        let missing = has.missing_from(MinimumCapabilities::all());
        assert!(missing.is_empty());
    }

    #[test]
    fn full_floor_includes_all() {
        let floor = MinimumCapabilities::full_floor();
        assert!(floor.contains(MinimumCapabilities::TEXT_EDITING));
        assert!(floor.contains(MinimumCapabilities::GOVERNANCE_REASONING));
        assert!(floor.contains(MinimumCapabilities::SEARCH_ASSISTANCE));
    }

    #[test]
    fn governance_floor_subset() {
        let gov = MinimumCapabilities::governance_floor();
        assert!(gov.contains(MinimumCapabilities::GOVERNANCE_REASONING));
        assert!(gov.contains(MinimumCapabilities::TEXT_EDITING));
        assert!(!gov.contains(MinimumCapabilities::DESIGN_SUGGESTION));
    }

    #[test]
    fn capability_names_correct() {
        let caps = MinimumCapabilities::TEXT_EDITING | MinimumCapabilities::TRANSLATION;
        let names = caps.capability_names();
        assert_eq!(names, vec!["text_editing", "translation"]);
    }

    #[test]
    fn display_format() {
        let caps = MinimumCapabilities::TEXT_EDITING;
        assert_eq!(format!("{caps}"), "text_editing");

        let empty = MinimumCapabilities::empty();
        assert_eq!(format!("{empty}"), "(none)");
    }

    #[test]
    fn serde_round_trip() {
        let caps = MinimumCapabilities::TEXT_EDITING
            | MinimumCapabilities::GOVERNANCE_REASONING
            | MinimumCapabilities::SEARCH_ASSISTANCE;
        let json = serde_json::to_string(&caps).unwrap();
        let restored: MinimumCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps, restored);
    }

    // --- CapabilityAssessment ---

    #[test]
    fn assessment_meets_full_floor() {
        let assessment = CapabilityAssessment::new(
            "local-llama",
            MinimumCapabilities::all(),
            MinimumCapabilities::empty(),
            "llama-3.2-70b",
        );
        assert!(assessment.meets_full_floor());
        assert!(assessment.meets_governance_floor());
    }

    #[test]
    fn assessment_missing_governance() {
        let met = MinimumCapabilities::TEXT_EDITING | MinimumCapabilities::DESIGN_SUGGESTION;
        let missing = MinimumCapabilities::all() & !met;
        let assessment = CapabilityAssessment::new("tiny-model", met, missing, "phi-2");
        assert!(!assessment.meets_full_floor());
        assert!(!assessment.meets_governance_floor());
    }

    #[test]
    fn assessment_partial_governance_floor() {
        // Has governance reasoning but not text editing — fails governance floor.
        let met = MinimumCapabilities::GOVERNANCE_REASONING;
        let missing = MinimumCapabilities::all() & !met;
        let assessment = CapabilityAssessment::new("model-x", met, missing, "model-x");
        assert!(!assessment.meets_governance_floor());
    }

    #[test]
    fn assessment_has_capability() {
        let assessment = CapabilityAssessment::new(
            "test",
            MinimumCapabilities::TEXT_EDITING,
            MinimumCapabilities::all() & !MinimumCapabilities::TEXT_EDITING,
            "test-model",
        );
        assert!(assessment.has_capability(MinimumCapabilities::TEXT_EDITING));
        assert!(!assessment.has_capability(MinimumCapabilities::TRANSLATION));
    }

    #[test]
    fn assessment_serde() {
        let assessment = CapabilityAssessment::new(
            "provider-1",
            MinimumCapabilities::TEXT_EDITING | MinimumCapabilities::DATA_ANALYSIS,
            MinimumCapabilities::GOVERNANCE_REASONING,
            "test-model-7b",
        );
        let json = serde_json::to_string(&assessment).unwrap();
        let restored: CapabilityAssessment = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.provider_id, "provider-1");
        assert!(restored
            .capabilities_met
            .contains(MinimumCapabilities::TEXT_EDITING));
    }

    // --- BenchmarkResult ---

    #[test]
    fn benchmark_result_pass() {
        let result = BenchmarkResult::pass(0.85, "good");
        assert!(result.passed);
        assert!((result.score - 0.85).abs() < 0.001);
    }

    #[test]
    fn benchmark_result_fail() {
        let result = BenchmarkResult::fail(0.3, "below threshold");
        assert!(!result.passed);
    }

    #[test]
    fn benchmark_result_score_clamped() {
        let result = BenchmarkResult::pass(1.5, "over max");
        assert!((result.score - 1.0).abs() < 0.001);

        let result = BenchmarkResult::fail(-0.5, "under min");
        assert!((result.score - 0.0).abs() < 0.001);
    }

    #[test]
    fn benchmark_result_serde() {
        let result = BenchmarkResult::pass(0.9, "excellent");
        let json = serde_json::to_string(&result).unwrap();
        let restored: BenchmarkResult = serde_json::from_str(&json).unwrap();
        assert!(restored.passed);
        assert!((restored.score - 0.9).abs() < 0.001);
    }

    // --- CapabilityBenchmark scoring ---

    #[test]
    fn benchmark_text_editing_pass() {
        let result = CapabilityBenchmark::benchmark_text_editing(7, 10);
        assert!(result.passed); // 0.7 >= 0.6
    }

    #[test]
    fn benchmark_text_editing_fail() {
        let result = CapabilityBenchmark::benchmark_text_editing(5, 10);
        assert!(!result.passed); // 0.5 < 0.6
    }

    #[test]
    fn benchmark_governance_strict_threshold() {
        // Governance requires 0.75 — stricter than most.
        let result = CapabilityBenchmark::benchmark_governance_reasoning(7, 10);
        assert!(!result.passed); // 0.7 < 0.75

        let result = CapabilityBenchmark::benchmark_governance_reasoning(8, 10);
        assert!(result.passed); // 0.8 >= 0.75
    }

    #[test]
    fn benchmark_zero_total_fails() {
        let result = CapabilityBenchmark::benchmark_text_editing(0, 0);
        assert!(!result.passed);
        assert!((result.score - 0.0).abs() < 0.001);
    }

    #[test]
    fn assess_provider_all_pass() {
        let results = vec![
            (MinimumCapabilities::TEXT_EDITING, 8, 10),
            (MinimumCapabilities::DESIGN_SUGGESTION, 6, 10),
            (MinimumCapabilities::ACCESSIBILITY_CHECK, 8, 10),
            (MinimumCapabilities::DATA_ANALYSIS, 7, 10),
            (MinimumCapabilities::TRANSLATION, 7, 10),
            (MinimumCapabilities::GOVERNANCE_REASONING, 9, 10),
            (MinimumCapabilities::SEARCH_ASSISTANCE, 6, 10),
        ];
        let assessment =
            CapabilityBenchmark::assess_provider("local-llama", "llama-3.2-70b", &results);
        assert!(assessment.meets_full_floor());
        assert!(assessment.capabilities_missing.is_empty());
    }

    #[test]
    fn assess_provider_partial_pass() {
        let results = vec![
            (MinimumCapabilities::TEXT_EDITING, 8, 10),
            (MinimumCapabilities::GOVERNANCE_REASONING, 5, 10), // fails: 0.5 < 0.75
        ];
        let assessment =
            CapabilityBenchmark::assess_provider("small-model", "phi-3-mini", &results);
        assert!(!assessment.meets_full_floor());
        assert!(assessment
            .capabilities_met
            .contains(MinimumCapabilities::TEXT_EDITING));
        assert!(assessment
            .capabilities_missing
            .contains(MinimumCapabilities::GOVERNANCE_REASONING));
        // Untested capabilities are also missing.
        assert!(assessment
            .capabilities_missing
            .contains(MinimumCapabilities::TRANSLATION));
    }

    // --- DeferToHuman ---

    #[test]
    fn defer_to_human_governance() {
        let defer = DeferToHuman::governance(MinimumCapabilities::GOVERNANCE_REASONING);
        assert!(defer.reason.contains("governance"));
        assert!(defer
            .missing_capabilities
            .contains(MinimumCapabilities::GOVERNANCE_REASONING));
    }

    #[test]
    fn should_defer_when_no_providers() {
        let result = should_defer_governance(&[]);
        assert!(result.is_some());
    }

    #[test]
    fn should_defer_when_none_meet_floor() {
        let assessments = vec![CapabilityAssessment::new(
            "weak-model",
            MinimumCapabilities::DESIGN_SUGGESTION,
            MinimumCapabilities::all() & !MinimumCapabilities::DESIGN_SUGGESTION,
            "tiny-model",
        )];
        let result = should_defer_governance(&assessments);
        assert!(result.is_some());
    }

    #[test]
    fn should_not_defer_when_provider_meets_floor() {
        let gov_floor = MinimumCapabilities::governance_floor();
        let assessments = vec![CapabilityAssessment::new(
            "good-model",
            gov_floor,
            MinimumCapabilities::all() & !gov_floor,
            "llama-3.2-7b",
        )];
        let result = should_defer_governance(&assessments);
        assert!(result.is_none());
    }

    #[test]
    fn defer_serde() {
        let defer = DeferToHuman::governance(MinimumCapabilities::GOVERNANCE_REASONING);
        let json = serde_json::to_string(&defer).unwrap();
        let restored: DeferToHuman = serde_json::from_str(&json).unwrap();
        assert!(restored
            .missing_capabilities
            .contains(MinimumCapabilities::GOVERNANCE_REASONING));
    }
}
