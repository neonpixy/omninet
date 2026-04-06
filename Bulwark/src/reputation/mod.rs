pub mod consequence;
pub mod fraud;
pub mod score;

pub use consequence::{Consequence, ConsequenceDuration, ConsequenceType};
pub use fraud::{FraudIndicator, FraudSeverity, RiskRecommendation, RiskScore, SuspiciousPattern, SuspiciousPatternType};
pub use score::{Reputation, ReputationEvent, ReputationEventType, ReputationFactors, Standing};
