//! Compound queries — rich, multi-dimensional queries over indexed events.
//!
//! Goes beyond text search to support:
//! - Tag-based filtering (e.g., "all events tagged 'logo'")
//! - Sorting by any field or tag value
//! - Faceted search (filter + count by multiple dimensions)
//! - Logical combinations of conditions
//!
//! Compound queries work against the structured metadata and tag tables,
//! optionally combined with FTS5 text search.

use serde::{Deserialize, Serialize};

// -- Sort --

/// A field to sort results by.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SortField {
    /// Sort by creation timestamp (default).
    CreatedAt,
    /// Sort by event kind number.
    Kind,
    /// Sort by author pubkey (lexicographic).
    Author,
    /// Sort by FTS5 relevance score (only valid with text search).
    Relevance,
    /// Sort by a tag value. Events missing this tag sort last.
    TagValue(String),
}

/// Sort direction.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum SortDirection {
    /// Ascending (oldest first, lowest first).
    Asc,
    /// Descending (newest first, highest first).
    #[default]
    Desc,
}

/// A sort clause: field + direction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SortClause {
    /// The field to sort by.
    pub field: SortField,
    /// The direction.
    pub direction: SortDirection,
}

impl SortClause {
    /// Sort by creation time, newest first.
    pub fn newest_first() -> Self {
        Self {
            field: SortField::CreatedAt,
            direction: SortDirection::Desc,
        }
    }

    /// Sort by creation time, oldest first.
    pub fn oldest_first() -> Self {
        Self {
            field: SortField::CreatedAt,
            direction: SortDirection::Asc,
        }
    }

    /// Sort by relevance (highest first). Only meaningful with text search.
    pub fn by_relevance() -> Self {
        Self {
            field: SortField::Relevance,
            direction: SortDirection::Desc,
        }
    }

    /// Sort by a tag value.
    pub fn by_tag(tag_name: &str, direction: SortDirection) -> Self {
        Self {
            field: SortField::TagValue(tag_name.into()),
            direction,
        }
    }
}

// -- Tag filter --

/// A filter condition on a tag key-value pair.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TagFilter {
    /// The tag key (e.g., "t", "d", "status").
    pub key: String,
    /// The tag values to match (OR within a single TagFilter).
    pub values: Vec<String>,
}

impl TagFilter {
    /// Match events with this tag key set to any of the given values.
    pub fn new(key: &str, values: Vec<String>) -> Self {
        Self {
            key: key.into(),
            values,
        }
    }

    /// Match events with this tag key set to a single value.
    pub fn exact(key: &str, value: &str) -> Self {
        Self {
            key: key.into(),
            values: vec![value.into()],
        }
    }
}

// -- Facet request --

/// A request for facet counts along a dimension.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum FacetRequest {
    /// Count events by kind.
    ByKind,
    /// Count events by author.
    ByAuthor,
    /// Count events by tag key (e.g., count distinct values of "t" tags).
    ByTag(String),
}

/// A single facet bucket: a value and its count.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FacetBucket {
    /// The facet value (kind number, author pubkey, or tag value).
    pub value: String,
    /// Number of matching events in this bucket.
    pub count: u64,
}

/// Facet results for one dimension.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FacetResult {
    /// Which dimension this facet covers.
    pub dimension: String,
    /// The buckets, sorted by count descending.
    pub buckets: Vec<FacetBucket>,
}

// -- Compound query --

/// A rich, multi-dimensional query.
///
/// Combines text search, metadata filters, tag filters, sorting,
/// and faceted search into a single query.
///
/// All conditions are AND'd: every filter must match.
/// Within a `TagFilter`, values are OR'd: any value matches.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CompoundQuery {
    /// Optional full-text search (uses FTS5).
    pub text: Option<String>,
    /// Filter by event kinds.
    pub kinds: Option<Vec<u32>>,
    /// Filter by authors (pubkey hex).
    pub authors: Option<Vec<String>>,
    /// Events created after this timestamp (inclusive).
    pub since: Option<i64>,
    /// Events created before this timestamp (inclusive).
    pub until: Option<i64>,
    /// Tag filters. All must match (AND). Values within each are OR'd.
    pub tag_filters: Vec<TagFilter>,
    /// Sort order. First clause is primary, etc. Default: newest first.
    pub sort: Vec<SortClause>,
    /// Facet requests. Results include counts per dimension.
    pub facets: Vec<FacetRequest>,
    /// Maximum results to return (default: 20).
    pub limit: usize,
    /// Offset for pagination (default: 0).
    pub offset: usize,
}

impl CompoundQuery {
    /// Start a new compound query with no filters.
    pub fn new() -> Self {
        Self {
            limit: 20,
            ..Default::default()
        }
    }

    /// Start with a text search.
    pub fn text(text: &str) -> Self {
        Self {
            text: Some(text.into()),
            limit: 20,
            ..Default::default()
        }
    }

    /// Filter by event kinds.
    pub fn with_kinds(mut self, kinds: Vec<u32>) -> Self {
        self.kinds = Some(kinds);
        self
    }

    /// Filter by authors.
    pub fn with_authors(mut self, authors: Vec<String>) -> Self {
        self.authors = Some(authors);
        self
    }

    /// Filter by time range.
    pub fn with_time_range(mut self, since: Option<i64>, until: Option<i64>) -> Self {
        self.since = since;
        self.until = until;
        self
    }

    /// Add a tag filter. Multiple tag filters are AND'd.
    pub fn with_tag(mut self, key: &str, values: Vec<String>) -> Self {
        self.tag_filters.push(TagFilter::new(key, values));
        self
    }

    /// Add an exact tag filter (single value).
    pub fn with_tag_exact(mut self, key: &str, value: &str) -> Self {
        self.tag_filters.push(TagFilter::exact(key, value));
        self
    }

    /// Set the sort order. Replaces any previous sort.
    pub fn with_sort(mut self, sort: Vec<SortClause>) -> Self {
        self.sort = sort;
        self
    }

    /// Sort by a single field. Replaces any previous sort.
    pub fn sorted_by(mut self, clause: SortClause) -> Self {
        self.sort = vec![clause];
        self
    }

    /// Request facet counts along a dimension.
    pub fn with_facet(mut self, facet: FacetRequest) -> Self {
        self.facets.push(facet);
        self
    }

    /// Set the result limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set the result offset (pagination).
    pub fn with_offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }
}

/// Response from a compound query.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CompoundResponse {
    /// The matching events (metadata only, not full events).
    pub results: Vec<CompoundResult>,
    /// Total number of matches (before limit/offset).
    pub total_matches: usize,
    /// Facet results, one per requested dimension.
    pub facets: Vec<FacetResult>,
}

/// A single result from a compound query.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompoundResult {
    /// The event's ID.
    pub event_id: String,
    /// The event's author (pubkey hex).
    pub author: String,
    /// The event kind.
    pub kind: u32,
    /// When the event was created (Unix timestamp).
    pub created_at: i64,
    /// Relevance score (only populated when text search is used).
    pub relevance: Option<f64>,
    /// Text snippet (only populated when text search is used).
    pub snippet: Option<String>,
    /// Tag values for the sort tag (if sorting by tag).
    pub sort_tag_value: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compound_query_builder() {
        let q = CompoundQuery::text("logo")
            .with_kinds(vec![1, 7030])
            .with_tag_exact("t", "logo")
            .with_tag_exact("status", "approved")
            .with_time_range(Some(1000), None)
            .sorted_by(SortClause::newest_first())
            .with_facet(FacetRequest::ByKind)
            .with_limit(50)
            .with_offset(10);

        assert_eq!(q.text, Some("logo".into()));
        assert_eq!(q.kinds, Some(vec![1, 7030]));
        assert_eq!(q.tag_filters.len(), 2);
        assert_eq!(q.tag_filters[0].key, "t");
        assert_eq!(q.tag_filters[0].values, vec!["logo"]);
        assert_eq!(q.tag_filters[1].key, "status");
        assert_eq!(q.since, Some(1000));
        assert_eq!(q.sort.len(), 1);
        assert_eq!(q.facets.len(), 1);
        assert_eq!(q.limit, 50);
        assert_eq!(q.offset, 10);
    }

    #[test]
    fn compound_query_defaults() {
        let q = CompoundQuery::new();
        assert!(q.text.is_none());
        assert!(q.kinds.is_none());
        assert_eq!(q.limit, 20);
        assert_eq!(q.offset, 0);
        assert!(q.sort.is_empty());
        assert!(q.facets.is_empty());
    }

    #[test]
    fn sort_clause_constructors() {
        let newest = SortClause::newest_first();
        assert_eq!(newest.field, SortField::CreatedAt);
        assert_eq!(newest.direction, SortDirection::Desc);

        let oldest = SortClause::oldest_first();
        assert_eq!(oldest.direction, SortDirection::Asc);

        let relevance = SortClause::by_relevance();
        assert_eq!(relevance.field, SortField::Relevance);

        let tag = SortClause::by_tag("downloads", SortDirection::Desc);
        assert_eq!(tag.field, SortField::TagValue("downloads".into()));
    }

    #[test]
    fn tag_filter_constructors() {
        let multi = TagFilter::new("t", vec!["logo".into(), "icon".into()]);
        assert_eq!(multi.key, "t");
        assert_eq!(multi.values.len(), 2);

        let exact = TagFilter::exact("status", "approved");
        assert_eq!(exact.key, "status");
        assert_eq!(exact.values, vec!["approved"]);
    }

    #[test]
    fn compound_query_serde_round_trip() {
        let q = CompoundQuery::text("test")
            .with_kinds(vec![1])
            .with_tag_exact("t", "rust")
            .sorted_by(SortClause::newest_first())
            .with_facet(FacetRequest::ByKind)
            .with_limit(10);

        let json = serde_json::to_string(&q).unwrap();
        let loaded: CompoundQuery = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.text, Some("test".into()));
        assert_eq!(loaded.kinds, Some(vec![1]));
        assert_eq!(loaded.tag_filters.len(), 1);
        assert_eq!(loaded.limit, 10);
    }

    #[test]
    fn compound_response_defaults() {
        let r = CompoundResponse::default();
        assert!(r.results.is_empty());
        assert_eq!(r.total_matches, 0);
        assert!(r.facets.is_empty());
    }
}
