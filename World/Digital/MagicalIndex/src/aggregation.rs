//! Aggregation — count, sum, min/max, group-by over indexed events.
//!
//! Runs SQL aggregate queries against the structured metadata and tag
//! tables. Does not touch FTS5 — aggregation is about numbers, not text.

use serde::{Deserialize, Serialize};

// -- Aggregation types --

/// What to aggregate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AggregateFunction {
    /// Count matching events.
    Count,
    /// Sum a numeric tag value.
    Sum(String),
    /// Minimum of a numeric tag value.
    Min(String),
    /// Maximum of a numeric tag value.
    Max(String),
    /// Average of a numeric tag value.
    Avg(String),
}

/// How to group results.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum GroupBy {
    /// Group by event kind.
    Kind,
    /// Group by author.
    Author,
    /// Group by a tag key's value.
    Tag(String),
}

/// An aggregation query over indexed events.
///
/// Filters narrow the event set, then the aggregate function runs
/// over the filtered set, optionally grouped by a dimension.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AggregateQuery {
    /// The aggregation function (default: Count).
    pub function: Option<AggregateFunction>,
    /// Optional grouping dimension.
    pub group_by: Option<GroupBy>,
    /// Filter by event kinds.
    pub kinds: Option<Vec<u32>>,
    /// Filter by authors.
    pub authors: Option<Vec<String>>,
    /// Events created after this timestamp (inclusive).
    pub since: Option<i64>,
    /// Events created before this timestamp (inclusive).
    pub until: Option<i64>,
    /// Maximum number of groups to return (default: 100).
    pub limit: usize,
}

impl AggregateQuery {
    /// Count all indexed events.
    pub fn count() -> Self {
        Self {
            function: Some(AggregateFunction::Count),
            limit: 100,
            ..Default::default()
        }
    }

    /// Count events grouped by a dimension.
    pub fn count_by(group: GroupBy) -> Self {
        Self {
            function: Some(AggregateFunction::Count),
            group_by: Some(group),
            limit: 100,
            ..Default::default()
        }
    }

    /// Sum a numeric tag value.
    pub fn sum(tag_key: &str) -> Self {
        Self {
            function: Some(AggregateFunction::Sum(tag_key.into())),
            limit: 100,
            ..Default::default()
        }
    }

    /// Min of a numeric tag value.
    pub fn min(tag_key: &str) -> Self {
        Self {
            function: Some(AggregateFunction::Min(tag_key.into())),
            limit: 100,
            ..Default::default()
        }
    }

    /// Max of a numeric tag value.
    pub fn max(tag_key: &str) -> Self {
        Self {
            function: Some(AggregateFunction::Max(tag_key.into())),
            limit: 100,
            ..Default::default()
        }
    }

    /// Average of a numeric tag value.
    pub fn avg(tag_key: &str) -> Self {
        Self {
            function: Some(AggregateFunction::Avg(tag_key.into())),
            limit: 100,
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

    /// Set the group limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

/// Result of an aggregation query.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AggregateResponse {
    /// The scalar result (for ungrouped queries).
    pub value: Option<f64>,
    /// Grouped results (for group-by queries).
    pub groups: Vec<AggregateGroup>,
}

/// A single group in a grouped aggregation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AggregateGroup {
    /// The group key (kind number, author pubkey, or tag value).
    pub key: String,
    /// The aggregate value for this group.
    pub value: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_query() {
        let q = AggregateQuery::count();
        assert!(matches!(q.function, Some(AggregateFunction::Count)));
        assert!(q.group_by.is_none());
    }

    #[test]
    fn count_by_kind() {
        let q = AggregateQuery::count_by(GroupBy::Kind);
        assert!(matches!(q.function, Some(AggregateFunction::Count)));
        assert!(matches!(q.group_by, Some(GroupBy::Kind)));
    }

    #[test]
    fn sum_query_with_filters() {
        let q = AggregateQuery::sum("downloads")
            .with_kinds(vec![1])
            .with_time_range(Some(1000), Some(5000));
        assert!(matches!(q.function, Some(AggregateFunction::Sum(_))));
        assert_eq!(q.kinds, Some(vec![1]));
        assert_eq!(q.since, Some(1000));
        assert_eq!(q.until, Some(5000));
    }

    #[test]
    fn aggregate_response_defaults() {
        let r = AggregateResponse::default();
        assert!(r.value.is_none());
        assert!(r.groups.is_empty());
    }

    #[test]
    fn serde_round_trip() {
        let q = AggregateQuery::count_by(GroupBy::Tag("t".into()))
            .with_kinds(vec![1, 7030])
            .with_limit(50);
        let json = serde_json::to_string(&q).unwrap();
        let loaded: AggregateQuery = serde_json::from_str(&json).unwrap();
        assert!(matches!(loaded.function, Some(AggregateFunction::Count)));
        assert!(matches!(loaded.group_by, Some(GroupBy::Tag(_))));
        assert_eq!(loaded.kinds, Some(vec![1, 7030]));
        assert_eq!(loaded.limit, 50);
    }
}
