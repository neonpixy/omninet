use std::ops::Range;

use serde::{Deserialize, Serialize};

// -- Standard event kinds --

/// User profile metadata (kind 0).
pub const PROFILE: u32 = 0;
/// Short text note (kind 1).
pub const TEXT_NOTE: u32 = 1;
/// Contact/following list (kind 3).
pub const CONTACT_LIST: u32 = 3;
/// Relay authentication event (kind 22242).
pub const AUTH_EVENT: u32 = 22242;

// -- ABC module kind ranges (1000 per letter) --

/// Advisor (AI cognition) kind range: 1000-1999.
pub const ADVISOR_RANGE: Range<u32> = 1000..2000;
/// Bulwark (safety & protection) kind range: 2000-2999.
pub const BULWARK_RANGE: Range<u32> = 2000..3000;
/// Crown (identity) kind range: 3000-3999.
pub const CROWN_RANGE: Range<u32> = 3000..4000;
/// Divinity (platform interface) kind range: 4000-4999.
pub const DIVINITY_RANGE: Range<u32> = 4000..5000;
/// Equipment (communication / Pact) kind range: 5000-5999.
pub const EQUIPMENT_RANGE: Range<u32> = 5000..6000;
/// Fortune (economics) kind range: 6000-6999.
pub const FORTUNE_RANGE: Range<u32> = 6000..7000;
/// Globe (networking / ORP) kind range: 7000-7999.
pub const GLOBE_RANGE: Range<u32> = 7000..8000;
/// Hall (file I/O) kind range: 8000-8999.
pub const HALL_RANGE: Range<u32> = 8000..9000;
/// Ideas (.idea content format) kind range: 9000-9999.
pub const IDEAS_RANGE: Range<u32> = 9000..10000;
/// Jail (verification & accountability) kind range: 10000-10999.
pub const JAIL_RANGE: Range<u32> = 10000..11000;
/// Kingdom (community governance) kind range: 11000-11999.
pub const KINGDOM_RANGE: Range<u32> = 11000..12000;
/// Lingo (language & translation) kind range: 12000-12999.
pub const LINGO_RANGE: Range<u32> = 12000..13000;
/// Magic (rendering & code) kind range: 13000-13999.
pub const MAGIC_RANGE: Range<u32> = 13000..14000;
/// Nexus (federation & interop) kind range: 14000-14999.
pub const NEXUS_RANGE: Range<u32> = 14000..15000;
/// Oracle (guidance & onboarding) kind range: 15000-15999.
pub const ORACLE_RANGE: Range<u32> = 15000..16000;
/// Polity (rights & constitution) kind range: 16000-16999.
pub const POLITY_RANGE: Range<u32> = 16000..17000;
/// Quest (gamification) kind range: 17000-17999.
pub const QUEST_RANGE: Range<u32> = 17000..18000;
/// Regalia (design language) kind range: 18000-18999.
pub const REGALIA_RANGE: Range<u32> = 18000..19000;
/// Sentinal (encryption) kind range: 19000-19999.
pub const SENTINAL_RANGE: Range<u32> = 19000..20000;
/// Throne (apps / programs) kind range: 20000-20999.
pub const THRONE_RANGE: Range<u32> = 20000..21000;
/// Universe (Undercroft / observatory) kind range: 21000-21999.
pub const UNIVERSE_RANGE: Range<u32> = 21000..22000;
/// Vault (encrypted storage) kind range: 22000-22999.
pub const VAULT_RANGE: Range<u32> = 22000..23000;
/// World (digital & physical) kind range: 23000-23999.
pub const WORLD_RANGE: Range<u32> = 23000..24000;
/// X (shared utilities) kind range: 24000-24999.
pub const X_RANGE: Range<u32> = 24000..25000;
/// Yoke (history & provenance) kind range: 25000-25999.
pub const YOKE_RANGE: Range<u32> = 25000..26000;
/// Zeitgeist (discovery & culture) kind range: 26000-26999.
pub const ZEITGEIST_RANGE: Range<u32> = 26000..27000;

// -- Special ranges --

/// Replaceable event range: 30000-39999. Only the latest event per author+kind is kept.
pub const REPLACEABLE_RANGE: Range<u32> = 30000..40000;
/// Parameterized replaceable range: 40000-49999. Latest event per author+kind+d-tag is kept.
pub const PARAMETERIZED_RANGE: Range<u32> = 40000..50000;
/// Extension kinds start at 50000. For third-party and experimental event types.
pub const EXTENSION_START: u32 = 50000;

// -- Fortune Commerce kinds (6000-6999) --

/// Product listing — seller publishes/updates a product (kind 6100).
/// Content: JSON product data (references .idea digit for content fields).
/// Tags: ["p", seller_crown_id], d-tag = product_id (replaceable).
pub const PRODUCT_LISTING: u32 = 6100;

/// Cart suggestion — storefront suggests an item to a user (kind 6150).
/// Content: JSON CartSuggestion (storefront_id, product_ref, message).
/// Tags: ["p", target_user_crown_id], ["storefront", storefront_id].
pub const CART_SUGGESTION: u32 = 6150;

/// Storefront declaration — seller publishes/updates their storefront (kind 6200).
/// Content: JSON storefront data.
/// Tags: ["p", owner_crown_id], d-tag = storefront_id (replaceable).
pub const STOREFRONT_DECLARATION: u32 = 6200;

/// Order event — order lifecycle (encrypted to buyer + seller) (kind 6300).
/// Content: encrypted JSON order data.
/// Tags: ["p", buyer_crown_id], ["p", seller_crown_id], ["order", order_id].
pub const ORDER_EVENT: u32 = 6300;

/// Product review — buyer reviews a product (kind 6400).
/// Content: JSON review data (rating, text).
/// Tags: ["p", author_crown_id], ["product", product_ref].
pub const PRODUCT_REVIEW: u32 = 6400;

/// Storefront review — buyer reviews a seller (kind 6500).
/// Content: JSON review data (rating, text).
/// Tags: ["p", author_crown_id], ["storefront", storefront_id].
pub const STOREFRONT_REVIEW: u32 = 6500;

/// All Fortune Commerce kind constants.
pub const COMMERCE_KINDS: &[u32] = &[
    PRODUCT_LISTING,
    CART_SUGGESTION,
    STOREFRONT_DECLARATION,
    ORDER_EVENT,
    PRODUCT_REVIEW,
    STOREFRONT_REVIEW,
];

/// Whether a kind is a Fortune Commerce kind.
pub fn is_commerce_kind(kind: u32) -> bool {
    COMMERCE_KINDS.contains(&kind)
}

/// All Globe naming system kind constants (7000-7005).
pub const NAME_KINDS: &[u32] = &[
    NAME_CLAIM,
    NAME_UPDATE,
    NAME_TRANSFER,
    NAME_DELEGATE,
    NAME_REVOKE,
    NAME_RENEWAL,
];

/// Whether a kind is a Globe naming system kind (7000-7005).
pub fn is_name_kind(kind: u32) -> bool {
    NAME_KINDS.contains(&kind)
}

// -- Globe-specific kinds (naming system) --

/// Claim a domain name (kind 7000).
pub const NAME_CLAIM: u32 = 7000;
/// Update a name's target (kind 7001).
pub const NAME_UPDATE: u32 = 7001;
/// Transfer name ownership (kind 7002).
pub const NAME_TRANSFER: u32 = 7002;
/// Delegate a subdomain (kind 7003).
pub const NAME_DELEGATE: u32 = 7003;
/// Revoke a name (kind 7004).
pub const NAME_REVOKE: u32 = 7004;
/// Renew a name registration (kind 7005).
pub const NAME_RENEWAL: u32 = 7005;

// -- Gospel kinds (evangelized discovery) --

/// Relay hints — advertise which relays a user can be found on (kind 7010).
/// Content: JSON `{"relays": ["wss://...", ...]}`.
/// Tags: d-tag = "relay-hints" (parameterized replaceable per author).
pub const RELAY_HINT: u32 = 7010;

/// Asset announcement — advertise that a relay stores a binary asset (kind 7020).
/// Content: empty (metadata in tags).
/// Tags: ["asset", hash, mime, size], ["r", relay_url], d-tag = hash.
pub const ASSET_ANNOUNCE: u32 = 7020;

/// Chunk manifest — ordered list of content-addressed chunks for large files (kind 9000).
/// Content: JSON ChunkManifest (content_hash, total_size, chunk_size, chunks[]).
/// Tags: ["chunks", count], ["size", total_size], d-tag = content_hash.
pub const CHUNK_MANIFEST: u32 = 9000;

// -- Equipment Communicator signaling kinds (5100–5112) --

/// Communicator offer — initiate a real-time session (kind 5100).
/// Content: encrypted offer JSON (encrypted to each participant's Crown pubkey).
/// Tags: ["p", each_participant_crown_id], ["session", session_id].
pub const COMMUNICATOR_OFFER: u32 = 5100;

/// Communicator answer — accept or decline an offer (kind 5101).
/// Content: JSON with accepted boolean.
/// Tags: ["p", initiator_crown_id], ["session", session_id].
pub const COMMUNICATOR_ANSWER: u32 = 5101;

/// Communicator end — signal session termination (kind 5102).
/// Content: JSON with reason.
/// Tags: ["session", session_id].
pub const COMMUNICATOR_END: u32 = 5102;

/// ICE candidate — WebRTC ICE candidate exchange (kind 5103).
/// Content: opaque ICE candidate JSON (encrypted to target's Crown pubkey).
/// Tags: ["p", target_crown_id], ["session", session_id].
pub const ICE_CANDIDATE: u32 = 5103;

/// Stream announcement — advertise a live stream (kind 5110).
/// Content: JSON stream metadata (title, kind, status, relay, fortune config).
/// Tags: ["session", session_id], d-tag = session_id (replaceable).
pub const STREAM_ANNOUNCE: u32 = 5110;

/// Stream update — update a live stream's metadata (kind 5111).
/// Tags: ["session", session_id], d-tag = session_id.
pub const STREAM_UPDATE: u32 = 5111;

/// Stream end — signal stream termination (kind 5112).
/// Tags: ["session", session_id].
pub const STREAM_END: u32 = 5112;

/// Stream recording — links a completed stream to its chunk manifest for replay (kind 5113).
/// Content: JSON metadata (duration_secs, format, thumbnail_hash, etc.).
/// Tags: ["session", session_id], d-tag = manifest_hash (replaceable — streamer can re-encode).
pub const STREAM_RECORDING: u32 = 5113;

// -- Discovery kinds (beacons, invitations, network key) --

/// Community beacon — self-describing discoverable entry point (kind 7030).
/// Content: JSON BeaconRecord (name, description, tags, member count, relay URLs).
/// Tags: d-tag = community_id (replaceable), ["t", tag...], ["r", relay_url...].
pub const BEACON: u32 = 7030;

/// Community beacon update — refreshed stats/preview (kind 7031).
pub const BEACON_UPDATE: u32 = 7031;

/// Tower lighthouse announcement — a Tower node advertises itself (kind 7032).
/// Content: JSON TowerAnnouncement (mode, relay_url, gospel_count, uptime, version).
/// Tags: d-tag = tower pubkey (replaceable), ["mode", "pharos"|"harbor"], ["r", relay_url].
pub const LIGHTHOUSE_ANNOUNCE: u32 = 7032;

/// Network Key delivery — encrypted key envelope for a recipient (kind 7040).
/// Content: JSON NetworkKeyEnvelope (encrypted to recipient's Crown pubkey).
/// Tags: ["p", recipient_crown_id], ["key_version", version].
pub const KEY_DELIVERY: u32 = 7040;

/// Network Key rotation — announces a new key version (kind 7041).
/// Content: JSON KeyRotation (old/new version, grace period, reason).
/// Tags: ["key_version", new], ["old_version", old].
pub const KEY_ROTATION: u32 = 7041;

/// Invitation — carries Network Key + relay addresses for a new participant (kind 7042).
/// Content: JSON Invitation (token, relay_urls, key_envelope).
/// Tags: ["p", invitee_crown_id], ["token", one_time_token].
pub const INVITATION: u32 = 7042;

// -- Yoke kinds (history & provenance, 25000-25006) --

/// Yoke relationship — typed edge between two entities (kind 25000).
/// Content: JSON YokeLink (source, target, relationship type, metadata).
/// Tags: ["source", id], ["target", id], ["rel", type_string].
pub const YOKE_RELATIONSHIP: u32 = 25000;

/// Yoke version tag — named snapshot of an .idea (kind 25001).
/// Content: JSON VersionTag (idea_id, name, snapshot_clock, branch).
/// Tags: d-tag = idea_id (parameterized per idea), ["branch", name], ["version", name].
pub const YOKE_VERSION_TAG: u32 = 25001;

/// Yoke branch — fork a version timeline (kind 25002).
/// Content: JSON Branch (name, created_from, author).
/// Tags: d-tag = idea_id, ["branch", name], ["from", version_id].
pub const YOKE_BRANCH: u32 = 25002;

/// Yoke merge — join branches (kind 25003).
/// Content: JSON MergeRecord (target_branch, merge_version).
/// Tags: d-tag = idea_id, ["source", source_branch], ["target", target_branch].
pub const YOKE_MERGE: u32 = 25003;

/// Yoke milestone — named moment in community history (kind 25004).
/// Content: JSON Milestone (name, significance, description).
/// Tags: d-tag = milestone_id, ["community", id].
pub const YOKE_MILESTONE: u32 = 25004;

/// Yoke ceremony — Covenant moment (kind 25005).
/// Content: JSON CeremonyRecord (type, participants, content).
/// Tags: d-tag = ceremony_id, ["type", ceremony_type], ["community", id].
pub const YOKE_CEREMONY: u32 = 25005;

/// Yoke activity — entry in the activity stream (kind 25006).
/// Content: JSON ActivityRecord (actor, action, target).
/// Tags: ["actor", crown_id], ["action", type], ["target", id], ["community", id].
pub const YOKE_ACTIVITY: u32 = 25006;

// -- Zeitgeist kinds (discovery & culture, 26000-26999) --

/// Semantic profile — a Tower advertises its search capabilities (kind 26000).
/// Content: JSON capabilities (keyword_search, semantic_search, suggestions).
/// Tags: d-tag = tower pubkey (replaceable), ["capability", name].
pub const SEMANTIC_PROFILE: u32 = 26000;

/// All Yoke kind constants.
pub const YOKE_KINDS: &[u32] = &[
    YOKE_RELATIONSHIP,
    YOKE_VERSION_TAG,
    YOKE_BRANCH,
    YOKE_MERGE,
    YOKE_MILESTONE,
    YOKE_CEREMONY,
    YOKE_ACTIVITY,
];

/// Whether a kind is a Yoke kind (25000-25006).
pub fn is_yoke_kind(kind: u32) -> bool {
    YOKE_KINDS.contains(&kind)
}

/// All event kinds that the gospel system evangelizes across the network.
/// These are the "registry records" — names and relay hints — that must
/// propagate globally for discovery to work.
pub const GOSPEL_REGISTRY_KINDS: &[u32] = &[
    NAME_CLAIM,
    NAME_UPDATE,
    NAME_TRANSFER,
    NAME_DELEGATE,
    NAME_REVOKE,
    NAME_RENEWAL,
    RELAY_HINT,
    ASSET_ANNOUNCE,
    PRODUCT_LISTING,
    STOREFRONT_DECLARATION,
    BEACON,
    BEACON_UPDATE,
    LIGHTHOUSE_ANNOUNCE,
    SEMANTIC_PROFILE,
];

/// Whether a kind is a gospel registry record (should be evangelized).
pub fn is_gospel_registry(kind: u32) -> bool {
    GOSPEL_REGISTRY_KINDS.contains(&kind)
}

/// Which ABC subsystem handles a given event kind.
///
/// Use [`subsystem_for_kind`] to map a numeric kind to its subsystem.
/// Kinds 0-999 are `Standard`, each ABC letter gets 1000 kinds, and
/// special ranges cover replaceable, parameterized, and extension events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Subsystem {
    /// Built-in kinds (0-999): profile, text note, contacts.
    Standard,
    /// Advisor — AI cognition (1000-1999).
    Advisor,
    /// Bulwark — safety & protection (2000-2999).
    Bulwark,
    /// Crown — identity (3000-3999).
    Crown,
    /// Divinity — platform interface (4000-4999).
    Divinity,
    /// Equipment — communication / Pact (5000-5999).
    Equipment,
    /// Fortune — economics (6000-6999).
    Fortune,
    /// Globe — networking / ORP (7000-7999).
    Globe,
    /// Hall — file I/O (8000-8999).
    Hall,
    /// Ideas — .idea content format (9000-9999).
    Ideas,
    /// Jail — verification & accountability (10000-10999).
    Jail,
    /// Kingdom — community governance (11000-11999).
    Kingdom,
    /// Lingo — language & translation (12000-12999).
    Lingo,
    /// Magic — rendering & code (13000-13999).
    Magic,
    /// Nexus — federation & interop (14000-14999).
    Nexus,
    /// Oracle — guidance & onboarding (15000-15999).
    Oracle,
    /// Polity — rights & constitution (16000-16999).
    Polity,
    /// Quest — gamification (17000-17999).
    Quest,
    /// Regalia — design language (18000-18999).
    Regalia,
    /// Sentinal — encryption (19000-19999).
    Sentinal,
    /// Throne — apps / programs (20000-20999).
    Throne,
    /// Universe — Undercroft / observatory (21000-21999).
    Universe,
    /// Vault — encrypted storage (22000-22999).
    Vault,
    /// World — digital & physical (23000-23999).
    World,
    /// X — shared utilities (24000-24999).
    X,
    /// Yoke — history & provenance (25000-25999).
    Yoke,
    /// Zeitgeist — discovery & culture (26000-26999).
    Zeitgeist,
    /// Replaceable events (30000-39999). Latest per author+kind wins.
    Replaceable,
    /// Parameterized replaceable events (40000-49999). Latest per author+kind+d-tag wins.
    Parameterized,
    /// Extension events (50000+). Third-party and experimental.
    Extension,
    /// Kind falls outside all known ranges.
    Unknown,
}

/// Determine which subsystem handles a given event kind.
pub fn subsystem_for_kind(kind: u32) -> Subsystem {
    match kind {
        0..1000 => Subsystem::Standard,
        k if ADVISOR_RANGE.contains(&k) => Subsystem::Advisor,
        k if BULWARK_RANGE.contains(&k) => Subsystem::Bulwark,
        k if CROWN_RANGE.contains(&k) => Subsystem::Crown,
        k if DIVINITY_RANGE.contains(&k) => Subsystem::Divinity,
        k if EQUIPMENT_RANGE.contains(&k) => Subsystem::Equipment,
        k if FORTUNE_RANGE.contains(&k) => Subsystem::Fortune,
        k if GLOBE_RANGE.contains(&k) => Subsystem::Globe,
        k if HALL_RANGE.contains(&k) => Subsystem::Hall,
        k if IDEAS_RANGE.contains(&k) => Subsystem::Ideas,
        k if JAIL_RANGE.contains(&k) => Subsystem::Jail,
        k if KINGDOM_RANGE.contains(&k) => Subsystem::Kingdom,
        k if LINGO_RANGE.contains(&k) => Subsystem::Lingo,
        k if MAGIC_RANGE.contains(&k) => Subsystem::Magic,
        k if NEXUS_RANGE.contains(&k) => Subsystem::Nexus,
        k if ORACLE_RANGE.contains(&k) => Subsystem::Oracle,
        k if POLITY_RANGE.contains(&k) => Subsystem::Polity,
        k if QUEST_RANGE.contains(&k) => Subsystem::Quest,
        k if REGALIA_RANGE.contains(&k) => Subsystem::Regalia,
        k if SENTINAL_RANGE.contains(&k) => Subsystem::Sentinal,
        k if THRONE_RANGE.contains(&k) => Subsystem::Throne,
        k if UNIVERSE_RANGE.contains(&k) => Subsystem::Universe,
        k if VAULT_RANGE.contains(&k) => Subsystem::Vault,
        k if WORLD_RANGE.contains(&k) => Subsystem::World,
        k if X_RANGE.contains(&k) => Subsystem::X,
        k if YOKE_RANGE.contains(&k) => Subsystem::Yoke,
        k if ZEITGEIST_RANGE.contains(&k) => Subsystem::Zeitgeist,
        k if REPLACEABLE_RANGE.contains(&k) => Subsystem::Replaceable,
        k if PARAMETERIZED_RANGE.contains(&k) => Subsystem::Parameterized,
        k if k >= EXTENSION_START => Subsystem::Extension,
        _ => Subsystem::Unknown,
    }
}

/// Whether a kind is in the replaceable range (30000-39999).
pub fn is_replaceable(kind: u32) -> bool {
    REPLACEABLE_RANGE.contains(&kind)
}

/// Whether a kind is in the parameterized replaceable range (40000-49999).
pub fn is_parameterized_replaceable(kind: u32) -> bool {
    PARAMETERIZED_RANGE.contains(&kind)
}

/// Whether a kind is in the extension range (50000+).
pub fn is_extension(kind: u32) -> bool {
    kind >= EXTENSION_START
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_kinds_route_correctly() {
        assert_eq!(subsystem_for_kind(PROFILE), Subsystem::Standard);
        assert_eq!(subsystem_for_kind(TEXT_NOTE), Subsystem::Standard);
        assert_eq!(subsystem_for_kind(CONTACT_LIST), Subsystem::Standard);
        assert_eq!(subsystem_for_kind(999), Subsystem::Standard);
    }

    #[test]
    fn abc_ranges_route_correctly() {
        assert_eq!(subsystem_for_kind(1000), Subsystem::Advisor);
        assert_eq!(subsystem_for_kind(1999), Subsystem::Advisor);
        assert_eq!(subsystem_for_kind(6500), Subsystem::Fortune);
        assert_eq!(subsystem_for_kind(7000), Subsystem::Globe);
        assert_eq!(subsystem_for_kind(26999), Subsystem::Zeitgeist);
    }

    #[test]
    fn globe_naming_kinds() {
        assert_eq!(subsystem_for_kind(NAME_CLAIM), Subsystem::Globe);
        assert_eq!(subsystem_for_kind(NAME_TRANSFER), Subsystem::Globe);
        assert_eq!(subsystem_for_kind(NAME_REVOKE), Subsystem::Globe);
    }

    #[test]
    fn special_ranges() {
        assert!(is_replaceable(30000));
        assert!(is_replaceable(39999));
        assert!(!is_replaceable(40000));

        assert!(is_parameterized_replaceable(40000));
        assert!(is_parameterized_replaceable(49999));
        assert!(!is_parameterized_replaceable(50000));

        assert!(is_extension(50000));
        assert!(is_extension(99999));
        assert!(!is_extension(49999));
    }

    #[test]
    fn special_ranges_route_correctly() {
        assert_eq!(subsystem_for_kind(35000), Subsystem::Replaceable);
        assert_eq!(subsystem_for_kind(45000), Subsystem::Parameterized);
        assert_eq!(subsystem_for_kind(60000), Subsystem::Extension);
    }

    #[test]
    fn unknown_gap_kinds() {
        // 27000-29999 is between Zeitgeist and Replaceable — unknown.
        assert_eq!(subsystem_for_kind(28000), Subsystem::Unknown);
    }

    #[test]
    fn boundary_values() {
        // Each ABC boundary: end of one range, start of next.
        assert_eq!(subsystem_for_kind(1999), Subsystem::Advisor);
        assert_eq!(subsystem_for_kind(2000), Subsystem::Bulwark);
        assert_eq!(subsystem_for_kind(26999), Subsystem::Zeitgeist);
        assert_eq!(subsystem_for_kind(27000), Subsystem::Unknown);
        assert_eq!(subsystem_for_kind(29999), Subsystem::Unknown);
        assert_eq!(subsystem_for_kind(30000), Subsystem::Replaceable);
    }

    #[test]
    fn gospel_kinds_route_to_globe() {
        assert_eq!(subsystem_for_kind(RELAY_HINT), Subsystem::Globe);
    }

    #[test]
    fn equipment_signaling_kinds_route_to_equipment() {
        assert_eq!(subsystem_for_kind(COMMUNICATOR_OFFER), Subsystem::Equipment);
        assert_eq!(subsystem_for_kind(COMMUNICATOR_ANSWER), Subsystem::Equipment);
        assert_eq!(subsystem_for_kind(COMMUNICATOR_END), Subsystem::Equipment);
        assert_eq!(subsystem_for_kind(ICE_CANDIDATE), Subsystem::Equipment);
        assert_eq!(subsystem_for_kind(STREAM_ANNOUNCE), Subsystem::Equipment);
        assert_eq!(subsystem_for_kind(STREAM_UPDATE), Subsystem::Equipment);
        assert_eq!(subsystem_for_kind(STREAM_END), Subsystem::Equipment);
        assert_eq!(subsystem_for_kind(STREAM_RECORDING), Subsystem::Equipment);
    }

    #[test]
    fn chunk_manifest_routes_to_ideas() {
        assert_eq!(subsystem_for_kind(CHUNK_MANIFEST), Subsystem::Ideas);
    }

    #[test]
    fn is_gospel_registry_true_for_registry_kinds() {
        assert!(is_gospel_registry(NAME_CLAIM));
        assert!(is_gospel_registry(NAME_UPDATE));
        assert!(is_gospel_registry(NAME_TRANSFER));
        assert!(is_gospel_registry(NAME_DELEGATE));
        assert!(is_gospel_registry(NAME_REVOKE));
        assert!(is_gospel_registry(RELAY_HINT));
        assert!(is_gospel_registry(LIGHTHOUSE_ANNOUNCE));
        assert!(is_gospel_registry(SEMANTIC_PROFILE));
    }

    #[test]
    fn is_gospel_registry_false_for_non_registry() {
        assert!(!is_gospel_registry(PROFILE));
        assert!(!is_gospel_registry(TEXT_NOTE));
        assert!(!is_gospel_registry(AUTH_EVENT));
        assert!(!is_gospel_registry(1000)); // Advisor range
    }

    #[test]
    fn yoke_kinds_route_to_yoke() {
        assert_eq!(subsystem_for_kind(YOKE_RELATIONSHIP), Subsystem::Yoke);
        assert_eq!(subsystem_for_kind(YOKE_VERSION_TAG), Subsystem::Yoke);
        assert_eq!(subsystem_for_kind(YOKE_BRANCH), Subsystem::Yoke);
        assert_eq!(subsystem_for_kind(YOKE_MERGE), Subsystem::Yoke);
        assert_eq!(subsystem_for_kind(YOKE_MILESTONE), Subsystem::Yoke);
        assert_eq!(subsystem_for_kind(YOKE_CEREMONY), Subsystem::Yoke);
        assert_eq!(subsystem_for_kind(YOKE_ACTIVITY), Subsystem::Yoke);
    }

    #[test]
    fn is_yoke_kind_true_for_yoke_kinds() {
        assert!(is_yoke_kind(YOKE_RELATIONSHIP));
        assert!(is_yoke_kind(YOKE_VERSION_TAG));
        assert!(is_yoke_kind(YOKE_BRANCH));
        assert!(is_yoke_kind(YOKE_MERGE));
        assert!(is_yoke_kind(YOKE_MILESTONE));
        assert!(is_yoke_kind(YOKE_CEREMONY));
        assert!(is_yoke_kind(YOKE_ACTIVITY));
    }

    #[test]
    fn is_yoke_kind_false_for_non_yoke() {
        assert!(!is_yoke_kind(PROFILE));
        assert!(!is_yoke_kind(TEXT_NOTE));
        assert!(!is_yoke_kind(NAME_CLAIM));
        assert!(!is_yoke_kind(24999)); // X range
        assert!(!is_yoke_kind(26000)); // Zeitgeist range
    }

    #[test]
    fn semantic_profile_routes_to_zeitgeist() {
        assert_eq!(subsystem_for_kind(SEMANTIC_PROFILE), Subsystem::Zeitgeist);
        assert!(is_gospel_registry(SEMANTIC_PROFILE));
    }

    #[test]
    fn commerce_kinds_in_fortune_range() {
        assert!(FORTUNE_RANGE.contains(&PRODUCT_LISTING));
        assert!(FORTUNE_RANGE.contains(&CART_SUGGESTION));
        assert!(FORTUNE_RANGE.contains(&STOREFRONT_DECLARATION));
        assert!(FORTUNE_RANGE.contains(&ORDER_EVENT));
        assert!(FORTUNE_RANGE.contains(&PRODUCT_REVIEW));
        assert!(FORTUNE_RANGE.contains(&STOREFRONT_REVIEW));
    }

    #[test]
    fn commerce_kinds_route_to_fortune() {
        assert_eq!(subsystem_for_kind(PRODUCT_LISTING), Subsystem::Fortune);
        assert_eq!(subsystem_for_kind(CART_SUGGESTION), Subsystem::Fortune);
        assert_eq!(subsystem_for_kind(STOREFRONT_DECLARATION), Subsystem::Fortune);
        assert_eq!(subsystem_for_kind(ORDER_EVENT), Subsystem::Fortune);
        assert_eq!(subsystem_for_kind(PRODUCT_REVIEW), Subsystem::Fortune);
        assert_eq!(subsystem_for_kind(STOREFRONT_REVIEW), Subsystem::Fortune);
    }

    #[test]
    fn is_commerce_kind_true_for_commerce_kinds() {
        assert!(is_commerce_kind(PRODUCT_LISTING));
        assert!(is_commerce_kind(CART_SUGGESTION));
        assert!(is_commerce_kind(STOREFRONT_DECLARATION));
        assert!(is_commerce_kind(ORDER_EVENT));
        assert!(is_commerce_kind(PRODUCT_REVIEW));
        assert!(is_commerce_kind(STOREFRONT_REVIEW));
    }

    #[test]
    fn is_commerce_kind_false_for_non_commerce() {
        assert!(!is_commerce_kind(PROFILE));
        assert!(!is_commerce_kind(TEXT_NOTE));
        assert!(!is_commerce_kind(NAME_CLAIM));
        assert!(!is_commerce_kind(7000)); // Globe range, not Fortune commerce
    }

    #[test]
    fn commerce_gospel_kinds_registered() {
        assert!(is_gospel_registry(PRODUCT_LISTING));
        assert!(is_gospel_registry(STOREFRONT_DECLARATION));
        // These should NOT be in gospel (they're not discovery records)
        assert!(!is_gospel_registry(CART_SUGGESTION));
        assert!(!is_gospel_registry(ORDER_EVENT));
        assert!(!is_gospel_registry(PRODUCT_REVIEW));
        assert!(!is_gospel_registry(STOREFRONT_REVIEW));
    }

    #[test]
    fn subsystem_serde_round_trip() {
        let s = Subsystem::Fortune;
        let json = serde_json::to_string(&s).unwrap();
        let loaded: Subsystem = serde_json::from_str(&json).unwrap();
        assert_eq!(s, loaded);
    }

    #[test]
    fn is_name_kind_true_for_name_kinds() {
        assert!(is_name_kind(NAME_CLAIM));
        assert!(is_name_kind(NAME_UPDATE));
        assert!(is_name_kind(NAME_TRANSFER));
        assert!(is_name_kind(NAME_DELEGATE));
        assert!(is_name_kind(NAME_REVOKE));
        assert!(is_name_kind(NAME_RENEWAL));
    }

    #[test]
    fn is_name_kind_false_for_non_name() {
        assert!(!is_name_kind(PROFILE));
        assert!(!is_name_kind(TEXT_NOTE));
        assert!(!is_name_kind(RELAY_HINT));
        assert!(!is_name_kind(6999)); // Fortune boundary
        assert!(!is_name_kind(7006)); // Next after name kinds
        assert!(!is_name_kind(7010)); // Relay hint
    }

    #[test]
    fn name_renewal_routes_to_globe() {
        assert_eq!(subsystem_for_kind(NAME_RENEWAL), Subsystem::Globe);
    }

    #[test]
    fn name_renewal_is_gospel_registry() {
        assert!(is_gospel_registry(NAME_RENEWAL));
    }
}
