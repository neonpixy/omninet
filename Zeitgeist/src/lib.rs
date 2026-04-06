//! Zeitgeist — Discovery & culture for the Omnidea network.
//!
//! The user-facing discovery engine. Runs on the user's device, not on Towers.
//!
//! # Architecture
//!
//! - `TowerDirectory` — built from gospel (Semantic Profiles + lighthouse announcements)
//! - `QueryRouter` — smart routing via keyword/capability matching to select the best Towers
//! - `ResultMerger` — dedup by event ID, rank by relevance, progressive result merging
//! - `LocalCache` — personal index of previously seen results
//! - `DiscoveryProvider` trait — pluggable discovery strategies
//! - `Trending` — aggregate signals from Tower responses and Semantic Profiles
//!
//! Zeitgeist reads the gospel directory, picks the right Towers for each query,
//! queries them directly in parallel, and merges the results.

pub mod cache;
pub mod directory;
pub mod error;
pub mod federation_scope;
pub mod global_trends;
pub mod merger;
pub mod router;
pub mod traits;
pub mod trending;

pub use cache::LocalCache;
pub use directory::{TowerDirectory, TowerEntry};
pub use error::ZeitgeistError;
pub use federation_scope::FederationScope;
pub use merger::ResultMerger;
pub use router::QueryRouter;
pub use traits::DiscoveryProvider;
pub use trending::{TrendSignal, TrendTracker};
pub use global_trends::{
    CommunityTrendView, GlobalTrend, GlobalTrendTracker, TrendPerspective, TrendSentiment,
    ZeitgeistScope,
};
