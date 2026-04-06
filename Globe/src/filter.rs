use std::collections::HashMap;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::event::OmniEvent;
use crate::kind::{self, Subsystem};

/// A subscription filter for querying events from relays.
///
/// All conditions within a single filter are AND'd together.
/// Multiple filters in a subscription are OR'd (any match = delivered).
///
/// Tag filters use single-letter keys: `'e'` for event refs, `'p'` for
/// pubkey refs, `'d'` for d-tags, etc.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OmniFilter {
    /// Match specific event IDs.
    pub ids: Option<Vec<String>>,
    /// Match specific authors (pubkey hex).
    pub authors: Option<Vec<String>>,
    /// Match specific event kinds.
    pub kinds: Option<Vec<u32>>,
    /// Events created after this timestamp (inclusive).
    pub since: Option<i64>,
    /// Events created before this timestamp (inclusive).
    pub until: Option<i64>,
    /// Maximum number of events to return.
    pub limit: Option<usize>,
    /// Tag filters: key is single letter, values are matched with OR.
    pub tag_filters: HashMap<char, Vec<String>>,
    /// Search query text (server-side only — delegated to SearchHandler).
    /// Ignored in client-side `matches()`.
    pub search: Option<String>,
}

impl OmniFilter {
    /// Check whether an event matches this filter.
    ///
    /// All conditions are AND'd: every present field must match.
    /// Arrays within a field are OR'd: at least one value must match.
    pub fn matches(&self, event: &OmniEvent) -> bool {
        // IDs.
        if let Some(ids) = &self.ids {
            if !ids.iter().any(|id| id == &event.id) {
                return false;
            }
        }

        // Authors.
        if let Some(authors) = &self.authors {
            if !authors.iter().any(|a| a == &event.author) {
                return false;
            }
        }

        // Kinds.
        if let Some(kinds) = &self.kinds {
            if !kinds.contains(&event.kind) {
                return false;
            }
        }

        // Since.
        if let Some(since) = self.since {
            if event.created_at < since {
                return false;
            }
        }

        // Until.
        if let Some(until) = self.until {
            if event.created_at > until {
                return false;
            }
        }

        // Tag filters.
        for (tag_name, filter_values) in &self.tag_filters {
            let tag_str = tag_name.to_string();
            let event_values = event.tag_values(&tag_str);
            if !filter_values.iter().any(|fv| event_values.contains(&fv.as_str())) {
                return false;
            }
        }

        true
    }

    // -- Convenience builders --

    /// Filter for events from a specific subsystem.
    pub fn for_subsystem(subsystem: Subsystem) -> Self {
        let range = match subsystem {
            Subsystem::Advisor => kind::ADVISOR_RANGE,
            Subsystem::Bulwark => kind::BULWARK_RANGE,
            Subsystem::Crown => kind::CROWN_RANGE,
            Subsystem::Divinity => kind::DIVINITY_RANGE,
            Subsystem::Equipment => kind::EQUIPMENT_RANGE,
            Subsystem::Fortune => kind::FORTUNE_RANGE,
            Subsystem::Globe => kind::GLOBE_RANGE,
            Subsystem::Hall => kind::HALL_RANGE,
            Subsystem::Ideas => kind::IDEAS_RANGE,
            Subsystem::Jail => kind::JAIL_RANGE,
            Subsystem::Kingdom => kind::KINGDOM_RANGE,
            Subsystem::Lingo => kind::LINGO_RANGE,
            Subsystem::Magic => kind::MAGIC_RANGE,
            Subsystem::Nexus => kind::NEXUS_RANGE,
            Subsystem::Oracle => kind::ORACLE_RANGE,
            Subsystem::Polity => kind::POLITY_RANGE,
            Subsystem::Quest => kind::QUEST_RANGE,
            Subsystem::Regalia => kind::REGALIA_RANGE,
            Subsystem::Sentinal => kind::SENTINAL_RANGE,
            Subsystem::Throne => kind::THRONE_RANGE,
            Subsystem::Universe => kind::UNIVERSE_RANGE,
            Subsystem::Vault => kind::VAULT_RANGE,
            Subsystem::World => kind::WORLD_RANGE,
            Subsystem::X => kind::X_RANGE,
            Subsystem::Yoke => kind::YOKE_RANGE,
            Subsystem::Zeitgeist => kind::ZEITGEIST_RANGE,
            _ => return Self::default(),
        };
        Self {
            kinds: Some(range.collect()),
            ..Default::default()
        }
    }

    /// Filter for a user's profile (kind 0, limit 1).
    pub fn for_profile(pubkey: &str) -> Self {
        Self {
            kinds: Some(vec![kind::PROFILE]),
            authors: Some(vec![pubkey.to_string()]),
            limit: Some(1),
            ..Default::default()
        }
    }

    /// Filter for a user's contact list (kind 3, limit 1).
    pub fn for_contact_list(pubkey: &str) -> Self {
        Self {
            kinds: Some(vec![kind::CONTACT_LIST]),
            authors: Some(vec![pubkey.to_string()]),
            limit: Some(1),
            ..Default::default()
        }
    }

    /// Filter for all gospel registry records, optionally since a timestamp.
    ///
    /// Matches name events (7000-7004) and relay hints (7010).
    /// Used by gospel sync to request records from peers.
    pub fn for_gospel(since: Option<i64>) -> Self {
        Self {
            kinds: Some(kind::GOSPEL_REGISTRY_KINDS.to_vec()),
            since,
            ..Default::default()
        }
    }

    /// Filter for a specific author's relay hints.
    pub fn for_relay_hints(author: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('d', vec!["relay-hints".to_string()]);
        Self {
            kinds: Some(vec![kind::RELAY_HINT]),
            authors: Some(vec![author.to_string()]),
            tag_filters,
            limit: Some(1),
            ..Default::default()
        }
    }

    /// Filter for a domain name record.
    pub fn for_name(name: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('d', vec![name.to_string()]);
        Self {
            kinds: Some(vec![kind::NAME_CLAIM]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for an asset announcement by hash.
    pub fn for_asset(hash: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('d', vec![hash.to_string()]);
        Self {
            kinds: Some(vec![kind::ASSET_ANNOUNCE]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for all asset announcements, optionally since a timestamp.
    pub fn for_asset_announcements(since: Option<i64>) -> Self {
        Self {
            kinds: Some(vec![kind::ASSET_ANNOUNCE]),
            since,
            ..Default::default()
        }
    }

    /// Filter for communicator signaling events addressed to me.
    ///
    /// Matches offer (5100), answer (5101), end (5102), and ICE
    /// candidate (5103) events tagged with my crown_id.
    pub fn for_communicator_signals(my_crown_id: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('p', vec![my_crown_id.to_string()]);
        Self {
            kinds: Some(vec![
                kind::COMMUNICATOR_OFFER,
                kind::COMMUNICATOR_ANSWER,
                kind::COMMUNICATOR_END,
                kind::ICE_CANDIDATE,
            ]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for live stream events, optionally since a timestamp.
    ///
    /// Matches stream announce (5110), update (5111), and end (5112).
    pub fn for_live_streams(since: Option<i64>) -> Self {
        Self {
            kinds: Some(vec![
                kind::STREAM_ANNOUNCE,
                kind::STREAM_UPDATE,
                kind::STREAM_END,
            ]),
            since,
            ..Default::default()
        }
    }

    /// Filter for events related to a specific communicator session.
    pub fn for_session(session_id: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('s', vec![session_id.to_string()]);
        Self {
            kinds: Some(vec![
                kind::COMMUNICATOR_OFFER,
                kind::COMMUNICATOR_ANSWER,
                kind::COMMUNICATOR_END,
            ]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for community beacons, optionally since a timestamp.
    pub fn for_beacons(since: Option<i64>) -> Self {
        Self {
            kinds: Some(vec![kind::BEACON, kind::BEACON_UPDATE]),
            since,
            ..Default::default()
        }
    }

    /// Filter for beacons matching a specific topic tag.
    pub fn for_beacons_by_tag(tag: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('t', vec![tag.to_string()]);
        Self {
            kinds: Some(vec![kind::BEACON, kind::BEACON_UPDATE]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for Network Key events addressed to me.
    pub fn for_key_events(my_crown_id: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('p', vec![my_crown_id.to_string()]);
        Self {
            kinds: Some(vec![
                kind::KEY_DELIVERY,
                kind::KEY_ROTATION,
                kind::INVITATION,
            ]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for a chunk manifest by content hash.
    pub fn for_chunk_manifest(content_hash: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('d', vec![content_hash.to_string()]);
        Self {
            kinds: Some(vec![kind::CHUNK_MANIFEST]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for all chunk manifests, optionally since a timestamp.
    pub fn for_chunk_manifests(since: Option<i64>) -> Self {
        Self {
            kinds: Some(vec![kind::CHUNK_MANIFEST]),
            since,
            ..Default::default()
        }
    }

    /// Filter for stream recordings by a specific author.
    pub fn for_stream_recordings(author: &str) -> Self {
        Self {
            kinds: Some(vec![kind::STREAM_RECORDING]),
            authors: Some(vec![author.to_string()]),
            ..Default::default()
        }
    }

    /// Filter for invitations addressed to me.
    pub fn for_invitations(my_crown_id: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('p', vec![my_crown_id.to_string()]);
        Self {
            kinds: Some(vec![kind::INVITATION]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for Tower lighthouse announcements, optionally since a timestamp.
    pub fn for_lighthouses(since: Option<i64>) -> Self {
        Self {
            kinds: Some(vec![kind::LIGHTHOUSE_ANNOUNCE]),
            since,
            ..Default::default()
        }
    }

    /// Filter for Tower lighthouse announcements by mode (e.g., "pharos" or "harbor").
    pub fn for_lighthouses_by_mode(mode: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('m', vec![mode.to_string()]);
        Self {
            kinds: Some(vec![kind::LIGHTHOUSE_ANNOUNCE]),
            tag_filters,
            ..Default::default()
        }
    }

    // -- Yoke filters (history & provenance) --

    /// Filter for all Yoke events (kinds 25000-25006), optionally since a timestamp.
    pub fn for_yoke(since: Option<i64>) -> Self {
        Self {
            kinds: Some(kind::YOKE_KINDS.to_vec()),
            since,
            ..Default::default()
        }
    }

    /// Filter for Yoke relationships involving an entity (as source or target).
    ///
    /// To query both directions, create two filters and OR them in a subscription.
    /// This one matches by source tag.
    pub fn for_yoke_relationships_from(source_id: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('s', vec![source_id.to_string()]);
        Self {
            kinds: Some(vec![kind::YOKE_RELATIONSHIP]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for Yoke relationships targeting an entity.
    pub fn for_yoke_relationships_to(target_id: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('t', vec![target_id.to_string()]);
        Self {
            kinds: Some(vec![kind::YOKE_RELATIONSHIP]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for Yoke relationships of a specific type.
    pub fn for_yoke_relationships_by_type(rel_type: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('r', vec![rel_type.to_string()]);
        Self {
            kinds: Some(vec![kind::YOKE_RELATIONSHIP]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for version tags, branches, and merges for a specific .idea.
    pub fn for_yoke_versions(idea_id: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('d', vec![idea_id.to_string()]);
        Self {
            kinds: Some(vec![
                kind::YOKE_VERSION_TAG,
                kind::YOKE_BRANCH,
                kind::YOKE_MERGE,
            ]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for Yoke milestones in a community.
    pub fn for_yoke_milestones(community_id: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('c', vec![community_id.to_string()]);
        Self {
            kinds: Some(vec![kind::YOKE_MILESTONE]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for Yoke ceremonies in a community.
    pub fn for_yoke_ceremonies(community_id: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('c', vec![community_id.to_string()]);
        Self {
            kinds: Some(vec![kind::YOKE_CEREMONY]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for Yoke ceremonies of a specific type.
    pub fn for_yoke_ceremonies_by_type(ceremony_type: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('y', vec![ceremony_type.to_string()]);
        Self {
            kinds: Some(vec![kind::YOKE_CEREMONY]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for Yoke activity records by actor.
    pub fn for_yoke_activities_by_actor(actor_crown_id: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('a', vec![actor_crown_id.to_string()]);
        Self {
            kinds: Some(vec![kind::YOKE_ACTIVITY]),
            tag_filters,
            ..Default::default()
        }
    }

    /// Filter for a full-text search query (server-side).
    ///
    /// The `search` field is handled by the relay's SearchHandler, not
    /// by client-side `matches()`. Other filter fields (kinds, authors,
    /// since/until) can be combined to narrow results.
    pub fn for_search(query: &str) -> Self {
        Self {
            search: Some(query.to_string()),
            ..Default::default()
        }
    }

    /// Add a search query to an existing filter (builder pattern).
    pub fn with_search(mut self, query: &str) -> Self {
        self.search = Some(query.to_string());
        self
    }

    /// Filter for Yoke activity records targeting a specific entity.
    pub fn for_yoke_activities_for_target(target_id: &str) -> Self {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('t', vec![target_id.to_string()]);
        Self {
            kinds: Some(vec![kind::YOKE_ACTIVITY]),
            tag_filters,
            ..Default::default()
        }
    }
}

// -- Custom Serialize: tag_filters as "#e", "#p", etc. --

impl Serialize for OmniFilter {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let field_count = self.ids.is_some() as usize
            + self.authors.is_some() as usize
            + self.kinds.is_some() as usize
            + self.since.is_some() as usize
            + self.until.is_some() as usize
            + self.limit.is_some() as usize
            + self.search.is_some() as usize
            + self.tag_filters.len();

        let mut map = serializer.serialize_map(Some(field_count))?;

        if let Some(ids) = &self.ids {
            map.serialize_entry("ids", ids)?;
        }
        if let Some(authors) = &self.authors {
            map.serialize_entry("authors", authors)?;
        }
        if let Some(kinds) = &self.kinds {
            map.serialize_entry("kinds", kinds)?;
        }
        if let Some(since) = &self.since {
            map.serialize_entry("since", since)?;
        }
        if let Some(until) = &self.until {
            map.serialize_entry("until", until)?;
        }
        if let Some(limit) = &self.limit {
            map.serialize_entry("limit", limit)?;
        }
        if let Some(search) = &self.search {
            map.serialize_entry("search", search)?;
        }
        for (tag, values) in &self.tag_filters {
            let key = format!("#{tag}");
            map.serialize_entry(&key, values)?;
        }

        map.end()
    }
}

// -- Custom Deserialize: parse "#e", "#p", etc. into tag_filters --

impl<'de> Deserialize<'de> for OmniFilter {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(OmniFilterVisitor)
    }
}

struct OmniFilterVisitor;

impl<'de> Visitor<'de> for OmniFilterVisitor {
    type Value = OmniFilter;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("an OmniFilter object")
    }

    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Self::Value, M::Error> {
        let mut filter = OmniFilter::default();

        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "ids" => filter.ids = Some(map.next_value()?),
                "authors" => filter.authors = Some(map.next_value()?),
                "kinds" => filter.kinds = Some(map.next_value()?),
                "since" => filter.since = Some(map.next_value()?),
                "until" => filter.until = Some(map.next_value()?),
                "limit" => filter.limit = Some(map.next_value()?),
                "search" => filter.search = Some(map.next_value()?),
                k if k.starts_with('#') && k.len() == 2 => {
                    // Safety: guard above ensures len == 2 and starts with '#',
                    // so the second char always exists.
                    let tag_char = k.chars().nth(1).expect("tag key has 2 chars");
                    let values: Vec<String> = map.next_value()?;
                    filter.tag_filters.insert(tag_char, values);
                }
                _ => {
                    // Skip unknown fields.
                    let _: serde_json::Value = map.next_value()?;
                }
            }
        }

        Ok(filter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_event(kind: u32, author: &str, tags: Vec<Vec<String>>) -> OmniEvent {
        OmniEvent {
            id: "a".repeat(64),
            author: author.to_string(),
            created_at: Utc::now().timestamp(),
            kind,
            tags,
            content: String::new(),
            sig: "c".repeat(128),
        }
    }

    #[test]
    fn empty_filter_matches_everything() {
        let filter = OmniFilter::default();
        let event = make_event(1, &"b".repeat(64), vec![]);
        assert!(filter.matches(&event));
    }

    #[test]
    fn filter_by_kind() {
        let filter = OmniFilter {
            kinds: Some(vec![1, 7000]),
            ..Default::default()
        };
        assert!(filter.matches(&make_event(1, &"b".repeat(64), vec![])));
        assert!(filter.matches(&make_event(7000, &"b".repeat(64), vec![])));
        assert!(!filter.matches(&make_event(2, &"b".repeat(64), vec![])));
    }

    #[test]
    fn filter_by_author() {
        let author = "a".repeat(64);
        let filter = OmniFilter {
            authors: Some(vec![author.clone()]),
            ..Default::default()
        };
        assert!(filter.matches(&make_event(1, &author, vec![])));
        assert!(!filter.matches(&make_event(1, &"b".repeat(64), vec![])));
    }

    #[test]
    fn filter_by_since_until() {
        let now = Utc::now().timestamp();
        let mut event = make_event(1, &"b".repeat(64), vec![]);
        event.created_at = now;

        let filter = OmniFilter {
            since: Some(now - 100),
            until: Some(now + 100),
            ..Default::default()
        };
        assert!(filter.matches(&event));

        let old_filter = OmniFilter {
            since: Some(now + 10),
            ..Default::default()
        };
        assert!(!old_filter.matches(&event));
    }

    #[test]
    fn filter_by_tag() {
        let event = make_event(
            1,
            &"b".repeat(64),
            vec![vec!["e".into(), "event123".into()]],
        );

        let mut tag_filters = HashMap::new();
        tag_filters.insert('e', vec!["event123".into()]);
        let filter = OmniFilter {
            tag_filters,
            ..Default::default()
        };
        assert!(filter.matches(&event));

        let mut wrong_tags = HashMap::new();
        wrong_tags.insert('e', vec!["wrong".into()]);
        let wrong_filter = OmniFilter {
            tag_filters: wrong_tags,
            ..Default::default()
        };
        assert!(!wrong_filter.matches(&event));
    }

    #[test]
    fn and_logic_all_conditions_must_match() {
        let author = "a".repeat(64);
        let event = make_event(1, &author, vec![]);

        let filter = OmniFilter {
            kinds: Some(vec![1]),
            authors: Some(vec![author]),
            ..Default::default()
        };
        assert!(filter.matches(&event));

        // Wrong kind + right author = no match.
        let filter2 = OmniFilter {
            kinds: Some(vec![999]),
            authors: Some(vec!["a".repeat(64)]),
            ..Default::default()
        };
        assert!(!filter2.matches(&event));
    }

    #[test]
    fn serde_round_trip_simple() {
        let filter = OmniFilter {
            kinds: Some(vec![1, 7000]),
            authors: Some(vec!["abc".into()]),
            limit: Some(10),
            ..Default::default()
        };
        let json = serde_json::to_string(&filter).unwrap();
        let loaded: OmniFilter = serde_json::from_str(&json).unwrap();
        assert_eq!(filter, loaded);
    }

    #[test]
    fn serde_round_trip_with_tags() {
        let mut tag_filters = HashMap::new();
        tag_filters.insert('e', vec!["abc123".into()]);
        tag_filters.insert('p', vec!["def456".into()]);

        let filter = OmniFilter {
            tag_filters,
            ..Default::default()
        };
        let json = serde_json::to_string(&filter).unwrap();
        assert!(json.contains("\"#e\""));
        assert!(json.contains("\"#p\""));

        let loaded: OmniFilter = serde_json::from_str(&json).unwrap();
        assert_eq!(filter, loaded);
    }

    #[test]
    fn convenience_for_profile() {
        let filter = OmniFilter::for_profile("mypubkey");
        assert_eq!(filter.kinds, Some(vec![0]));
        assert_eq!(filter.authors, Some(vec!["mypubkey".into()]));
        assert_eq!(filter.limit, Some(1));
    }

    #[test]
    fn convenience_for_name() {
        let filter = OmniFilter::for_name("sam.com");
        assert_eq!(filter.kinds, Some(vec![kind::NAME_CLAIM]));
        assert_eq!(
            filter.tag_filters.get(&'d'),
            Some(&vec!["sam.com".to_string()])
        );
    }

    #[test]
    fn convenience_for_gospel() {
        let filter = OmniFilter::for_gospel(None);
        let kinds = filter.kinds.unwrap();
        assert!(kinds.contains(&kind::NAME_CLAIM));
        assert!(kinds.contains(&kind::NAME_REVOKE));
        assert!(kinds.contains(&kind::RELAY_HINT));
        assert!(filter.since.is_none());
    }

    #[test]
    fn convenience_for_gospel_with_since() {
        let filter = OmniFilter::for_gospel(Some(1000));
        assert_eq!(filter.since, Some(1000));
        assert!(filter.kinds.unwrap().contains(&kind::RELAY_HINT));
    }

    #[test]
    fn convenience_for_relay_hints() {
        let filter = OmniFilter::for_relay_hints("mypubkey");
        assert_eq!(filter.kinds, Some(vec![kind::RELAY_HINT]));
        assert_eq!(filter.authors, Some(vec!["mypubkey".into()]));
        assert_eq!(filter.limit, Some(1));
        assert_eq!(
            filter.tag_filters.get(&'d'),
            Some(&vec!["relay-hints".to_string()])
        );
    }

    #[test]
    fn convenience_for_subsystem() {
        let filter = OmniFilter::for_subsystem(Subsystem::Fortune);
        let kinds = filter.kinds.unwrap();
        assert_eq!(kinds.len(), 1000);
        assert_eq!(*kinds.first().unwrap(), 6000);
        assert_eq!(*kinds.last().unwrap(), 6999);
    }

    #[test]
    fn convenience_for_lighthouses() {
        let filter = OmniFilter::for_lighthouses(None);
        assert_eq!(filter.kinds, Some(vec![kind::LIGHTHOUSE_ANNOUNCE]));
        assert!(filter.since.is_none());
    }

    #[test]
    fn convenience_for_lighthouses_with_since() {
        let filter = OmniFilter::for_lighthouses(Some(5000));
        assert_eq!(filter.kinds, Some(vec![kind::LIGHTHOUSE_ANNOUNCE]));
        assert_eq!(filter.since, Some(5000));
    }

    #[test]
    fn convenience_for_lighthouses_by_mode() {
        let filter = OmniFilter::for_lighthouses_by_mode("pharos");
        assert_eq!(filter.kinds, Some(vec![kind::LIGHTHOUSE_ANNOUNCE]));
        assert_eq!(
            filter.tag_filters.get(&'m'),
            Some(&vec!["pharos".to_string()])
        );
    }

    #[test]
    fn convenience_for_yoke() {
        let filter = OmniFilter::for_yoke(None);
        let kinds = filter.kinds.unwrap();
        assert_eq!(kinds.len(), 7);
        assert!(kinds.contains(&kind::YOKE_RELATIONSHIP));
        assert!(kinds.contains(&kind::YOKE_VERSION_TAG));
        assert!(kinds.contains(&kind::YOKE_ACTIVITY));
        assert!(filter.since.is_none());
    }

    #[test]
    fn convenience_for_yoke_with_since() {
        let filter = OmniFilter::for_yoke(Some(5000));
        assert_eq!(filter.since, Some(5000));
        assert!(filter.kinds.unwrap().contains(&kind::YOKE_RELATIONSHIP));
    }

    #[test]
    fn convenience_for_yoke_relationships_from() {
        let filter = OmniFilter::for_yoke_relationships_from("source-event-id");
        assert_eq!(filter.kinds, Some(vec![kind::YOKE_RELATIONSHIP]));
        assert_eq!(
            filter.tag_filters.get(&'s'),
            Some(&vec!["source-event-id".to_string()])
        );
    }

    #[test]
    fn convenience_for_yoke_relationships_to() {
        let filter = OmniFilter::for_yoke_relationships_to("target-event-id");
        assert_eq!(filter.kinds, Some(vec![kind::YOKE_RELATIONSHIP]));
        assert_eq!(
            filter.tag_filters.get(&'t'),
            Some(&vec!["target-event-id".to_string()])
        );
    }

    #[test]
    fn convenience_for_yoke_relationships_by_type() {
        let filter = OmniFilter::for_yoke_relationships_by_type("derived-from");
        assert_eq!(filter.kinds, Some(vec![kind::YOKE_RELATIONSHIP]));
        assert_eq!(
            filter.tag_filters.get(&'r'),
            Some(&vec!["derived-from".to_string()])
        );
    }

    #[test]
    fn convenience_for_yoke_versions() {
        let filter = OmniFilter::for_yoke_versions("idea-uuid-123");
        let kinds = filter.kinds.unwrap();
        assert_eq!(kinds.len(), 3);
        assert!(kinds.contains(&kind::YOKE_VERSION_TAG));
        assert!(kinds.contains(&kind::YOKE_BRANCH));
        assert!(kinds.contains(&kind::YOKE_MERGE));
        assert_eq!(
            filter.tag_filters.get(&'d'),
            Some(&vec!["idea-uuid-123".to_string()])
        );
    }

    #[test]
    fn convenience_for_yoke_milestones() {
        let filter = OmniFilter::for_yoke_milestones("design-guild");
        assert_eq!(filter.kinds, Some(vec![kind::YOKE_MILESTONE]));
        assert_eq!(
            filter.tag_filters.get(&'c'),
            Some(&vec!["design-guild".to_string()])
        );
    }

    #[test]
    fn convenience_for_yoke_ceremonies() {
        let filter = OmniFilter::for_yoke_ceremonies("design-guild");
        assert_eq!(filter.kinds, Some(vec![kind::YOKE_CEREMONY]));
        assert_eq!(
            filter.tag_filters.get(&'c'),
            Some(&vec!["design-guild".to_string()])
        );
    }

    #[test]
    fn convenience_for_yoke_ceremonies_by_type() {
        let filter = OmniFilter::for_yoke_ceremonies_by_type("CovenantOath");
        assert_eq!(filter.kinds, Some(vec![kind::YOKE_CEREMONY]));
        assert_eq!(
            filter.tag_filters.get(&'y'),
            Some(&vec!["CovenantOath".to_string()])
        );
    }

    #[test]
    fn convenience_for_yoke_activities_by_actor() {
        let filter = OmniFilter::for_yoke_activities_by_actor("cpub1alice");
        assert_eq!(filter.kinds, Some(vec![kind::YOKE_ACTIVITY]));
        assert_eq!(
            filter.tag_filters.get(&'a'),
            Some(&vec!["cpub1alice".to_string()])
        );
    }

    #[test]
    fn convenience_for_yoke_activities_for_target() {
        let filter = OmniFilter::for_yoke_activities_for_target("logo-v3");
        assert_eq!(filter.kinds, Some(vec![kind::YOKE_ACTIVITY]));
        assert_eq!(
            filter.tag_filters.get(&'t'),
            Some(&vec!["logo-v3".to_string()])
        );
    }

    #[test]
    fn search_field_serde_round_trip() {
        let filter = OmniFilter::for_search("woodworking");
        let json = serde_json::to_string(&filter).unwrap();
        assert!(json.contains("\"search\""));
        let loaded: OmniFilter = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.search, Some("woodworking".into()));
    }

    #[test]
    fn search_field_ignored_in_matches() {
        // search is server-side only — matches() ignores it.
        let filter = OmniFilter {
            search: Some("woodworking".into()),
            ..Default::default()
        };
        let event = make_event(1, &"b".repeat(64), vec![]);
        assert!(filter.matches(&event));
    }

    #[test]
    fn convenience_for_search() {
        let filter = OmniFilter::for_search("hello world");
        assert_eq!(filter.search, Some("hello world".into()));
        assert!(filter.kinds.is_none());
    }

    #[test]
    fn convenience_with_search() {
        let filter = OmniFilter {
            kinds: Some(vec![1]),
            ..Default::default()
        }
        .with_search("test query");
        assert_eq!(filter.search, Some("test query".into()));
        assert_eq!(filter.kinds, Some(vec![1]));
    }

    #[test]
    fn filter_by_id() {
        let mut event = make_event(1, &"b".repeat(64), vec![]);
        event.id = "d".repeat(64);

        let filter = OmniFilter {
            ids: Some(vec!["d".repeat(64)]),
            ..Default::default()
        };
        assert!(filter.matches(&event));

        let wrong = OmniFilter {
            ids: Some(vec!["e".repeat(64)]),
            ..Default::default()
        };
        assert!(!wrong.matches(&event));
    }
}
