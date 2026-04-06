//! Progressive Web App hardening for Divinity/Web.
//!
//! Defines capability flags, tier-based capability matrices, service worker
//! strategies, and offline cache management for resilient browser-based
//! access to the Omnidea protocol.
//!
//! # Architecture
//!
//! The PWA layer sits between the browser environment and the Rust core.
//! It determines which protocol capabilities are available based on the
//! participant's sovereignty tier, manages offline caching of `.idea` files,
//! and queues operations for sync when connectivity returns.
//!
//! # Capability Model
//!
//! Not every browser session gets full protocol access. [`WebCapabilities`]
//! are bitflags gating what the PWA can do. [`WebCapabilityMatrix`] maps
//! each [`SovereigntyTier`] to the appropriate capability set:
//!
//! - **Sheltered / Citizen**: Rendering + Vault + Network (consumption + basic creation)
//! - **Steward**: adds Editing + Offline (full creation, offline capability)
//! - **Architect**: adds Crypto (full protocol participation including signing)
//!
//! # No Chromium Dependency
//!
//! `.idea` content is rendered via Magic -> DOM translation. Chromium is only
//! needed for legacy HTML in Scry/Browser. The PWA can be a standalone
//! `.idea` viewer/editor without any browser engine dependency.

use oracle::SovereigntyTier;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

// ---------------------------------------------------------------------------
// WebCapabilities (bitflags)
// ---------------------------------------------------------------------------

/// Capability flags for the PWA environment.
///
/// Each flag gates a category of protocol functionality in the browser.
/// Implemented as a newtype over `u32` with bitwise operations — no
/// external `bitflags` dependency needed.
///
/// # Examples
///
/// ```
/// use divinity_web::pwa::WebCapabilities;
///
/// let caps = WebCapabilities::RENDERING | WebCapabilities::VAULT;
/// assert!(caps.contains(WebCapabilities::RENDERING));
/// assert!(!caps.contains(WebCapabilities::CRYPTO));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WebCapabilities(u32);

impl WebCapabilities {
    /// No capabilities.
    pub const NONE: Self = Self(0);

    /// Magic -> DOM rendering of `.idea` files.
    pub const RENDERING: Self = Self(1 << 0);

    /// Basic Throne editing (create and modify `.idea` content).
    pub const EDITING: Self = Self(1 << 1);

    /// IndexedDB-encrypted storage (local Vault).
    pub const VAULT: Self = Self(1 << 2);

    /// Globe WebSocket connectivity.
    pub const NETWORK: Self = Self(1 << 3);

    /// Service Worker + local cache for offline access.
    pub const OFFLINE: Self = Self(1 << 4);

    /// WebCrypto API for Sentinal operations (signing, verification).
    pub const CRYPTO: Self = Self(1 << 5);

    /// All capabilities enabled.
    pub const ALL: Self = Self(
        Self::RENDERING.0
            | Self::EDITING.0
            | Self::VAULT.0
            | Self::NETWORK.0
            | Self::OFFLINE.0
            | Self::CRYPTO.0,
    );

    /// Returns `true` if `self` contains all flags in `other`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Returns `true` if no flags are set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns the raw `u32` representation.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Creates capabilities from a raw `u32`.
    ///
    /// Bits outside the known flags are silently masked off.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits & Self::ALL.0)
    }

    /// Returns `true` if any flags in `other` are set in `self`.
    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    /// Returns the number of enabled capabilities.
    #[must_use]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }
}

impl std::ops::BitOr for WebCapabilities {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for WebCapabilities {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for WebCapabilities {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::BitAndAssign for WebCapabilities {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl std::ops::Not for WebCapabilities {
    type Output = Self;
    fn not(self) -> Self {
        // Mask to known bits only.
        Self(!self.0 & Self::ALL.0)
    }
}

impl fmt::Debug for WebCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut flags = Vec::new();
        if self.contains(Self::RENDERING) {
            flags.push("RENDERING");
        }
        if self.contains(Self::EDITING) {
            flags.push("EDITING");
        }
        if self.contains(Self::VAULT) {
            flags.push("VAULT");
        }
        if self.contains(Self::NETWORK) {
            flags.push("NETWORK");
        }
        if self.contains(Self::OFFLINE) {
            flags.push("OFFLINE");
        }
        if self.contains(Self::CRYPTO) {
            flags.push("CRYPTO");
        }
        if flags.is_empty() {
            write!(f, "WebCapabilities(NONE)")
        } else {
            write!(f, "WebCapabilities({})", flags.join(" | "))
        }
    }
}

impl fmt::Display for WebCapabilities {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

// ---------------------------------------------------------------------------
// WebCapabilityMatrix
// ---------------------------------------------------------------------------

/// Maps sovereignty tiers to web capabilities.
///
/// The matrix defines what each tier can do in the PWA. The default
/// matrix follows the R5C specification:
///
/// | Tier | Capabilities |
/// |------|-------------|
/// | Sheltered | Rendering, Vault, Network |
/// | Citizen | Rendering, Vault, Network |
/// | Steward | Rendering, Editing, Vault, Network, Offline |
/// | Architect | All (Rendering, Editing, Vault, Network, Offline, Crypto) |
///
/// Custom matrices can be created via [`WebCapabilityMatrix::new`] for
/// testing or special deployments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WebCapabilityMatrix {
    tiers: HashMap<TierKey, WebCapabilities>,
}

/// Internal key type for the tier map, since `SovereigntyTier` already
/// derives `Hash` and `Eq`.
type TierKey = SovereigntyTier;

impl WebCapabilityMatrix {
    /// Creates a new empty matrix.
    #[must_use]
    pub fn new() -> Self {
        Self {
            tiers: HashMap::new(),
        }
    }

    /// Creates the default matrix per the R5C specification.
    ///
    /// - Sheltered/Citizen: Rendering | Vault | Network
    /// - Steward: + Editing | Offline
    /// - Architect: + Crypto
    #[must_use]
    pub fn default_matrix() -> Self {
        let base = WebCapabilities::RENDERING | WebCapabilities::VAULT | WebCapabilities::NETWORK;
        let steward = base | WebCapabilities::EDITING | WebCapabilities::OFFLINE;
        let architect = steward | WebCapabilities::CRYPTO;

        let mut tiers = HashMap::new();
        tiers.insert(SovereigntyTier::Sheltered, base);
        tiers.insert(SovereigntyTier::Citizen, base);
        tiers.insert(SovereigntyTier::Steward, steward);
        tiers.insert(SovereigntyTier::Architect, architect);

        Self { tiers }
    }

    /// Sets the capabilities for a specific tier.
    pub fn set(&mut self, tier: SovereigntyTier, capabilities: WebCapabilities) {
        self.tiers.insert(tier, capabilities);
    }

    /// Returns the capabilities for a tier, or [`WebCapabilities::NONE`] if
    /// the tier has no entry.
    #[must_use]
    pub fn get(&self, tier: SovereigntyTier) -> WebCapabilities {
        self.tiers
            .get(&tier)
            .copied()
            .unwrap_or(WebCapabilities::NONE)
    }

    /// Returns `true` if the given tier has the specified capability.
    #[must_use]
    pub fn tier_has(&self, tier: SovereigntyTier, capability: WebCapabilities) -> bool {
        self.get(tier).contains(capability)
    }

    /// Returns all tiers that have the specified capability.
    #[must_use]
    pub fn tiers_with(&self, capability: WebCapabilities) -> Vec<SovereigntyTier> {
        let mut result: Vec<_> = self
            .tiers
            .iter()
            .filter(|(_, caps)| caps.contains(capability))
            .map(|(tier, _)| *tier)
            .collect();
        result.sort();
        result
    }

    /// Validates that the matrix is monotonically increasing — higher tiers
    /// never lose capabilities that lower tiers have.
    ///
    /// Returns `Ok(())` if valid, or `Err` with a description of the violation.
    pub fn validate_monotonic(&self) -> Result<(), String> {
        let ordered = [
            SovereigntyTier::Sheltered,
            SovereigntyTier::Citizen,
            SovereigntyTier::Steward,
            SovereigntyTier::Architect,
        ];

        for window in ordered.windows(2) {
            let lower = self.get(window[0]);
            let upper = self.get(window[1]);
            if !upper.contains(lower) {
                return Err(format!(
                    "{:?} has capabilities ({:?}) not present in {:?} ({:?})",
                    window[0], lower, window[1], upper,
                ));
            }
        }

        Ok(())
    }
}

impl Default for WebCapabilityMatrix {
    fn default() -> Self {
        Self::default_matrix()
    }
}

// ---------------------------------------------------------------------------
// ServiceWorkerStrategy
// ---------------------------------------------------------------------------

/// Strategies the service worker can employ for offline resilience.
///
/// Each strategy targets a different aspect of the PWA's offline behavior.
/// Multiple strategies can be active simultaneously.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ServiceWorkerStrategy {
    /// Cache `.idea` files locally for offline access.
    ///
    /// When a user views an `.idea` file, the service worker stores a copy
    /// in the browser's Cache API. Subsequent requests are served from cache
    /// if the network is unavailable.
    CacheIdea,

    /// Queue Globe publishes for sync when online.
    ///
    /// When the user creates or edits content while offline, the publish
    /// operation is queued. When connectivity returns, queued publishes
    /// are sent in order.
    QueuePublish,

    /// Pre-cache Regalia themes for offline rendering.
    ///
    /// Ensures that even without network, `.idea` files render with
    /// proper styling. Themes are fetched eagerly during install.
    PreCacheThemes,

    /// Pre-cache Advisor local model for offline AI assistance.
    ///
    /// If the browser supports it (sufficient storage, WebGPU), cache
    /// the local Advisor model for offline cognitive support.
    PreCacheModel,
}

impl ServiceWorkerStrategy {
    /// Returns all defined strategies.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::CacheIdea,
            Self::QueuePublish,
            Self::PreCacheThemes,
            Self::PreCacheModel,
        ]
    }

    /// Returns the strategies appropriate for a given tier.
    ///
    /// - Sheltered/Citizen: CacheIdea only (basic caching for viewed content)
    /// - Steward: CacheIdea + QueuePublish + PreCacheThemes (full offline creation)
    /// - Architect: All strategies (including model pre-caching)
    #[must_use]
    pub fn for_tier(tier: SovereigntyTier) -> Vec<Self> {
        match tier {
            SovereigntyTier::Sheltered | SovereigntyTier::Citizen => {
                vec![Self::CacheIdea]
            }
            SovereigntyTier::Steward => {
                vec![Self::CacheIdea, Self::QueuePublish, Self::PreCacheThemes]
            }
            SovereigntyTier::Architect => Self::all().to_vec(),
        }
    }
}

// ---------------------------------------------------------------------------
// PwaConfig
// ---------------------------------------------------------------------------

/// Configuration for a PWA session.
///
/// Combines the capability set, active service worker strategies, and
/// offline toggle into a single configuration object. Created from a
/// sovereignty tier via [`PwaConfig::for_tier`], or built manually.
///
/// # Examples
///
/// ```
/// use divinity_web::pwa::PwaConfig;
/// use oracle::SovereigntyTier;
///
/// let config = PwaConfig::for_tier(SovereigntyTier::Steward);
/// assert!(config.offline_enabled);
/// assert_eq!(config.strategies.len(), 3);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PwaConfig {
    /// The tier this config was derived from.
    pub tier: SovereigntyTier,

    /// Active web capabilities.
    pub capabilities: WebCapabilities,

    /// Active service worker strategies.
    pub strategies: Vec<ServiceWorkerStrategy>,

    /// Whether offline mode is enabled.
    ///
    /// This is `true` when the capabilities include `OFFLINE`.
    /// When `false`, the service worker still installs (for update
    /// management) but does not intercept fetch requests.
    pub offline_enabled: bool,
}

impl PwaConfig {
    /// Creates a PWA configuration for the given sovereignty tier using
    /// the default capability matrix.
    #[must_use]
    pub fn for_tier(tier: SovereigntyTier) -> Self {
        Self::for_tier_with_matrix(tier, &WebCapabilityMatrix::default_matrix())
    }

    /// Creates a PWA configuration for the given tier using a custom
    /// capability matrix.
    #[must_use]
    pub fn for_tier_with_matrix(tier: SovereigntyTier, matrix: &WebCapabilityMatrix) -> Self {
        let capabilities = matrix.get(tier);
        let offline_enabled = capabilities.contains(WebCapabilities::OFFLINE);
        let strategies = ServiceWorkerStrategy::for_tier(tier);

        Self {
            tier,
            capabilities,
            strategies,
            offline_enabled,
        }
    }

    /// Returns `true` if the config allows the specified capability.
    #[must_use]
    pub fn has_capability(&self, capability: WebCapabilities) -> bool {
        self.capabilities.contains(capability)
    }

    /// Serializes this config to JSON.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Deserializes a config from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

// ---------------------------------------------------------------------------
// OfflineCache
// ---------------------------------------------------------------------------

/// Tracks cached `.idea` file identifiers and their cache state.
///
/// This is the Rust-side bookkeeping for the service worker's Cache API.
/// The actual storage is in the browser; this struct tracks what we expect
/// to be cached so we can make informed decisions about eviction, sync,
/// and offline availability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OfflineCache {
    /// Map of idea ID -> cache entry metadata.
    entries: HashMap<String, CacheEntry>,

    /// Maximum number of cached ideas.
    max_entries: usize,
}

/// Metadata about a single cached `.idea` file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheEntry {
    /// The `.idea` identifier (e.g., a Gospel event ID or content hash).
    pub idea_id: String,

    /// Size in bytes (approximate, for eviction decisions).
    pub size_bytes: u64,

    /// Whether this entry has local modifications not yet synced.
    pub dirty: bool,

    /// Unix timestamp (seconds) when this entry was cached.
    pub cached_at: u64,

    /// Unix timestamp (seconds) of last access.
    pub last_accessed: u64,
}

/// Items queued for publish when connectivity returns.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedPublish {
    /// The `.idea` identifier.
    pub idea_id: String,

    /// Serialized publish payload.
    pub payload: Vec<u8>,

    /// Unix timestamp (seconds) when the publish was queued.
    pub queued_at: u64,
}

impl OfflineCache {
    /// Creates a new offline cache with the given maximum entry count.
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
        }
    }

    /// Creates a cache with the default maximum (1000 entries).
    #[must_use]
    pub fn with_default_capacity() -> Self {
        Self::new(1000)
    }

    /// Adds or updates a cache entry. Returns the evicted entry if the
    /// cache was full and an LRU eviction occurred.
    ///
    /// Dirty entries are never evicted — if all entries are dirty and the
    /// cache is full, the new entry is rejected and returned as `Err`.
    pub fn put(&mut self, entry: CacheEntry) -> Result<Option<CacheEntry>, CacheEntry> {
        // If updating an existing entry, just replace it.
        if self.entries.contains_key(&entry.idea_id) {
            let old = self.entries.insert(entry.idea_id.clone(), entry);
            return Ok(old);
        }

        // If at capacity, evict the least-recently-accessed non-dirty entry.
        if self.entries.len() >= self.max_entries {
            let evict_id = self
                .entries
                .iter()
                .filter(|(_, e)| !e.dirty)
                .min_by_key(|(_, e)| e.last_accessed)
                .map(|(id, _)| id.clone());

            match evict_id {
                Some(id) => {
                    let evicted = self.entries.remove(&id);
                    self.entries.insert(entry.idea_id.clone(), entry);
                    Ok(evicted)
                }
                None => {
                    // All entries are dirty — cannot evict.
                    Err(entry)
                }
            }
        } else {
            self.entries.insert(entry.idea_id.clone(), entry);
            Ok(None)
        }
    }

    /// Retrieves a cache entry by idea ID, updating its last-accessed time.
    pub fn get(&mut self, idea_id: &str, now: u64) -> Option<&CacheEntry> {
        if let Some(entry) = self.entries.get_mut(idea_id) {
            entry.last_accessed = now;
        }
        self.entries.get(idea_id)
    }

    /// Retrieves a cache entry without updating access time.
    #[must_use]
    pub fn peek(&self, idea_id: &str) -> Option<&CacheEntry> {
        self.entries.get(idea_id)
    }

    /// Removes a cache entry. Returns `true` if it existed.
    pub fn remove(&mut self, idea_id: &str) -> bool {
        self.entries.remove(idea_id).is_some()
    }

    /// Marks an entry as dirty (has local modifications).
    pub fn mark_dirty(&mut self, idea_id: &str) -> bool {
        if let Some(entry) = self.entries.get_mut(idea_id) {
            entry.dirty = true;
            true
        } else {
            false
        }
    }

    /// Marks an entry as clean (synced with network).
    pub fn mark_clean(&mut self, idea_id: &str) -> bool {
        if let Some(entry) = self.entries.get_mut(idea_id) {
            entry.dirty = false;
            true
        } else {
            false
        }
    }

    /// Returns all dirty entries (need sync when online).
    #[must_use]
    pub fn dirty_entries(&self) -> Vec<&CacheEntry> {
        self.entries.values().filter(|e| e.dirty).collect()
    }

    /// Returns the number of cached entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the total approximate size of all cached entries.
    #[must_use]
    pub fn total_size_bytes(&self) -> u64 {
        self.entries.values().map(|e| e.size_bytes).sum()
    }

    /// Clears all non-dirty entries. Returns the count of entries removed.
    pub fn evict_clean(&mut self) -> usize {
        let before = self.entries.len();
        self.entries.retain(|_, e| e.dirty);
        before - self.entries.len()
    }

    /// Returns `true` if an idea is cached.
    #[must_use]
    pub fn contains(&self, idea_id: &str) -> bool {
        self.entries.contains_key(idea_id)
    }
}

impl Default for OfflineCache {
    fn default() -> Self {
        Self::with_default_capacity()
    }
}

// ---------------------------------------------------------------------------
// PublishQueue
// ---------------------------------------------------------------------------

/// Queue of `.idea` publishes waiting for network connectivity.
///
/// When the user creates or modifies content while offline, the publish
/// is queued here. When connectivity returns, items are drained in order
/// (oldest first) and published to Globe.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PublishQueue {
    items: Vec<QueuedPublish>,
}

impl PublishQueue {
    /// Creates a new empty publish queue.
    #[must_use]
    pub fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Enqueues a publish operation.
    pub fn enqueue(&mut self, item: QueuedPublish) {
        self.items.push(item);
    }

    /// Dequeues the oldest publish operation (FIFO).
    pub fn dequeue(&mut self) -> Option<QueuedPublish> {
        if self.items.is_empty() {
            None
        } else {
            Some(self.items.remove(0))
        }
    }

    /// Returns the number of queued publishes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the queue is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Peeks at the next item without removing it.
    #[must_use]
    pub fn peek(&self) -> Option<&QueuedPublish> {
        self.items.first()
    }

    /// Drains all queued items, returning them oldest-first.
    pub fn drain_all(&mut self) -> Vec<QueuedPublish> {
        std::mem::take(&mut self.items)
    }

    /// Removes all queued publishes for a specific idea.
    /// Returns the count removed.
    pub fn remove_for_idea(&mut self, idea_id: &str) -> usize {
        let before = self.items.len();
        self.items.retain(|item| item.idea_id != idea_id);
        before - self.items.len()
    }

    /// Serializes the queue to JSON for persistence.
    #[must_use]
    pub fn to_json(&self) -> String {
        serde_json::to_string(&self.items).unwrap_or_else(|_| "[]".to_string())
    }

    /// Restores the queue from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let items: Vec<QueuedPublish> = serde_json::from_str(json)?;
        Ok(Self { items })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- WebCapabilities tests --

    #[test]
    fn capabilities_bitwise_or() {
        let caps = WebCapabilities::RENDERING | WebCapabilities::VAULT;
        assert!(caps.contains(WebCapabilities::RENDERING));
        assert!(caps.contains(WebCapabilities::VAULT));
        assert!(!caps.contains(WebCapabilities::EDITING));
        assert!(!caps.contains(WebCapabilities::CRYPTO));
    }

    #[test]
    fn capabilities_bitwise_and() {
        let a = WebCapabilities::RENDERING | WebCapabilities::VAULT | WebCapabilities::NETWORK;
        let b = WebCapabilities::VAULT | WebCapabilities::CRYPTO;
        let intersection = a & b;
        assert!(intersection.contains(WebCapabilities::VAULT));
        assert!(!intersection.contains(WebCapabilities::RENDERING));
        assert!(!intersection.contains(WebCapabilities::CRYPTO));
    }

    #[test]
    fn capabilities_not() {
        let caps = WebCapabilities::RENDERING | WebCapabilities::VAULT;
        let inverse = !caps;
        assert!(!inverse.contains(WebCapabilities::RENDERING));
        assert!(!inverse.contains(WebCapabilities::VAULT));
        assert!(inverse.contains(WebCapabilities::EDITING));
        assert!(inverse.contains(WebCapabilities::NETWORK));
        assert!(inverse.contains(WebCapabilities::OFFLINE));
        assert!(inverse.contains(WebCapabilities::CRYPTO));
    }

    #[test]
    fn capabilities_none_and_all() {
        assert!(WebCapabilities::NONE.is_empty());
        assert!(!WebCapabilities::ALL.is_empty());
        assert_eq!(WebCapabilities::ALL.count(), 6);
        assert!(WebCapabilities::ALL.contains(WebCapabilities::CRYPTO));
    }

    #[test]
    fn capabilities_from_bits_masks_unknown() {
        let caps = WebCapabilities::from_bits(0xFFFF_FFFF);
        assert_eq!(caps, WebCapabilities::ALL);
    }

    #[test]
    fn capabilities_debug_display() {
        let caps = WebCapabilities::RENDERING | WebCapabilities::CRYPTO;
        let debug = format!("{:?}", caps);
        assert!(debug.contains("RENDERING"));
        assert!(debug.contains("CRYPTO"));
        assert!(!debug.contains("VAULT"));
    }

    #[test]
    fn capabilities_serde_roundtrip() {
        let caps = WebCapabilities::RENDERING | WebCapabilities::VAULT | WebCapabilities::NETWORK;
        let json = serde_json::to_string(&caps).expect("serialize");
        let deserialized: WebCapabilities = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(caps, deserialized);
    }

    // -- WebCapabilityMatrix tests --

    #[test]
    fn default_matrix_sheltered_citizen_equal() {
        let matrix = WebCapabilityMatrix::default_matrix();
        assert_eq!(
            matrix.get(SovereigntyTier::Sheltered),
            matrix.get(SovereigntyTier::Citizen),
        );
    }

    #[test]
    fn default_matrix_tier_capabilities() {
        let matrix = WebCapabilityMatrix::default_matrix();

        // Sheltered: Rendering | Vault | Network
        let sheltered = matrix.get(SovereigntyTier::Sheltered);
        assert!(sheltered.contains(WebCapabilities::RENDERING));
        assert!(sheltered.contains(WebCapabilities::VAULT));
        assert!(sheltered.contains(WebCapabilities::NETWORK));
        assert!(!sheltered.contains(WebCapabilities::EDITING));
        assert!(!sheltered.contains(WebCapabilities::OFFLINE));
        assert!(!sheltered.contains(WebCapabilities::CRYPTO));

        // Steward: + Editing | Offline
        let steward = matrix.get(SovereigntyTier::Steward);
        assert!(steward.contains(WebCapabilities::RENDERING));
        assert!(steward.contains(WebCapabilities::VAULT));
        assert!(steward.contains(WebCapabilities::NETWORK));
        assert!(steward.contains(WebCapabilities::EDITING));
        assert!(steward.contains(WebCapabilities::OFFLINE));
        assert!(!steward.contains(WebCapabilities::CRYPTO));

        // Architect: + Crypto (all)
        let architect = matrix.get(SovereigntyTier::Architect);
        assert_eq!(architect, WebCapabilities::ALL);
    }

    #[test]
    fn default_matrix_is_monotonic() {
        let matrix = WebCapabilityMatrix::default_matrix();
        assert!(matrix.validate_monotonic().is_ok());
    }

    #[test]
    fn non_monotonic_matrix_detected() {
        let mut matrix = WebCapabilityMatrix::new();
        matrix.set(
            SovereigntyTier::Sheltered,
            WebCapabilities::RENDERING | WebCapabilities::CRYPTO,
        );
        matrix.set(SovereigntyTier::Citizen, WebCapabilities::RENDERING);
        // Citizen lost CRYPTO that Sheltered has -> not monotonic.
        assert!(matrix.validate_monotonic().is_err());
    }

    #[test]
    fn matrix_tiers_with() {
        let matrix = WebCapabilityMatrix::default_matrix();
        let crypto_tiers = matrix.tiers_with(WebCapabilities::CRYPTO);
        assert_eq!(crypto_tiers, vec![SovereigntyTier::Architect]);

        let rendering_tiers = matrix.tiers_with(WebCapabilities::RENDERING);
        assert_eq!(rendering_tiers.len(), 4); // All tiers have rendering
    }

    // -- ServiceWorkerStrategy tests --

    #[test]
    fn strategy_for_citizen() {
        let strategies = ServiceWorkerStrategy::for_tier(SovereigntyTier::Citizen);
        assert_eq!(strategies, vec![ServiceWorkerStrategy::CacheIdea]);
    }

    #[test]
    fn strategy_for_steward() {
        let strategies = ServiceWorkerStrategy::for_tier(SovereigntyTier::Steward);
        assert_eq!(strategies.len(), 3);
        assert!(strategies.contains(&ServiceWorkerStrategy::CacheIdea));
        assert!(strategies.contains(&ServiceWorkerStrategy::QueuePublish));
        assert!(strategies.contains(&ServiceWorkerStrategy::PreCacheThemes));
        assert!(!strategies.contains(&ServiceWorkerStrategy::PreCacheModel));
    }

    #[test]
    fn strategy_for_architect_has_all() {
        let strategies = ServiceWorkerStrategy::for_tier(SovereigntyTier::Architect);
        assert_eq!(strategies.len(), 4);
        assert!(strategies.contains(&ServiceWorkerStrategy::PreCacheModel));
    }

    // -- PwaConfig tests --

    #[test]
    fn config_for_citizen_offline_disabled() {
        let config = PwaConfig::for_tier(SovereigntyTier::Citizen);
        assert!(!config.offline_enabled);
        assert!(!config.has_capability(WebCapabilities::OFFLINE));
        assert!(config.has_capability(WebCapabilities::RENDERING));
    }

    #[test]
    fn config_for_steward_offline_enabled() {
        let config = PwaConfig::for_tier(SovereigntyTier::Steward);
        assert!(config.offline_enabled);
        assert!(config.has_capability(WebCapabilities::OFFLINE));
        assert!(config.has_capability(WebCapabilities::EDITING));
    }

    #[test]
    fn config_json_roundtrip() {
        let config = PwaConfig::for_tier(SovereigntyTier::Architect);
        let json = config.to_json();
        let restored = PwaConfig::from_json(&json).expect("deserialize");
        assert_eq!(config, restored);
    }

    // -- OfflineCache tests --

    #[test]
    fn cache_put_and_get() {
        let mut cache = OfflineCache::new(10);
        let entry = CacheEntry {
            idea_id: "idea-1".to_string(),
            size_bytes: 1024,
            dirty: false,
            cached_at: 1000,
            last_accessed: 1000,
        };
        assert!(cache.put(entry.clone()).is_ok());
        assert_eq!(cache.len(), 1);

        let fetched = cache.get("idea-1", 2000).expect("should exist");
        assert_eq!(fetched.idea_id, "idea-1");
        assert_eq!(fetched.last_accessed, 2000);
    }

    #[test]
    fn cache_eviction_lru_non_dirty() {
        let mut cache = OfflineCache::new(2);

        let e1 = CacheEntry {
            idea_id: "old".to_string(),
            size_bytes: 100,
            dirty: false,
            cached_at: 100,
            last_accessed: 100,
        };
        let e2 = CacheEntry {
            idea_id: "new".to_string(),
            size_bytes: 200,
            dirty: false,
            cached_at: 200,
            last_accessed: 200,
        };
        let e3 = CacheEntry {
            idea_id: "newest".to_string(),
            size_bytes: 300,
            dirty: false,
            cached_at: 300,
            last_accessed: 300,
        };

        cache.put(e1).unwrap();
        cache.put(e2).unwrap();
        // Cache is full. Inserting e3 should evict "old" (lowest last_accessed).
        let evicted = cache.put(e3).unwrap();
        assert!(evicted.is_some());
        assert_eq!(evicted.unwrap().idea_id, "old");
        assert!(cache.contains("new"));
        assert!(cache.contains("newest"));
        assert!(!cache.contains("old"));
    }

    #[test]
    fn cache_rejects_when_all_dirty() {
        let mut cache = OfflineCache::new(1);
        let e1 = CacheEntry {
            idea_id: "dirty-one".to_string(),
            size_bytes: 100,
            dirty: true,
            cached_at: 100,
            last_accessed: 100,
        };
        cache.put(e1).unwrap();

        let e2 = CacheEntry {
            idea_id: "new".to_string(),
            size_bytes: 200,
            dirty: false,
            cached_at: 200,
            last_accessed: 200,
        };
        // Should be rejected because the only entry is dirty.
        let result = cache.put(e2);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().idea_id, "new");
    }

    #[test]
    fn cache_dirty_tracking() {
        let mut cache = OfflineCache::new(10);
        let entry = CacheEntry {
            idea_id: "idea-1".to_string(),
            size_bytes: 512,
            dirty: false,
            cached_at: 1000,
            last_accessed: 1000,
        };
        cache.put(entry).unwrap();
        assert!(cache.dirty_entries().is_empty());

        cache.mark_dirty("idea-1");
        assert_eq!(cache.dirty_entries().len(), 1);

        cache.mark_clean("idea-1");
        assert!(cache.dirty_entries().is_empty());
    }

    #[test]
    fn cache_evict_clean() {
        let mut cache = OfflineCache::new(10);
        for i in 0..5 {
            let entry = CacheEntry {
                idea_id: format!("idea-{}", i),
                size_bytes: 100,
                dirty: i % 2 == 0, // 0, 2, 4 are dirty
                cached_at: 1000,
                last_accessed: 1000,
            };
            cache.put(entry).unwrap();
        }
        assert_eq!(cache.len(), 5);

        let evicted = cache.evict_clean();
        assert_eq!(evicted, 2); // ideas 1, 3 evicted
        assert_eq!(cache.len(), 3); // ideas 0, 2, 4 remain
    }

    // -- PublishQueue tests --

    #[test]
    fn publish_queue_fifo() {
        let mut queue = PublishQueue::new();
        queue.enqueue(QueuedPublish {
            idea_id: "first".to_string(),
            payload: vec![1, 2, 3],
            queued_at: 100,
        });
        queue.enqueue(QueuedPublish {
            idea_id: "second".to_string(),
            payload: vec![4, 5, 6],
            queued_at: 200,
        });

        assert_eq!(queue.len(), 2);
        assert_eq!(queue.dequeue().unwrap().idea_id, "first");
        assert_eq!(queue.dequeue().unwrap().idea_id, "second");
        assert!(queue.is_empty());
    }

    #[test]
    fn publish_queue_json_roundtrip() {
        let mut queue = PublishQueue::new();
        queue.enqueue(QueuedPublish {
            idea_id: "test".to_string(),
            payload: vec![42],
            queued_at: 999,
        });

        let json = queue.to_json();
        let restored = PublishQueue::from_json(&json).expect("deserialize");
        assert_eq!(queue, restored);
    }

    #[test]
    fn publish_queue_remove_for_idea() {
        let mut queue = PublishQueue::new();
        queue.enqueue(QueuedPublish {
            idea_id: "keep".to_string(),
            payload: vec![1],
            queued_at: 100,
        });
        queue.enqueue(QueuedPublish {
            idea_id: "remove".to_string(),
            payload: vec![2],
            queued_at: 200,
        });
        queue.enqueue(QueuedPublish {
            idea_id: "remove".to_string(),
            payload: vec![3],
            queued_at: 300,
        });

        let removed = queue.remove_for_idea("remove");
        assert_eq!(removed, 2);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue.peek().unwrap().idea_id, "keep");
    }
}
