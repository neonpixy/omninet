//! Globe Commons — the public square.
//!
//! A shared event space alongside community-specific relay channels.
//! Communities can publish events to the Commons for cross-community
//! visibility. The Commons is opt-in: communities choose their
//! participation level through `CommonsPolicy`.
//!
//! # Event Kind
//!
//! `COMMONS_PUBLICATION` (7100) — a wrapper event referencing the
//! original event's ID as a tag. The original event is unchanged;
//! the Commons publication is a separate event that references it.

use serde::{Deserialize, Serialize};

use crate::event::OmniEvent;
use crate::filter::OmniFilter;

/// Globe event kind for Commons publications.
pub const COMMONS_PUBLICATION: u32 = 7100;

/// Tags applied to Commons events to indicate their nature.
///
/// These classify the intent behind publishing an event to
/// the shared space, helping consumers filter and prioritize.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommonsTag {
    /// Relevant across community boundaries.
    CrossCommunity,
    /// Intended for open discussion.
    PublicDiscourse,
    /// Educational or reference material.
    SharedKnowledge,
    /// A question seeking diverse input.
    OpenQuestion,
    /// Official announcement from a community.
    Announcement,
}

impl CommonsTag {
    /// String representation used in ORP event tags.
    pub fn as_str(&self) -> &'static str {
        match self {
            CommonsTag::CrossCommunity => "cross-community",
            CommonsTag::PublicDiscourse => "public-discourse",
            CommonsTag::SharedKnowledge => "shared-knowledge",
            CommonsTag::OpenQuestion => "open-question",
            CommonsTag::Announcement => "announcement",
        }
    }

    /// Parse from a tag string. Returns `None` if unrecognized.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cross-community" => Some(CommonsTag::CrossCommunity),
            "public-discourse" => Some(CommonsTag::PublicDiscourse),
            "shared-knowledge" => Some(CommonsTag::SharedKnowledge),
            "open-question" => Some(CommonsTag::OpenQuestion),
            "announcement" => Some(CommonsTag::Announcement),
            _ => None,
        }
    }
}

impl std::fmt::Display for CommonsTag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// An event published to the Globe Commons.
///
/// Wraps a standard `OmniEvent` with commons-specific metadata.
/// The underlying event is unchanged — this is a view layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommonsEvent {
    /// The underlying signed event (kind `COMMONS_PUBLICATION`).
    pub event: OmniEvent,
    /// Which community published this to the Commons, if any.
    pub source_community: Option<String>,
    /// Classification tags for this Commons publication.
    pub commons_tags: Vec<CommonsTag>,
}

impl CommonsEvent {
    /// Create a new Commons event wrapping a standard OmniEvent.
    pub fn new(event: OmniEvent) -> Self {
        Self {
            source_community: event.tag_value("community").map(String::from),
            commons_tags: event
                .tag_values("commons_tag")
                .into_iter()
                .filter_map(CommonsTag::parse)
                .collect(),
            event,
        }
    }

    /// The ID of the original event this publication references.
    pub fn referenced_event_id(&self) -> Option<&str> {
        self.event.tag_value("e")
    }

    /// Whether this Commons event carries a specific tag.
    pub fn has_tag(&self, tag: CommonsTag) -> bool {
        self.commons_tags.contains(&tag)
    }

    /// Whether this event is a valid Commons publication.
    pub fn is_valid(&self) -> bool {
        self.event.kind == COMMONS_PUBLICATION && self.referenced_event_id().is_some()
    }
}

/// How a community chooses to publish events to the Commons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CommonsPublishPolicy {
    /// Members can choose per-event whether to publish to Commons.
    Default,
    /// Community explicitly selects events for Commons publication.
    OptIn,
    /// Community auto-publishes all non-private events to Commons.
    OptOut,
    /// Community does not participate in the Commons.
    Disabled,
}

impl std::fmt::Display for CommonsPublishPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommonsPublishPolicy::Default => write!(f, "default"),
            CommonsPublishPolicy::OptIn => write!(f, "opt-in"),
            CommonsPublishPolicy::OptOut => write!(f, "opt-out"),
            CommonsPublishPolicy::Disabled => write!(f, "disabled"),
        }
    }
}

/// Per-community Commons policy, stored in the community Charter.
///
/// Controls how the community interacts with the shared Commons space.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommonsPolicy {
    /// How the community publishes events to Commons.
    pub publish_to_commons: CommonsPublishPolicy,
    /// Whether the community reads from Commons (default: true).
    /// Almost no reason to disable reading.
    pub read_from_commons: bool,
    /// Which relays carry Commons events for this community.
    /// Defaults to the community's own relays if empty.
    pub commons_relay_urls: Vec<String>,
}

impl Default for CommonsPolicy {
    fn default() -> Self {
        Self {
            publish_to_commons: CommonsPublishPolicy::Default,
            read_from_commons: true,
            commons_relay_urls: Vec::new(),
        }
    }
}

impl CommonsPolicy {
    /// Whether publishing to Commons is allowed at all.
    pub fn can_publish(&self) -> bool {
        self.publish_to_commons != CommonsPublishPolicy::Disabled
    }

    /// Whether auto-publishing is active (OptOut mode).
    pub fn auto_publishes(&self) -> bool {
        self.publish_to_commons == CommonsPublishPolicy::OptOut
    }

    /// Whether member choice is available (Default or OptIn mode).
    pub fn member_choice_available(&self) -> bool {
        matches!(
            self.publish_to_commons,
            CommonsPublishPolicy::Default | CommonsPublishPolicy::OptIn
        )
    }

    /// The effective relay URLs for Commons traffic.
    /// Returns the configured URLs, or an empty slice if none set
    /// (caller should fall back to community relays).
    pub fn effective_relay_urls(&self) -> &[String] {
        &self.commons_relay_urls
    }
}

/// Filter extension for Commons-specific queries.
///
/// Layered on top of `OmniFilter` to add Commons semantics.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CommonsFilter {
    /// Only return events tagged as Commons publications.
    pub commons_only: bool,
    /// Filter to events from a specific community's publications.
    pub source_community: Option<String>,
    /// Filter by Commons tag type.
    pub commons_tag: Option<CommonsTag>,
}

impl CommonsFilter {
    /// Create a filter that returns only Commons events.
    pub fn commons_only() -> Self {
        Self {
            commons_only: true,
            ..Default::default()
        }
    }

    /// Create a filter for a specific community's Commons publications.
    pub fn from_community(community_id: impl Into<String>) -> Self {
        Self {
            commons_only: true,
            source_community: Some(community_id.into()),
            commons_tag: None,
        }
    }

    /// Create a filter for a specific Commons tag.
    pub fn with_tag(tag: CommonsTag) -> Self {
        Self {
            commons_only: true,
            source_community: None,
            commons_tag: Some(tag),
        }
    }

    /// Whether a CommonsEvent matches this filter.
    pub fn matches(&self, event: &CommonsEvent) -> bool {
        // commons_only check: must be a valid Commons publication.
        if self.commons_only && !event.is_valid() {
            return false;
        }

        // Source community filter.
        if let Some(ref community) = self.source_community {
            match &event.source_community {
                Some(src) if src == community => {}
                _ => return false,
            }
        }

        // Tag filter.
        if let Some(tag) = self.commons_tag {
            if !event.has_tag(tag) {
                return false;
            }
        }

        true
    }

    /// Build an `OmniFilter` that selects Commons publication events.
    ///
    /// The returned filter can be sent to relays to fetch Commons data.
    pub fn to_omni_filter(&self) -> OmniFilter {
        let mut filter = OmniFilter {
            kinds: Some(vec![COMMONS_PUBLICATION]),
            ..Default::default()
        };

        if let Some(ref community) = self.source_community {
            filter
                .tag_filters
                .entry('c')
                .or_default()
                .push(community.clone());
        }

        if let Some(tag) = self.commons_tag {
            filter
                .tag_filters
                .entry('t')
                .or_default()
                .push(tag.as_str().to_string());
        }

        filter
    }
}

/// Build the ORP event tags for a Commons publication.
///
/// Produces tags suitable for an `UnsignedEvent` that will become
/// a `COMMONS_PUBLICATION` event on the wire.
pub fn commons_publication_tags(
    original_event_id: &str,
    source_community: Option<&str>,
    tags: &[CommonsTag],
) -> Vec<Vec<String>> {
    let mut event_tags = vec![vec!["e".into(), original_event_id.into()]];

    if let Some(community) = source_community {
        event_tags.push(vec!["community".into(), community.into()]);
    }

    for tag in tags {
        event_tags.push(vec!["commons_tag".into(), tag.as_str().into()]);
    }

    event_tags
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_commons_event(
        original_id: &str,
        community: Option<&str>,
        tags: &[CommonsTag],
    ) -> CommonsEvent {
        let mut event_tags = vec![vec!["e".into(), original_id.into()]];
        if let Some(c) = community {
            event_tags.push(vec!["community".into(), c.into()]);
        }
        for t in tags {
            event_tags.push(vec!["commons_tag".into(), t.as_str().into()]);
        }

        let event = OmniEvent {
            id: format!("commons-{original_id}"),
            author: "cpub1test".into(),
            created_at: 1000,
            kind: COMMONS_PUBLICATION,
            tags: event_tags,
            content: String::new(),
            sig: "sig".into(),
        };

        CommonsEvent::new(event)
    }

    fn make_non_commons_event() -> OmniEvent {
        OmniEvent {
            id: "regular-event".into(),
            author: "cpub1test".into(),
            created_at: 1000,
            kind: 1,
            tags: vec![],
            content: "hello".into(),
            sig: "sig".into(),
        }
    }

    // --- CommonsTag tests ---

    #[test]
    fn commons_tag_round_trip() {
        let tags = [
            CommonsTag::CrossCommunity,
            CommonsTag::PublicDiscourse,
            CommonsTag::SharedKnowledge,
            CommonsTag::OpenQuestion,
            CommonsTag::Announcement,
        ];
        for tag in &tags {
            let s = tag.as_str();
            let parsed = CommonsTag::parse(s).unwrap();
            assert_eq!(*tag, parsed);
        }
    }

    #[test]
    fn commons_tag_unknown_returns_none() {
        assert!(CommonsTag::parse("unknown-tag").is_none());
        assert!(CommonsTag::parse("").is_none());
    }

    #[test]
    fn commons_tag_display() {
        assert_eq!(CommonsTag::CrossCommunity.to_string(), "cross-community");
        assert_eq!(CommonsTag::Announcement.to_string(), "announcement");
    }

    #[test]
    fn commons_tag_serde() {
        let tag = CommonsTag::SharedKnowledge;
        let json = serde_json::to_string(&tag).unwrap();
        let restored: CommonsTag = serde_json::from_str(&json).unwrap();
        assert_eq!(tag, restored);
    }

    #[test]
    fn all_tags_distinct_strings() {
        let tags = [
            CommonsTag::CrossCommunity,
            CommonsTag::PublicDiscourse,
            CommonsTag::SharedKnowledge,
            CommonsTag::OpenQuestion,
            CommonsTag::Announcement,
        ];
        let strings: Vec<&str> = tags.iter().map(|t| t.as_str()).collect();
        let unique: std::collections::HashSet<&str> = strings.iter().copied().collect();
        assert_eq!(strings.len(), unique.len());
    }

    // --- CommonsEvent tests ---

    #[test]
    fn create_commons_event() {
        let evt = make_commons_event(
            "orig-123",
            Some("design-guild"),
            &[CommonsTag::SharedKnowledge],
        );
        assert_eq!(evt.source_community.as_deref(), Some("design-guild"));
        assert_eq!(evt.commons_tags, vec![CommonsTag::SharedKnowledge]);
        assert_eq!(evt.referenced_event_id(), Some("orig-123"));
        assert!(evt.is_valid());
    }

    #[test]
    fn commons_event_multiple_tags() {
        let evt = make_commons_event(
            "orig-456",
            None,
            &[CommonsTag::CrossCommunity, CommonsTag::OpenQuestion],
        );
        assert!(evt.has_tag(CommonsTag::CrossCommunity));
        assert!(evt.has_tag(CommonsTag::OpenQuestion));
        assert!(!evt.has_tag(CommonsTag::Announcement));
    }

    #[test]
    fn commons_event_no_community() {
        let evt = make_commons_event("orig-789", None, &[CommonsTag::PublicDiscourse]);
        assert!(evt.source_community.is_none());
        assert!(evt.is_valid());
    }

    #[test]
    fn commons_event_invalid_kind() {
        let event = make_non_commons_event();
        let commons = CommonsEvent::new(event);
        assert!(!commons.is_valid());
    }

    #[test]
    fn commons_event_invalid_no_reference() {
        let event = OmniEvent {
            id: "no-ref".into(),
            author: "cpub1test".into(),
            created_at: 1000,
            kind: COMMONS_PUBLICATION,
            tags: vec![],
            content: String::new(),
            sig: "sig".into(),
        };
        let commons = CommonsEvent::new(event);
        assert!(!commons.is_valid());
    }

    #[test]
    fn commons_event_serde() {
        let evt = make_commons_event("orig-serde", Some("guild"), &[CommonsTag::Announcement]);
        let json = serde_json::to_string(&evt).unwrap();
        let restored: CommonsEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(evt, restored);
    }

    // --- CommonsPublishPolicy tests ---

    #[test]
    fn publish_policy_display() {
        assert_eq!(CommonsPublishPolicy::Default.to_string(), "default");
        assert_eq!(CommonsPublishPolicy::OptIn.to_string(), "opt-in");
        assert_eq!(CommonsPublishPolicy::OptOut.to_string(), "opt-out");
        assert_eq!(CommonsPublishPolicy::Disabled.to_string(), "disabled");
    }

    #[test]
    fn publish_policy_serde() {
        let policy = CommonsPublishPolicy::OptIn;
        let json = serde_json::to_string(&policy).unwrap();
        let restored: CommonsPublishPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, restored);
    }

    // --- CommonsPolicy tests ---

    #[test]
    fn default_policy() {
        let policy = CommonsPolicy::default();
        assert_eq!(policy.publish_to_commons, CommonsPublishPolicy::Default);
        assert!(policy.read_from_commons);
        assert!(policy.commons_relay_urls.is_empty());
        assert!(policy.can_publish());
        assert!(!policy.auto_publishes());
        assert!(policy.member_choice_available());
    }

    #[test]
    fn disabled_policy() {
        let policy = CommonsPolicy {
            publish_to_commons: CommonsPublishPolicy::Disabled,
            read_from_commons: false,
            commons_relay_urls: Vec::new(),
        };
        assert!(!policy.can_publish());
        assert!(!policy.auto_publishes());
        assert!(!policy.member_choice_available());
    }

    #[test]
    fn opt_out_policy_auto_publishes() {
        let policy = CommonsPolicy {
            publish_to_commons: CommonsPublishPolicy::OptOut,
            read_from_commons: true,
            commons_relay_urls: vec!["wss://commons.relay".into()],
        };
        assert!(policy.can_publish());
        assert!(policy.auto_publishes());
        assert!(!policy.member_choice_available());
    }

    #[test]
    fn opt_in_policy() {
        let policy = CommonsPolicy {
            publish_to_commons: CommonsPublishPolicy::OptIn,
            read_from_commons: true,
            commons_relay_urls: Vec::new(),
        };
        assert!(policy.can_publish());
        assert!(!policy.auto_publishes());
        assert!(policy.member_choice_available());
    }

    #[test]
    fn policy_relay_urls() {
        let policy = CommonsPolicy {
            publish_to_commons: CommonsPublishPolicy::Default,
            read_from_commons: true,
            commons_relay_urls: vec![
                "wss://commons1.relay".into(),
                "wss://commons2.relay".into(),
            ],
        };
        assert_eq!(policy.effective_relay_urls().len(), 2);
    }

    #[test]
    fn policy_serde() {
        let policy = CommonsPolicy {
            publish_to_commons: CommonsPublishPolicy::OptIn,
            read_from_commons: false,
            commons_relay_urls: vec!["wss://test.relay".into()],
        };
        let json = serde_json::to_string(&policy).unwrap();
        let restored: CommonsPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(policy, restored);
    }

    // --- CommonsFilter tests ---

    #[test]
    fn default_filter_matches_everything() {
        let filter = CommonsFilter::default();
        let evt = make_commons_event("x", None, &[]);
        // Default filter has commons_only=false, so it matches.
        assert!(filter.matches(&evt));
    }

    #[test]
    fn commons_only_filter() {
        let filter = CommonsFilter::commons_only();
        let valid = make_commons_event("x", None, &[]);
        assert!(filter.matches(&valid));

        // Non-commons event fails.
        let non_commons = CommonsEvent::new(make_non_commons_event());
        assert!(!filter.matches(&non_commons));
    }

    #[test]
    fn community_filter() {
        let filter = CommonsFilter::from_community("guild-a");

        let match_evt =
            make_commons_event("x", Some("guild-a"), &[CommonsTag::CrossCommunity]);
        assert!(filter.matches(&match_evt));

        let no_match =
            make_commons_event("y", Some("guild-b"), &[CommonsTag::CrossCommunity]);
        assert!(!filter.matches(&no_match));

        let no_community = make_commons_event("z", None, &[CommonsTag::CrossCommunity]);
        assert!(!filter.matches(&no_community));
    }

    #[test]
    fn tag_filter() {
        let filter = CommonsFilter::with_tag(CommonsTag::Announcement);

        let match_evt = make_commons_event("x", None, &[CommonsTag::Announcement]);
        assert!(filter.matches(&match_evt));

        let no_match = make_commons_event("y", None, &[CommonsTag::OpenQuestion]);
        assert!(!filter.matches(&no_match));
    }

    #[test]
    fn combined_filter() {
        let filter = CommonsFilter {
            commons_only: true,
            source_community: Some("guild-a".into()),
            commons_tag: Some(CommonsTag::SharedKnowledge),
        };

        let full_match = make_commons_event(
            "x",
            Some("guild-a"),
            &[CommonsTag::SharedKnowledge],
        );
        assert!(filter.matches(&full_match));

        // Wrong community.
        let wrong_community = make_commons_event(
            "y",
            Some("guild-b"),
            &[CommonsTag::SharedKnowledge],
        );
        assert!(!filter.matches(&wrong_community));

        // Wrong tag.
        let wrong_tag = make_commons_event(
            "z",
            Some("guild-a"),
            &[CommonsTag::Announcement],
        );
        assert!(!filter.matches(&wrong_tag));
    }

    #[test]
    fn filter_serde() {
        let filter = CommonsFilter {
            commons_only: true,
            source_community: Some("guild".into()),
            commons_tag: Some(CommonsTag::OpenQuestion),
        };
        let json = serde_json::to_string(&filter).unwrap();
        let restored: CommonsFilter = serde_json::from_str(&json).unwrap();
        assert_eq!(filter, restored);
    }

    #[test]
    fn to_omni_filter_basic() {
        let filter = CommonsFilter::commons_only();
        let omni = filter.to_omni_filter();
        assert_eq!(omni.kinds, Some(vec![COMMONS_PUBLICATION]));
        assert!(omni.tag_filters.is_empty());
    }

    #[test]
    fn to_omni_filter_with_community() {
        let filter = CommonsFilter::from_community("guild-a");
        let omni = filter.to_omni_filter();
        assert_eq!(omni.kinds, Some(vec![COMMONS_PUBLICATION]));
        assert_eq!(
            omni.tag_filters.get(&'c'),
            Some(&vec!["guild-a".to_string()])
        );
    }

    #[test]
    fn to_omni_filter_with_tag() {
        let filter = CommonsFilter::with_tag(CommonsTag::PublicDiscourse);
        let omni = filter.to_omni_filter();
        assert_eq!(
            omni.tag_filters.get(&'t'),
            Some(&vec!["public-discourse".to_string()])
        );
    }

    // --- Tag builder tests ---

    #[test]
    fn publication_tags_basic() {
        let tags = commons_publication_tags("orig-001", None, &[]);
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], vec!["e", "orig-001"]);
    }

    #[test]
    fn publication_tags_with_community() {
        let tags = commons_publication_tags("orig-002", Some("guild"), &[]);
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[1], vec!["community", "guild"]);
    }

    #[test]
    fn publication_tags_with_all() {
        let tags = commons_publication_tags(
            "orig-003",
            Some("guild"),
            &[CommonsTag::CrossCommunity, CommonsTag::Announcement],
        );
        assert_eq!(tags.len(), 4); // e + community + 2 commons_tags
        assert_eq!(tags[2], vec!["commons_tag", "cross-community"]);
        assert_eq!(tags[3], vec!["commons_tag", "announcement"]);
    }

    #[test]
    fn commons_publication_kind_value() {
        assert_eq!(COMMONS_PUBLICATION, 7100);
    }
}
