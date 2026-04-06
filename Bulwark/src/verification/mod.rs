pub mod method;
pub mod proximity;
pub mod sponsor;
pub mod vouch;

pub use method::{
    VerificationEvidence, VerificationMethod, VerificationResult,
};
pub use proximity::{ProximityBond, ProximityMethod, ProximityProof};
pub use sponsor::{SponsorEligibility, Sponsorship, SponsorshipStatus};
pub use vouch::{MutualVouch, VouchDiversityCheck, VouchEligibility, VouchRule, VouchRules};
