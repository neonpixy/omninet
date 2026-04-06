pub mod types;
pub mod pattern;
pub mod review;
pub mod warning;
pub mod weaponization;

pub use types::{
    AccountabilityFlag, FlagCategory, FlagContext, FlagReviewStatus, FlagSeverity,
};
pub use pattern::{
    detect_all_patterns, detect_all_patterns_scoped, detect_pattern, detect_pattern_scoped,
    FlagPattern,
};
pub use review::{CommunityAction, FlagReview, ReviewOutcome};
pub use warning::{DutyToWarn, PatternSummary, WarningRecord};
pub use weaponization::{
    check_rate_limit, detect_coordinated_campaign, detect_repetitive_grounds,
    detect_serial_filing, recommend_consequence, AbuseConsequence, AbuseIndicator, AbusePattern,
};
