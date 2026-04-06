use std::cmp::Ordering;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A row in the manifest database, representing a tracked .idea file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManifestEntry {
    pub id: Uuid,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Extended type (e.g., "music", "drawing").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extended_type: Option<String>,
    /// Creator's public key.
    pub creator: String,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collective_id: Option<Uuid>,
    /// JSON-encoded ideas::Header for fast browsing without decryption.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_cache: Option<String>,
}

impl ManifestEntry {
    /// Create a ManifestEntry from an ideas::Header and a relative path.
    pub fn from_header(header: &ideas::Header, path: String) -> Self {
        Self {
            id: header.id,
            path,
            title: None,
            extended_type: header.extended_type.clone(),
            creator: header.creator.public_key.clone(),
            created_at: header.created,
            modified_at: header.modified,
            collective_id: None,
            header_cache: serde_json::to_string(header).ok(),
        }
    }
}

/// Which field to sort manifest entries by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortField {
    /// Sort by title (alphabetical, entries without titles sort last).
    Title,
    /// Sort by creation timestamp.
    Created,
    /// Sort by last-modified timestamp.
    Modified,
    /// Sort by extended type (alphabetical, entries without types sort last).
    Type,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortOrder {
    /// Ascending (A-Z, oldest first).
    Asc,
    /// Descending (Z-A, newest first).
    Desc,
}

/// Filter criteria for querying manifest entries.
///
/// All fields are optional. An entry matches if it satisfies ALL
/// specified criteria (AND logic). An empty filter matches everything.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdeaFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub creator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collective_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extended_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_after: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub modified_before: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_contains: Option<String>,

    // --- Sorting & pagination ---

    /// Field to sort results by.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<SortField>,
    /// Sort direction (defaults to ascending if `sort_by` is set).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<SortOrder>,
    /// Maximum number of results to return.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    /// Number of results to skip before returning (for pagination).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
}

impl IdeaFilter {
    /// Create a new empty filter that matches all entries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: filter by creator public key.
    pub fn creator(mut self, creator: impl Into<String>) -> Self {
        self.creator = Some(creator.into());
        self
    }

    /// Builder: filter by collective.
    pub fn collective(mut self, id: Uuid) -> Self {
        self.collective_id = Some(id);
        self
    }

    /// Builder: filter by extended type.
    pub fn extended_type(mut self, t: impl Into<String>) -> Self {
        self.extended_type = Some(t.into());
        self
    }

    /// Builder: filter by path prefix.
    pub fn path_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.path_prefix = Some(prefix.into());
        self
    }

    /// Builder: filter by title substring (case-insensitive).
    pub fn title_contains(mut self, search: impl Into<String>) -> Self {
        self.title_contains = Some(search.into());
        self
    }

    /// Builder: set sort field and direction.
    pub fn sort(mut self, field: SortField, order: SortOrder) -> Self {
        self.sort_by = Some(field);
        self.sort_order = Some(order);
        self
    }

    /// Builder: set maximum number of results.
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    /// Builder: set offset (skip N results).
    pub fn offset(mut self, n: usize) -> Self {
        self.offset = Some(n);
        self
    }

    /// Apply sorting and pagination to a filtered result set.
    ///
    /// Call this on the output of filtering. If `sort_by` is set, entries
    /// are sorted in-place. Then `offset` and `limit` are applied.
    pub fn apply_sort_and_paginate<'a>(&self, mut entries: Vec<&'a ManifestEntry>) -> Vec<&'a ManifestEntry> {
        // Sort if requested.
        if let Some(field) = self.sort_by {
            let order = self.sort_order.unwrap_or(SortOrder::Asc);
            entries.sort_by(|a, b| {
                let cmp = Self::compare_by_field(a, b, field);
                match order {
                    SortOrder::Asc => cmp,
                    SortOrder::Desc => cmp.reverse(),
                }
            });
        }

        // Apply offset.
        let start = self.offset.unwrap_or(0).min(entries.len());
        let entries = entries.split_off(start);

        // Apply limit.
        match self.limit {
            Some(limit) => entries.into_iter().take(limit).collect(),
            None => entries,
        }
    }

    /// Compare two entries by the given sort field.
    fn compare_by_field(a: &ManifestEntry, b: &ManifestEntry, field: SortField) -> Ordering {
        match field {
            SortField::Title => {
                // Entries without titles sort last.
                match (&a.title, &b.title) {
                    (Some(at), Some(bt)) => at.to_lowercase().cmp(&bt.to_lowercase()),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                }
            }
            SortField::Created => a.created_at.cmp(&b.created_at),
            SortField::Modified => a.modified_at.cmp(&b.modified_at),
            SortField::Type => {
                // Entries without types sort last.
                match (&a.extended_type, &b.extended_type) {
                    (Some(at), Some(bt)) => at.to_lowercase().cmp(&bt.to_lowercase()),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                }
            }
        }
    }

    /// Check if an entry matches this filter.
    pub fn matches(&self, entry: &ManifestEntry) -> bool {
        if let Some(ref c) = self.creator {
            if &entry.creator != c {
                return false;
            }
        }
        if let Some(ref cid) = self.collective_id {
            if entry.collective_id.as_ref() != Some(cid) {
                return false;
            }
        }
        if let Some(ref t) = self.extended_type {
            if entry.extended_type.as_ref() != Some(t) {
                return false;
            }
        }
        if let Some(ref after) = self.modified_after {
            if &entry.modified_at <= after {
                return false;
            }
        }
        if let Some(ref before) = self.modified_before {
            if &entry.modified_at >= before {
                return false;
            }
        }
        if let Some(ref prefix) = self.path_prefix {
            if !entry.path.starts_with(prefix.as_str()) {
                return false;
            }
        }
        if let Some(ref title_search) = self.title_contains {
            match &entry.title {
                Some(title) => {
                    if !title.to_lowercase().contains(&title_search.to_lowercase()) {
                        return false;
                    }
                }
                None => return false,
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(id: Uuid, path: &str, creator: &str) -> ManifestEntry {
        ManifestEntry {
            id,
            path: path.to_string(),
            title: Some("Test Idea".to_string()),
            extended_type: Some("music".to_string()),
            creator: creator.to_string(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            collective_id: None,
            header_cache: None,
        }
    }

    #[test]
    fn manifest_entry_serde_round_trip() {
        let entry = make_entry(Uuid::new_v4(), "Personal/song.idea", "cpub1abc");
        let json = serde_json::to_string(&entry).unwrap();
        let restored: ManifestEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, entry.id);
        assert_eq!(restored.path, entry.path);
        assert_eq!(restored.creator, entry.creator);
    }

    #[test]
    fn idea_filter_matches_all_when_empty() {
        let filter = IdeaFilter::new();
        let entry = make_entry(Uuid::new_v4(), "Personal/test.idea", "cpub1abc");
        assert!(filter.matches(&entry));
    }

    #[test]
    fn idea_filter_creator_match() {
        let filter = IdeaFilter::new().creator("cpub1abc");
        let entry = make_entry(Uuid::new_v4(), "test.idea", "cpub1abc");
        assert!(filter.matches(&entry));

        let other = make_entry(Uuid::new_v4(), "test.idea", "cpub1xyz");
        assert!(!filter.matches(&other));
    }

    #[test]
    fn idea_filter_combined_criteria() {
        let filter = IdeaFilter::new()
            .creator("cpub1abc")
            .extended_type("music")
            .path_prefix("Personal/");

        let mut entry = make_entry(Uuid::new_v4(), "Personal/song.idea", "cpub1abc");
        entry.extended_type = Some("music".to_string());
        assert!(filter.matches(&entry));

        // Wrong type
        entry.extended_type = Some("drawing".to_string());
        assert!(!filter.matches(&entry));
    }

    #[test]
    fn idea_filter_title_contains_case_insensitive() {
        let filter = IdeaFilter::new().title_contains("TEST");
        let entry = make_entry(Uuid::new_v4(), "test.idea", "cpub1abc");
        assert!(filter.matches(&entry)); // "Test Idea" contains "test"

        let mut no_title = make_entry(Uuid::new_v4(), "test.idea", "cpub1abc");
        no_title.title = None;
        assert!(!filter.matches(&no_title));
    }

    // --- Sorting tests ---

    fn make_entry_with_title(title: &str, ext_type: Option<&str>) -> ManifestEntry {
        ManifestEntry {
            id: Uuid::new_v4(),
            path: format!("Personal/{}.idea", title.to_lowercase().replace(' ', "-")),
            title: Some(title.to_string()),
            extended_type: ext_type.map(|s| s.to_string()),
            creator: "cpub1test".to_string(),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            collective_id: None,
            header_cache: None,
        }
    }

    #[test]
    fn sort_by_title_ascending() {
        let mut a = make_entry_with_title("Zebra Notes", Some("text"));
        let mut b = make_entry_with_title("Apple Notes", Some("text"));
        let mut c = make_entry_with_title("Mango Notes", Some("text"));
        // Ensure distinct timestamps so we know sort is by title, not insertion order.
        a.created_at = Utc::now();
        b.created_at = Utc::now();
        c.created_at = Utc::now();

        let entries = vec![&a, &b, &c];
        let filter = IdeaFilter::new().sort(SortField::Title, SortOrder::Asc);
        let sorted = filter.apply_sort_and_paginate(entries);

        assert_eq!(sorted[0].title.as_deref(), Some("Apple Notes"));
        assert_eq!(sorted[1].title.as_deref(), Some("Mango Notes"));
        assert_eq!(sorted[2].title.as_deref(), Some("Zebra Notes"));
    }

    #[test]
    fn sort_by_title_descending() {
        let a = make_entry_with_title("Alpha", Some("text"));
        let b = make_entry_with_title("Beta", Some("text"));
        let c = make_entry_with_title("Gamma", Some("text"));

        let entries = vec![&a, &b, &c];
        let filter = IdeaFilter::new().sort(SortField::Title, SortOrder::Desc);
        let sorted = filter.apply_sort_and_paginate(entries);

        assert_eq!(sorted[0].title.as_deref(), Some("Gamma"));
        assert_eq!(sorted[1].title.as_deref(), Some("Beta"));
        assert_eq!(sorted[2].title.as_deref(), Some("Alpha"));
    }

    #[test]
    fn sort_by_title_none_sorts_last() {
        let a = make_entry_with_title("Beta", Some("text"));
        let mut b = make_entry_with_title("Alpha", Some("text"));
        b.title = None; // No title — should sort last in ascending.

        let entries = vec![&a, &b];
        let filter = IdeaFilter::new().sort(SortField::Title, SortOrder::Asc);
        let sorted = filter.apply_sort_and_paginate(entries);

        assert_eq!(sorted[0].title.as_deref(), Some("Beta"));
        assert!(sorted[1].title.is_none());
    }

    #[test]
    fn sort_by_created() {
        use chrono::Duration;

        let mut old = make_entry_with_title("Old", Some("text"));
        old.created_at = Utc::now() - Duration::hours(2);
        let mut mid = make_entry_with_title("Mid", Some("text"));
        mid.created_at = Utc::now() - Duration::hours(1);
        let new = make_entry_with_title("New", Some("text"));

        let entries = vec![&mid, &new, &old];
        let filter = IdeaFilter::new().sort(SortField::Created, SortOrder::Asc);
        let sorted = filter.apply_sort_and_paginate(entries);

        assert_eq!(sorted[0].title.as_deref(), Some("Old"));
        assert_eq!(sorted[1].title.as_deref(), Some("Mid"));
        assert_eq!(sorted[2].title.as_deref(), Some("New"));
    }

    #[test]
    fn sort_by_modified_descending() {
        use chrono::Duration;

        let mut old = make_entry_with_title("Old", Some("text"));
        old.modified_at = Utc::now() - Duration::hours(2);
        let mut mid = make_entry_with_title("Mid", Some("text"));
        mid.modified_at = Utc::now() - Duration::hours(1);
        let new = make_entry_with_title("New", Some("text"));

        let entries = vec![&old, &mid, &new];
        let filter = IdeaFilter::new().sort(SortField::Modified, SortOrder::Desc);
        let sorted = filter.apply_sort_and_paginate(entries);

        assert_eq!(sorted[0].title.as_deref(), Some("New"));
        assert_eq!(sorted[1].title.as_deref(), Some("Mid"));
        assert_eq!(sorted[2].title.as_deref(), Some("Old"));
    }

    #[test]
    fn sort_by_type() {
        let a = make_entry_with_title("Note A", Some("text"));
        let b = make_entry_with_title("Note B", Some("music"));
        let c = make_entry_with_title("Note C", Some("drawing"));

        let entries = vec![&a, &b, &c];
        let filter = IdeaFilter::new().sort(SortField::Type, SortOrder::Asc);
        let sorted = filter.apply_sort_and_paginate(entries);

        assert_eq!(sorted[0].extended_type.as_deref(), Some("drawing"));
        assert_eq!(sorted[1].extended_type.as_deref(), Some("music"));
        assert_eq!(sorted[2].extended_type.as_deref(), Some("text"));
    }

    // --- Pagination tests ---

    #[test]
    fn pagination_limit() {
        let a = make_entry_with_title("A", Some("text"));
        let b = make_entry_with_title("B", Some("text"));
        let c = make_entry_with_title("C", Some("text"));

        let entries = vec![&a, &b, &c];
        let filter = IdeaFilter::new()
            .sort(SortField::Title, SortOrder::Asc)
            .limit(2);
        let result = filter.apply_sort_and_paginate(entries);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].title.as_deref(), Some("A"));
        assert_eq!(result[1].title.as_deref(), Some("B"));
    }

    #[test]
    fn pagination_offset() {
        let a = make_entry_with_title("A", Some("text"));
        let b = make_entry_with_title("B", Some("text"));
        let c = make_entry_with_title("C", Some("text"));

        let entries = vec![&a, &b, &c];
        let filter = IdeaFilter::new()
            .sort(SortField::Title, SortOrder::Asc)
            .offset(1);
        let result = filter.apply_sort_and_paginate(entries);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].title.as_deref(), Some("B"));
        assert_eq!(result[1].title.as_deref(), Some("C"));
    }

    #[test]
    fn pagination_offset_and_limit() {
        let a = make_entry_with_title("A", Some("text"));
        let b = make_entry_with_title("B", Some("text"));
        let c = make_entry_with_title("C", Some("text"));
        let d = make_entry_with_title("D", Some("text"));

        let entries = vec![&a, &b, &c, &d];
        let filter = IdeaFilter::new()
            .sort(SortField::Title, SortOrder::Asc)
            .offset(1)
            .limit(2);
        let result = filter.apply_sort_and_paginate(entries);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].title.as_deref(), Some("B"));
        assert_eq!(result[1].title.as_deref(), Some("C"));
    }

    #[test]
    fn pagination_offset_beyond_results() {
        let a = make_entry_with_title("A", Some("text"));

        let entries = vec![&a];
        let filter = IdeaFilter::new().offset(10);
        let result = filter.apply_sort_and_paginate(entries);

        assert!(result.is_empty());
    }

    #[test]
    fn pagination_limit_larger_than_results() {
        let a = make_entry_with_title("A", Some("text"));

        let entries = vec![&a];
        let filter = IdeaFilter::new().limit(100);
        let result = filter.apply_sort_and_paginate(entries);

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn filter_with_sort_backward_compat_serde() {
        // Old JSON without sort/pagination fields should deserialize fine.
        let json = r#"{"creator":"cpub1abc"}"#;
        let filter: IdeaFilter = serde_json::from_str(json).unwrap();
        assert_eq!(filter.creator.as_deref(), Some("cpub1abc"));
        assert!(filter.sort_by.is_none());
        assert!(filter.sort_order.is_none());
        assert!(filter.limit.is_none());
        assert!(filter.offset.is_none());
    }

    #[test]
    fn filter_with_sort_serde_round_trip() {
        let filter = IdeaFilter::new()
            .creator("cpub1abc")
            .sort(SortField::Modified, SortOrder::Desc)
            .limit(20)
            .offset(40);
        let json = serde_json::to_string(&filter).unwrap();
        let restored: IdeaFilter = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.sort_by, Some(SortField::Modified));
        assert_eq!(restored.sort_order, Some(SortOrder::Desc));
        assert_eq!(restored.limit, Some(20));
        assert_eq!(restored.offset, Some(40));
    }
}
