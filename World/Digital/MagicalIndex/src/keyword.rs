//! KeywordIndex — full-text search via SQLite FTS5.
//!
//! The default SearchIndex implementation. Uses FTS5 with the Porter
//! stemmer for English and unicode61 tokenizer for international text.
//! Ranking uses BM25 (built into FTS5).
//!
//! Events are indexed by their content + tags. Metadata (author, kind,
//! created_at) is stored in a companion table for filtering. Structured
//! tags are stored in `search_tags` for compound queries, aggregation,
//! and faceted search.

use std::path::Path;
use std::sync::{Arc, Mutex};

use globe::event::OmniEvent;

/// Return type for `build_base_conditions` — SQL fragments and their bound parameters.
type BaseConditions = (Vec<String>, Vec<Box<dyn rusqlite::types::ToSql>>);
use rusqlite::{params, Connection};

use crate::aggregation::{
    AggregateFunction, AggregateGroup, AggregateQuery, AggregateResponse, GroupBy,
};
use crate::federation_scope::FederationScope;
use crate::compound::{
    CompoundQuery, CompoundResponse, CompoundResult, FacetBucket, FacetRequest, FacetResult,
    SortDirection, SortField,
};
use crate::error::MagicalError;
use crate::query::{SearchQuery, SearchResponse, SearchResult};
use crate::traits::SearchIndex;

/// Full-text search index backed by SQLite FTS5.
///
/// Thread-safe via `Arc<Mutex<Connection>>`. Can be in-memory (tests)
/// or file-backed (production). For encrypted storage, open the
/// connection with SQLCipher before passing it in.
///
/// Supports three query modes:
/// - `search()` — simple text search (via SearchIndex trait)
/// - `compound_search()` — rich multi-dimensional queries
/// - `aggregate()` — count/sum/min/max/avg with grouping
pub struct KeywordIndex {
    conn: Arc<Mutex<Connection>>,
}

impl Clone for KeywordIndex {
    fn clone(&self) -> Self {
        Self {
            conn: self.conn.clone(),
        }
    }
}

impl KeywordIndex {
    /// Create an in-memory index (for tests).
    pub fn in_memory() -> Result<Self, MagicalError> {
        let conn = Connection::open_in_memory()?;
        Self::init_tables(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Open a file-backed index (unencrypted).
    pub fn open(path: &Path) -> Result<Self, MagicalError> {
        let conn = Connection::open(path)?;
        Self::init_tables(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Create from an existing connection (e.g., SQLCipher-encrypted).
    ///
    /// The caller is responsible for setting up encryption (PRAGMA key)
    /// before passing the connection.
    pub fn from_connection(conn: Connection) -> Result<Self, MagicalError> {
        Self::init_tables(&conn)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn init_tables(conn: &Connection) -> Result<(), MagicalError> {
        // FTS5 virtual table for full-text search.
        // content + tags are searchable; event_id is stored but not searched.
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS search_fts USING fts5(
                content,
                tags,
                tokenize='porter unicode61'
            );

            CREATE TABLE IF NOT EXISTS search_meta (
                event_id TEXT PRIMARY KEY,
                author TEXT NOT NULL,
                kind INTEGER NOT NULL,
                created_at INTEGER NOT NULL,
                fts_rowid INTEGER NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_search_meta_kind
                ON search_meta(kind);
            CREATE INDEX IF NOT EXISTS idx_search_meta_author
                ON search_meta(author);
            CREATE INDEX IF NOT EXISTS idx_search_meta_created
                ON search_meta(created_at);

            CREATE TABLE IF NOT EXISTS search_tags (
                event_id TEXT NOT NULL,
                tag_key TEXT NOT NULL,
                tag_value TEXT NOT NULL,
                FOREIGN KEY (event_id) REFERENCES search_meta(event_id)
            );

            CREATE INDEX IF NOT EXISTS idx_search_tags_event
                ON search_tags(event_id);
            CREATE INDEX IF NOT EXISTS idx_search_tags_key_value
                ON search_tags(tag_key, tag_value);",
        )?;
        Ok(())
    }

    // -- Content extraction --

    /// Extract searchable text from an event's content.
    fn extract_content(event: &OmniEvent) -> String {
        // For JSON content (profiles, beacons), try to extract display text.
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&event.content) {
            let mut parts = Vec::new();
            // Common JSON fields that contain searchable text.
            for key in &["name", "display_name", "about", "description", "title"] {
                if let Some(val) = json.get(key).and_then(|v| v.as_str()) {
                    parts.push(val.to_string());
                }
            }
            if !parts.is_empty() {
                return parts.join(" ");
            }
        }
        // Plain text content.
        event.content.clone()
    }

    /// Extract searchable tag text from an event (for FTS5 column).
    fn extract_tags(event: &OmniEvent) -> String {
        let mut parts = Vec::new();
        for tag in &event.tags {
            match tag.first().map(|s| s.as_str()) {
                // d-tags (names, identifiers)
                Some("d") => {
                    if let Some(val) = tag.get(1) {
                        parts.push(val.clone());
                    }
                }
                // t-tags (topic tags)
                Some("t") => {
                    if let Some(val) = tag.get(1) {
                        parts.push(val.clone());
                    }
                }
                // subject tags
                Some("subject") => {
                    if let Some(val) = tag.get(1) {
                        parts.push(val.clone());
                    }
                }
                _ => {}
            }
        }
        parts.join(" ")
    }

    /// Extract structured tag key-value pairs from an event (for search_tags table).
    fn extract_tag_pairs(event: &OmniEvent) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        for tag in &event.tags {
            if let (Some(key), Some(value)) = (tag.first(), tag.get(1)) {
                if !key.is_empty() && !value.is_empty() {
                    pairs.push((key.clone(), value.clone()));
                }
            }
        }
        pairs
    }

    // -- Compound queries --

    /// Execute a compound query with tag filters, sorting, and facets.
    ///
    /// This is the power query API. For simple text search, use `search()`.
    pub fn compound_search(&self, query: &CompoundQuery) -> Result<CompoundResponse, MagicalError> {
        let conn = self.conn.lock().map_err(|e| MagicalError::Index(format!("lock poisoned: {e}")))?;
        let results = Self::execute_compound(&conn, query)?;
        let facets = Self::execute_facets(&conn, query)?;
        Ok(CompoundResponse {
            total_matches: results.0,
            results: results.1,
            facets,
        })
    }

    /// Core compound query execution.
    /// Returns (total_count, paginated_results).
    fn execute_compound(
        conn: &Connection,
        query: &CompoundQuery,
    ) -> Result<(usize, Vec<CompoundResult>), MagicalError> {
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        // Build the base query. Start from search_meta always.
        // If text search is requested, join with FTS5.
        let has_text = query.text.as_ref().is_some_and(|t| {
            let sanitized = sanitize_fts_query(t);
            !sanitized.is_empty()
        });

        let fts_query = query
            .text
            .as_ref()
            .map(|t| sanitize_fts_query(t))
            .unwrap_or_default();

        // Determine if we need to join with search_tags for tag sorting.
        let sort_tag_key = query.sort.first().and_then(|s| match &s.field {
            SortField::TagValue(key) => Some(key.clone()),
            _ => None,
        });

        // SELECT clause
        let mut select = String::from("SELECT m.event_id, m.author, m.kind, m.created_at");
        if has_text {
            select.push_str(", bm25(search_fts) AS rank");
            select.push_str(", snippet(search_fts, 0, '**', '**', '...', 32) AS snip");
        } else {
            select.push_str(", NULL AS rank, NULL AS snip");
        }
        if sort_tag_key.is_some() {
            select.push_str(", sort_tag.tag_value AS sort_val");
        } else {
            select.push_str(", NULL AS sort_val");
        }

        // FROM clause
        let mut from = String::from(" FROM search_meta m");
        if has_text {
            from.push_str(" JOIN search_fts f ON m.fts_rowid = f.rowid");
        }
        if let Some(ref tag_key) = sort_tag_key {
            // LEFT JOIN so events without the sort tag still appear (sorted last).
            let idx = param_values.len() + 1;
            from.push_str(&format!(
                " LEFT JOIN search_tags sort_tag ON sort_tag.event_id = m.event_id AND sort_tag.tag_key = ?{idx}"
            ));
            param_values.push(Box::new(tag_key.clone()));
        }

        // WHERE clause
        let mut conditions = Vec::new();
        if has_text {
            let idx = param_values.len() + 1;
            conditions.push(format!("search_fts MATCH ?{idx}"));
            param_values.push(Box::new(fts_query));
        }

        // Kind filter.
        if let Some(ref kinds) = query.kinds {
            if !kinds.is_empty() {
                let placeholders = Self::add_params(&mut param_values, kinds);
                conditions.push(format!("m.kind IN ({})", placeholders));
            }
        }

        // Author filter.
        if let Some(ref authors) = query.authors {
            if !authors.is_empty() {
                let placeholders = Self::add_string_params(&mut param_values, authors);
                conditions.push(format!("m.author IN ({})", placeholders));
            }
        }

        // Time range.
        if let Some(since) = query.since {
            let idx = param_values.len() + 1;
            conditions.push(format!("m.created_at >= ?{idx}"));
            param_values.push(Box::new(since));
        }
        if let Some(until) = query.until {
            let idx = param_values.len() + 1;
            conditions.push(format!("m.created_at <= ?{idx}"));
            param_values.push(Box::new(until));
        }

        // Tag filters. Each requires a subquery.
        for tf in &query.tag_filters {
            if tf.values.is_empty() {
                continue;
            }
            let value_placeholders = Self::add_string_params(&mut param_values, &tf.values);
            let key_idx = param_values.len() + 1;
            param_values.push(Box::new(tf.key.clone()));
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM search_tags st WHERE st.event_id = m.event_id AND st.tag_key = ?{key_idx} AND st.tag_value IN ({value_placeholders}))"
            ));
        }

        let mut where_clause = String::new();
        if !conditions.is_empty() {
            where_clause = format!(" WHERE {}", conditions.join(" AND "));
        }

        // Count total matches (before limit/offset).
        let count_sql = format!("SELECT COUNT(DISTINCT m.event_id){from}{where_clause}");
        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let total: i64 = conn.query_row(&count_sql, params_ref.as_slice(), |row| row.get(0))?;

        // ORDER BY clause.
        let order_by = Self::build_order_by(query, has_text);

        // LIMIT / OFFSET.
        let limit = if query.limit == 0 { 20 } else { query.limit };
        let limit_idx = param_values.len() + 1;
        let offset_idx = param_values.len() + 2;
        param_values.push(Box::new(limit as i64));
        param_values.push(Box::new(query.offset as i64));

        let full_sql = format!(
            "{select}{from}{where_clause}{order_by} LIMIT ?{limit_idx} OFFSET ?{offset_idx}"
        );

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&full_sql)?;
        let results: Vec<CompoundResult> = stmt
            .query_map(params_ref.as_slice(), |row| {
                let rank: Option<f64> = row.get(4)?;
                let snippet: Option<String> = row.get(5)?;
                let sort_val: Option<String> = row.get(6)?;
                Ok(CompoundResult {
                    event_id: row.get(0)?,
                    author: row.get(1)?,
                    kind: row.get(2)?,
                    created_at: row.get(3)?,
                    relevance: rank.map(bm25_to_score),
                    snippet,
                    sort_tag_value: sort_val,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok((total as usize, results))
    }

    /// Build ORDER BY clause from compound query sort specification.
    fn build_order_by(query: &CompoundQuery, has_text: bool) -> String {
        if query.sort.is_empty() {
            // Default: relevance first (if text search), then newest first.
            return if has_text {
                " ORDER BY rank".to_string()
            } else {
                " ORDER BY m.created_at DESC".to_string()
            };
        }

        let clauses: Vec<String> = query
            .sort
            .iter()
            .filter_map(|s| {
                let dir = match s.direction {
                    SortDirection::Asc => "ASC",
                    SortDirection::Desc => "DESC",
                };
                // NULLs sort last regardless of direction — events missing
                // the sort tag should never float to the top.
                let nulls = "NULLS LAST";
                match &s.field {
                    SortField::CreatedAt => Some(format!("m.created_at {dir}")),
                    SortField::Kind => Some(format!("m.kind {dir}")),
                    SortField::Author => Some(format!("m.author {dir}")),
                    SortField::Relevance => {
                        if has_text {
                            // BM25: lower (more negative) = better match.
                            // ASC gives best matches first, DESC gives worst first.
                            // We flip the direction to match user expectation:
                            // "Desc" = best first = ASC on raw BM25.
                            let bm25_dir = match s.direction {
                                SortDirection::Desc => "ASC",
                                SortDirection::Asc => "DESC",
                            };
                            Some(format!("rank {bm25_dir}"))
                        } else {
                            None
                        }
                    }
                    SortField::TagValue(_) => {
                        Some(format!("sort_val {dir} {nulls}"))
                    }
                }
            })
            .collect();

        if clauses.is_empty() {
            String::new()
        } else {
            format!(" ORDER BY {}", clauses.join(", "))
        }
    }

    /// Execute facet requests for a compound query.
    fn execute_facets(
        conn: &Connection,
        query: &CompoundQuery,
    ) -> Result<Vec<FacetResult>, MagicalError> {
        if query.facets.is_empty() {
            return Ok(Vec::new());
        }

        // Build the base WHERE conditions (same as compound query, minus sort/limit).
        let (base_conditions, base_params) = Self::build_base_conditions(query)?;

        let mut results = Vec::new();
        for facet in &query.facets {
            let result = Self::execute_single_facet(conn, facet, &base_conditions, &base_params)?;
            results.push(result);
        }
        Ok(results)
    }

    /// Build base conditions shared between compound query and facets.
    fn build_base_conditions(
        query: &CompoundQuery,
    ) -> Result<BaseConditions, MagicalError> {
        let mut conditions = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref kinds) = query.kinds {
            if !kinds.is_empty() {
                let placeholders = Self::add_params(&mut param_values, kinds);
                conditions.push(format!("m.kind IN ({})", placeholders));
            }
        }
        if let Some(ref authors) = query.authors {
            if !authors.is_empty() {
                let placeholders = Self::add_string_params(&mut param_values, authors);
                conditions.push(format!("m.author IN ({})", placeholders));
            }
        }
        if let Some(since) = query.since {
            let idx = param_values.len() + 1;
            conditions.push(format!("m.created_at >= ?{idx}"));
            param_values.push(Box::new(since));
        }
        if let Some(until) = query.until {
            let idx = param_values.len() + 1;
            conditions.push(format!("m.created_at <= ?{idx}"));
            param_values.push(Box::new(until));
        }
        for tf in &query.tag_filters {
            if tf.values.is_empty() {
                continue;
            }
            let value_placeholders = Self::add_string_params(&mut param_values, &tf.values);
            let key_idx = param_values.len() + 1;
            param_values.push(Box::new(tf.key.clone()));
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM search_tags st WHERE st.event_id = m.event_id AND st.tag_key = ?{key_idx} AND st.tag_value IN ({value_placeholders}))"
            ));
        }

        Ok((conditions, param_values))
    }

    /// Execute a single facet request.
    fn execute_single_facet(
        conn: &Connection,
        facet: &FacetRequest,
        base_conditions: &[String],
        base_params: &[Box<dyn rusqlite::types::ToSql>],
    ) -> Result<FacetResult, MagicalError> {
        let where_clause = if base_conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", base_conditions.join(" AND "))
        };

        let (sql, dimension) = match facet {
            FacetRequest::ByKind => (
                format!(
                    "SELECT CAST(m.kind AS TEXT), COUNT(*) FROM search_meta m{where_clause} GROUP BY m.kind ORDER BY COUNT(*) DESC LIMIT 100"
                ),
                "kind".to_string(),
            ),
            FacetRequest::ByAuthor => (
                format!(
                    "SELECT m.author, COUNT(*) FROM search_meta m{where_clause} GROUP BY m.author ORDER BY COUNT(*) DESC LIMIT 100"
                ),
                "author".to_string(),
            ),
            FacetRequest::ByTag(tag_key) => {
                let tag_key_idx = base_params.len() + 1;
                let sql = format!(
                    "SELECT st.tag_value, COUNT(DISTINCT st.event_id) FROM search_tags st JOIN search_meta m ON st.event_id = m.event_id{} AND st.tag_key = ?{tag_key_idx} GROUP BY st.tag_value ORDER BY COUNT(DISTINCT st.event_id) DESC LIMIT 100",
                    if base_conditions.is_empty() {
                        " WHERE 1=1".to_string()
                    } else {
                        format!(" WHERE {}", base_conditions.join(" AND "))
                    }
                );
                (sql, format!("tag:{tag_key}"))
            }
        };

        let mut all_params: Vec<&dyn rusqlite::types::ToSql> =
            base_params.iter().map(|p| p.as_ref()).collect();

        // For tag facets, add the tag key param.
        let tag_key_storage;
        if let FacetRequest::ByTag(tag_key) = facet {
            tag_key_storage = tag_key.clone();
            all_params.push(&tag_key_storage);
        }

        let mut stmt = conn.prepare(&sql)?;
        let buckets: Vec<FacetBucket> = stmt
            .query_map(all_params.as_slice(), |row| {
                Ok(FacetBucket {
                    value: row.get(0)?,
                    count: row.get::<_, i64>(1)? as u64,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(FacetResult { dimension, buckets })
    }

    // -- Aggregation --

    /// Execute an aggregation query.
    ///
    /// Returns scalar results (ungrouped) or grouped results.
    pub fn aggregate(&self, query: &AggregateQuery) -> Result<AggregateResponse, MagicalError> {
        let conn = self.conn.lock().map_err(|e| MagicalError::Index(format!("lock poisoned: {e}")))?;
        Self::execute_aggregate(&conn, query)
    }

    fn execute_aggregate(
        conn: &Connection,
        query: &AggregateQuery,
    ) -> Result<AggregateResponse, MagicalError> {
        let function = query
            .function
            .as_ref()
            .ok_or_else(|| MagicalError::Query("aggregate function required".into()))?;

        // Build WHERE conditions.
        let mut conditions = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref kinds) = query.kinds {
            if !kinds.is_empty() {
                let placeholders = Self::add_params(&mut param_values, kinds);
                conditions.push(format!("m.kind IN ({})", placeholders));
            }
        }
        if let Some(ref authors) = query.authors {
            if !authors.is_empty() {
                let placeholders = Self::add_string_params(&mut param_values, authors);
                conditions.push(format!("m.author IN ({})", placeholders));
            }
        }
        if let Some(since) = query.since {
            let idx = param_values.len() + 1;
            conditions.push(format!("m.created_at >= ?{idx}"));
            param_values.push(Box::new(since));
        }
        if let Some(until) = query.until {
            let idx = param_values.len() + 1;
            conditions.push(format!("m.created_at <= ?{idx}"));
            param_values.push(Box::new(until));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        match (&query.group_by, function) {
            // Ungrouped count.
            (None, AggregateFunction::Count) => {
                let sql = format!("SELECT COUNT(*) FROM search_meta m{where_clause}");
                let params_ref: Vec<&dyn rusqlite::types::ToSql> =
                    param_values.iter().map(|p| p.as_ref()).collect();
                let count: i64 = conn.query_row(&sql, params_ref.as_slice(), |row| row.get(0))?;
                Ok(AggregateResponse {
                    value: Some(count as f64),
                    groups: Vec::new(),
                })
            }

            // Ungrouped aggregate on tag value.
            (None, func) => {
                let (agg_fn, tag_key) = match func {
                    AggregateFunction::Sum(k) => ("SUM", k.as_str()),
                    AggregateFunction::Min(k) => ("MIN", k.as_str()),
                    AggregateFunction::Max(k) => ("MAX", k.as_str()),
                    AggregateFunction::Avg(k) => ("AVG", k.as_str()),
                    AggregateFunction::Count => unreachable!(),
                };
                let tag_key_idx = param_values.len() + 1;
                param_values.push(Box::new(tag_key.to_string()));

                let sql = format!(
                    "SELECT {agg_fn}(CAST(st.tag_value AS REAL)) FROM search_tags st JOIN search_meta m ON st.event_id = m.event_id{where_clause}{and}st.tag_key = ?{tag_key_idx}",
                    and = if conditions.is_empty() { " WHERE " } else { " AND " },
                );
                let params_ref: Vec<&dyn rusqlite::types::ToSql> =
                    param_values.iter().map(|p| p.as_ref()).collect();
                let value: Option<f64> =
                    conn.query_row(&sql, params_ref.as_slice(), |row| row.get(0))?;
                Ok(AggregateResponse {
                    value,
                    groups: Vec::new(),
                })
            }

            // Grouped count.
            (Some(group_by), AggregateFunction::Count) => {
                let limit = if query.limit == 0 { 100 } else { query.limit };
                let limit_idx = param_values.len() + 1;
                param_values.push(Box::new(limit as i64));

                let sql = match group_by {
                    GroupBy::Kind => format!(
                        "SELECT CAST(m.kind AS TEXT), COUNT(*) FROM search_meta m{where_clause} GROUP BY m.kind ORDER BY COUNT(*) DESC LIMIT ?{limit_idx}"
                    ),
                    GroupBy::Author => format!(
                        "SELECT m.author, COUNT(*) FROM search_meta m{where_clause} GROUP BY m.author ORDER BY COUNT(*) DESC LIMIT ?{limit_idx}"
                    ),
                    GroupBy::Tag(tag_key) => {
                        let tag_key_idx = param_values.len() + 1;
                        param_values.push(Box::new(tag_key.clone()));
                        format!(
                            "SELECT st.tag_value, COUNT(DISTINCT st.event_id) FROM search_tags st JOIN search_meta m ON st.event_id = m.event_id{}{and}st.tag_key = ?{tag_key_idx} GROUP BY st.tag_value ORDER BY COUNT(DISTINCT st.event_id) DESC LIMIT ?{limit_idx}",
                            where_clause,
                            and = if conditions.is_empty() { " WHERE " } else { " AND " },
                        )
                    }
                };

                let params_ref: Vec<&dyn rusqlite::types::ToSql> =
                    param_values.iter().map(|p| p.as_ref()).collect();
                let mut stmt = conn.prepare(&sql)?;
                let groups: Vec<AggregateGroup> = stmt
                    .query_map(params_ref.as_slice(), |row| {
                        let count: i64 = row.get(1)?;
                        Ok(AggregateGroup {
                            key: row.get(0)?,
                            value: count as f64,
                        })
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(AggregateResponse {
                    value: None,
                    groups,
                })
            }

            // Grouped aggregate on tag value.
            (Some(group_by), func) => {
                let (agg_fn, tag_key) = match func {
                    AggregateFunction::Sum(k) => ("SUM", k.as_str()),
                    AggregateFunction::Min(k) => ("MIN", k.as_str()),
                    AggregateFunction::Max(k) => ("MAX", k.as_str()),
                    AggregateFunction::Avg(k) => ("AVG", k.as_str()),
                    AggregateFunction::Count => unreachable!(),
                };
                let limit = if query.limit == 0 { 100 } else { query.limit };
                let limit_idx = param_values.len() + 1;
                param_values.push(Box::new(limit as i64));
                let tag_key_idx = param_values.len() + 1;
                param_values.push(Box::new(tag_key.to_string()));

                let (group_col, group_expr) = match group_by {
                    GroupBy::Kind => ("CAST(m.kind AS TEXT)", "m.kind"),
                    GroupBy::Author => ("m.author", "m.author"),
                    GroupBy::Tag(group_tag) => {
                        // This is complex: group by one tag, aggregate another.
                        // We join search_tags twice: once for group, once for aggregate.
                        let group_tag_idx = param_values.len() + 1;
                        param_values.push(Box::new(group_tag.clone()));

                        let sql = format!(
                            "SELECT gt.tag_value, {agg_fn}(CAST(at.tag_value AS REAL)) \
                             FROM search_tags gt \
                             JOIN search_meta m ON gt.event_id = m.event_id \
                             JOIN search_tags at ON at.event_id = m.event_id AND at.tag_key = ?{tag_key_idx} \
                             {}{and}gt.tag_key = ?{group_tag_idx} \
                             GROUP BY gt.tag_value \
                             ORDER BY {agg_fn}(CAST(at.tag_value AS REAL)) DESC \
                             LIMIT ?{limit_idx}",
                            where_clause,
                            and = if conditions.is_empty() { " WHERE " } else { " AND " },
                        );

                        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
                            param_values.iter().map(|p| p.as_ref()).collect();
                        let mut stmt = conn.prepare(&sql)?;
                        let groups: Vec<AggregateGroup> = stmt
                            .query_map(params_ref.as_slice(), |row| {
                                Ok(AggregateGroup {
                                    key: row.get(0)?,
                                    value: row.get::<_, f64>(1).unwrap_or(0.0),
                                })
                            })?
                            .filter_map(|r| r.ok())
                            .collect();

                        return Ok(AggregateResponse {
                            value: None,
                            groups,
                        });
                    }
                };

                let sql = format!(
                    "SELECT {group_col}, {agg_fn}(CAST(st.tag_value AS REAL)) \
                     FROM search_tags st \
                     JOIN search_meta m ON st.event_id = m.event_id \
                     {where_clause}{and}st.tag_key = ?{tag_key_idx} \
                     GROUP BY {group_expr} \
                     ORDER BY {agg_fn}(CAST(st.tag_value AS REAL)) DESC \
                     LIMIT ?{limit_idx}",
                    and = if conditions.is_empty() { " WHERE " } else { " AND " },
                );

                let params_ref: Vec<&dyn rusqlite::types::ToSql> =
                    param_values.iter().map(|p| p.as_ref()).collect();
                let mut stmt = conn.prepare(&sql)?;
                let groups: Vec<AggregateGroup> = stmt
                    .query_map(params_ref.as_slice(), |row| {
                        Ok(AggregateGroup {
                            key: row.get(0)?,
                            value: row.get::<_, f64>(1).unwrap_or(0.0),
                        })
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(AggregateResponse {
                    value: None,
                    groups,
                })
            }
        }
    }

    // -- Parameter helpers --

    /// Add u32 params and return comma-separated placeholders.
    fn add_params(
        param_values: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
        values: &[u32],
    ) -> String {
        let start = param_values.len() + 1;
        let placeholders: Vec<String> = values
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", start + i))
            .collect();
        for v in values {
            param_values.push(Box::new(*v));
        }
        placeholders.join(",")
    }

    /// Add String params and return comma-separated placeholders.
    fn add_string_params(
        param_values: &mut Vec<Box<dyn rusqlite::types::ToSql>>,
        values: &[String],
    ) -> String {
        let start = param_values.len() + 1;
        let placeholders: Vec<String> = values
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", start + i))
            .collect();
        for v in values {
            param_values.push(Box::new(v.clone()));
        }
        placeholders.join(",")
    }

    // -- Federation-scoped queries --

    /// Execute a text search scoped to federated communities.
    ///
    /// Like `search()`, but filters results to only include events
    /// tagged with a community in the given `FederationScope`.
    /// An unrestricted scope behaves identically to `search()`.
    pub fn scoped_search(
        &self,
        query: &SearchQuery,
        scope: &FederationScope,
    ) -> Result<SearchResponse, MagicalError> {
        if scope.is_unrestricted() {
            return self.search(query);
        }

        if query.text.is_empty() {
            return Ok(SearchResponse::default());
        }

        let conn = self.conn.lock().map_err(|e| MagicalError::Index(format!("lock poisoned: {e}")))?;

        let fts_query = sanitize_fts_query(&query.text);
        if fts_query.is_empty() {
            return Ok(SearchResponse::default());
        }

        let mut sql = String::from(
            "SELECT m.event_id, m.author, m.kind, m.created_at,
                    bm25(search_fts) AS rank,
                    snippet(search_fts, 0, '**', '**', '...', 32) AS snip
             FROM search_fts f
             JOIN search_meta m ON m.fts_rowid = f.rowid
             WHERE search_fts MATCH ?",
        );
        let mut conditions = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        param_values.push(Box::new(fts_query));

        // Kind filter.
        if let Some(ref kinds) = query.kinds {
            if !kinds.is_empty() {
                let start = param_values.len() + 1;
                let placeholders: Vec<String> = kinds
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("?{}", start + i))
                    .collect();
                conditions.push(format!("m.kind IN ({})", placeholders.join(",")));
                for k in kinds {
                    param_values.push(Box::new(*k));
                }
            }
        }

        // Author filter.
        if let Some(ref authors) = query.authors {
            if !authors.is_empty() {
                let start = param_values.len() + 1;
                let placeholders: Vec<String> = authors
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("?{}", start + i))
                    .collect();
                conditions.push(format!("m.author IN ({})", placeholders.join(",")));
                for a in authors {
                    param_values.push(Box::new(a.clone()));
                }
            }
        }

        // Time range.
        if let Some(since) = query.since {
            let idx = param_values.len() + 1;
            conditions.push(format!("m.created_at >= ?{idx}"));
            param_values.push(Box::new(since));
        }
        if let Some(until) = query.until {
            let idx = param_values.len() + 1;
            conditions.push(format!("m.created_at <= ?{idx}"));
            param_values.push(Box::new(until));
        }

        // Federation scope.
        if let Some((cond, scope_params)) = scope.sql_condition(param_values.len()) {
            conditions.push(cond);
            param_values.extend(scope_params);
        }

        for cond in &conditions {
            sql.push_str(" AND ");
            sql.push_str(cond);
        }

        sql.push_str(" ORDER BY rank");

        let limit = if query.limit == 0 { 20 } else { query.limit };
        let limit_idx = param_values.len() + 1;
        sql.push_str(&format!(" LIMIT ?{limit_idx}"));
        param_values.push(Box::new(limit as i64));

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let results: Vec<SearchResult> = stmt
            .query_map(params_ref.as_slice(), |row| {
                let rank: f64 = row.get(4)?;
                let snippet: Option<String> = row.get(5)?;
                Ok(SearchResult {
                    event_id: row.get(0)?,
                    author: row.get(1)?,
                    kind: row.get(2)?,
                    created_at: row.get(3)?,
                    relevance: bm25_to_score(rank),
                    snippet,
                    suggestions: Vec::new(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        let total = results.len();
        Ok(SearchResponse {
            results,
            total_matches: total,
            suggestions: Vec::new(),
        })
    }

    /// Execute a compound query scoped to federated communities.
    ///
    /// Like `compound_search()`, but filters results to only include
    /// events tagged with a community in the given `FederationScope`.
    /// An unrestricted scope behaves identically to `compound_search()`.
    pub fn scoped_compound_search(
        &self,
        query: &CompoundQuery,
        scope: &FederationScope,
    ) -> Result<CompoundResponse, MagicalError> {
        if scope.is_unrestricted() {
            return self.compound_search(query);
        }

        let conn = self.conn.lock().map_err(|e| MagicalError::Index(format!("lock poisoned: {e}")))?;
        let results = Self::execute_scoped_compound(&conn, query, scope)?;
        let facets = Self::execute_scoped_facets(&conn, query, scope)?;
        Ok(CompoundResponse {
            total_matches: results.0,
            results: results.1,
            facets,
        })
    }

    /// Core scoped compound query execution.
    /// Mirrors `execute_compound` but injects the federation scope condition.
    fn execute_scoped_compound(
        conn: &Connection,
        query: &CompoundQuery,
        scope: &FederationScope,
    ) -> Result<(usize, Vec<CompoundResult>), MagicalError> {
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        let has_text = query.text.as_ref().is_some_and(|t| {
            let sanitized = sanitize_fts_query(t);
            !sanitized.is_empty()
        });

        let fts_query = query
            .text
            .as_ref()
            .map(|t| sanitize_fts_query(t))
            .unwrap_or_default();

        let sort_tag_key = query.sort.first().and_then(|s| match &s.field {
            SortField::TagValue(key) => Some(key.clone()),
            _ => None,
        });

        // SELECT clause
        let mut select = String::from("SELECT m.event_id, m.author, m.kind, m.created_at");
        if has_text {
            select.push_str(", bm25(search_fts) AS rank");
            select.push_str(", snippet(search_fts, 0, '**', '**', '...', 32) AS snip");
        } else {
            select.push_str(", NULL AS rank, NULL AS snip");
        }
        if sort_tag_key.is_some() {
            select.push_str(", sort_tag.tag_value AS sort_val");
        } else {
            select.push_str(", NULL AS sort_val");
        }

        // FROM clause
        let mut from = String::from(" FROM search_meta m");
        if has_text {
            from.push_str(" JOIN search_fts f ON m.fts_rowid = f.rowid");
        }
        if let Some(ref tag_key) = sort_tag_key {
            let idx = param_values.len() + 1;
            from.push_str(&format!(
                " LEFT JOIN search_tags sort_tag ON sort_tag.event_id = m.event_id AND sort_tag.tag_key = ?{idx}"
            ));
            param_values.push(Box::new(tag_key.clone()));
        }

        // WHERE clause
        let mut conditions = Vec::new();
        if has_text {
            let idx = param_values.len() + 1;
            conditions.push(format!("search_fts MATCH ?{idx}"));
            param_values.push(Box::new(fts_query));
        }

        if let Some(ref kinds) = query.kinds {
            if !kinds.is_empty() {
                let placeholders = Self::add_params(&mut param_values, kinds);
                conditions.push(format!("m.kind IN ({})", placeholders));
            }
        }
        if let Some(ref authors) = query.authors {
            if !authors.is_empty() {
                let placeholders = Self::add_string_params(&mut param_values, authors);
                conditions.push(format!("m.author IN ({})", placeholders));
            }
        }
        if let Some(since) = query.since {
            let idx = param_values.len() + 1;
            conditions.push(format!("m.created_at >= ?{idx}"));
            param_values.push(Box::new(since));
        }
        if let Some(until) = query.until {
            let idx = param_values.len() + 1;
            conditions.push(format!("m.created_at <= ?{idx}"));
            param_values.push(Box::new(until));
        }
        for tf in &query.tag_filters {
            if tf.values.is_empty() {
                continue;
            }
            let value_placeholders = Self::add_string_params(&mut param_values, &tf.values);
            let key_idx = param_values.len() + 1;
            param_values.push(Box::new(tf.key.clone()));
            conditions.push(format!(
                "EXISTS (SELECT 1 FROM search_tags st WHERE st.event_id = m.event_id AND st.tag_key = ?{key_idx} AND st.tag_value IN ({value_placeholders}))"
            ));
        }

        // Federation scope condition.
        if let Some((cond, scope_params)) = scope.sql_condition(param_values.len()) {
            conditions.push(cond);
            param_values.extend(scope_params);
        }

        let mut where_clause = String::new();
        if !conditions.is_empty() {
            where_clause = format!(" WHERE {}", conditions.join(" AND "));
        }

        // Count total matches.
        let count_sql = format!("SELECT COUNT(DISTINCT m.event_id){from}{where_clause}");
        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let total: i64 = conn.query_row(&count_sql, params_ref.as_slice(), |row| row.get(0))?;

        // ORDER BY clause.
        let order_by = Self::build_order_by(query, has_text);

        // LIMIT / OFFSET.
        let limit = if query.limit == 0 { 20 } else { query.limit };
        let limit_idx = param_values.len() + 1;
        let offset_idx = param_values.len() + 2;
        param_values.push(Box::new(limit as i64));
        param_values.push(Box::new(query.offset as i64));

        let full_sql = format!(
            "{select}{from}{where_clause}{order_by} LIMIT ?{limit_idx} OFFSET ?{offset_idx}"
        );

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&full_sql)?;
        let results: Vec<CompoundResult> = stmt
            .query_map(params_ref.as_slice(), |row| {
                let rank: Option<f64> = row.get(4)?;
                let snippet: Option<String> = row.get(5)?;
                let sort_val: Option<String> = row.get(6)?;
                Ok(CompoundResult {
                    event_id: row.get(0)?,
                    author: row.get(1)?,
                    kind: row.get(2)?,
                    created_at: row.get(3)?,
                    relevance: rank.map(bm25_to_score),
                    snippet,
                    sort_tag_value: sort_val,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok((total as usize, results))
    }

    /// Execute scoped facet requests.
    fn execute_scoped_facets(
        conn: &Connection,
        query: &CompoundQuery,
        scope: &FederationScope,
    ) -> Result<Vec<FacetResult>, MagicalError> {
        if query.facets.is_empty() {
            return Ok(Vec::new());
        }

        let (mut base_conditions, mut base_params) = Self::build_base_conditions(query)?;

        // Inject federation scope into base conditions.
        if let Some((cond, scope_params)) = scope.sql_condition(base_params.len()) {
            base_conditions.push(cond);
            base_params.extend(scope_params);
        }

        let mut results = Vec::new();
        for facet in &query.facets {
            let result =
                Self::execute_single_facet(conn, facet, &base_conditions, &base_params)?;
            results.push(result);
        }
        Ok(results)
    }

    /// Execute an aggregation query scoped to federated communities.
    ///
    /// Like `aggregate()`, but filters the aggregated event set to only
    /// include events tagged with a community in the given `FederationScope`.
    /// An unrestricted scope behaves identically to `aggregate()`.
    pub fn scoped_aggregate(
        &self,
        query: &AggregateQuery,
        scope: &FederationScope,
    ) -> Result<AggregateResponse, MagicalError> {
        if scope.is_unrestricted() {
            return self.aggregate(query);
        }

        let conn = self.conn.lock().map_err(|e| MagicalError::Index(format!("lock poisoned: {e}")))?;
        Self::execute_scoped_aggregate(&conn, query, scope)
    }

    /// Scoped aggregation — mirrors `execute_aggregate` with federation
    /// scope injected into the WHERE clause.
    fn execute_scoped_aggregate(
        conn: &Connection,
        query: &AggregateQuery,
        scope: &FederationScope,
    ) -> Result<AggregateResponse, MagicalError> {
        let function = query
            .function
            .as_ref()
            .ok_or_else(|| MagicalError::Query("aggregate function required".into()))?;

        let mut conditions = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref kinds) = query.kinds {
            if !kinds.is_empty() {
                let placeholders = Self::add_params(&mut param_values, kinds);
                conditions.push(format!("m.kind IN ({})", placeholders));
            }
        }
        if let Some(ref authors) = query.authors {
            if !authors.is_empty() {
                let placeholders = Self::add_string_params(&mut param_values, authors);
                conditions.push(format!("m.author IN ({})", placeholders));
            }
        }
        if let Some(since) = query.since {
            let idx = param_values.len() + 1;
            conditions.push(format!("m.created_at >= ?{idx}"));
            param_values.push(Box::new(since));
        }
        if let Some(until) = query.until {
            let idx = param_values.len() + 1;
            conditions.push(format!("m.created_at <= ?{idx}"));
            param_values.push(Box::new(until));
        }

        // Federation scope condition.
        if let Some((cond, scope_params)) = scope.sql_condition(param_values.len()) {
            conditions.push(cond);
            param_values.extend(scope_params);
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", conditions.join(" AND "))
        };

        match (&query.group_by, function) {
            (None, AggregateFunction::Count) => {
                let sql = format!("SELECT COUNT(*) FROM search_meta m{where_clause}");
                let params_ref: Vec<&dyn rusqlite::types::ToSql> =
                    param_values.iter().map(|p| p.as_ref()).collect();
                let count: i64 = conn.query_row(&sql, params_ref.as_slice(), |row| row.get(0))?;
                Ok(AggregateResponse {
                    value: Some(count as f64),
                    groups: Vec::new(),
                })
            }

            (None, func) => {
                let (agg_fn, tag_key) = match func {
                    AggregateFunction::Sum(k) => ("SUM", k.as_str()),
                    AggregateFunction::Min(k) => ("MIN", k.as_str()),
                    AggregateFunction::Max(k) => ("MAX", k.as_str()),
                    AggregateFunction::Avg(k) => ("AVG", k.as_str()),
                    AggregateFunction::Count => unreachable!(),
                };
                let tag_key_idx = param_values.len() + 1;
                param_values.push(Box::new(tag_key.to_string()));

                let sql = format!(
                    "SELECT {agg_fn}(CAST(st.tag_value AS REAL)) FROM search_tags st JOIN search_meta m ON st.event_id = m.event_id{where_clause}{and}st.tag_key = ?{tag_key_idx}",
                    and = if conditions.is_empty() { " WHERE " } else { " AND " },
                );
                let params_ref: Vec<&dyn rusqlite::types::ToSql> =
                    param_values.iter().map(|p| p.as_ref()).collect();
                let value: Option<f64> =
                    conn.query_row(&sql, params_ref.as_slice(), |row| row.get(0))?;
                Ok(AggregateResponse {
                    value,
                    groups: Vec::new(),
                })
            }

            (Some(group_by), AggregateFunction::Count) => {
                let limit = if query.limit == 0 { 100 } else { query.limit };
                let limit_idx = param_values.len() + 1;
                param_values.push(Box::new(limit as i64));

                let sql = match group_by {
                    GroupBy::Kind => format!(
                        "SELECT CAST(m.kind AS TEXT), COUNT(*) FROM search_meta m{where_clause} GROUP BY m.kind ORDER BY COUNT(*) DESC LIMIT ?{limit_idx}"
                    ),
                    GroupBy::Author => format!(
                        "SELECT m.author, COUNT(*) FROM search_meta m{where_clause} GROUP BY m.author ORDER BY COUNT(*) DESC LIMIT ?{limit_idx}"
                    ),
                    GroupBy::Tag(tag_key) => {
                        let tag_key_idx = param_values.len() + 1;
                        param_values.push(Box::new(tag_key.clone()));
                        format!(
                            "SELECT st.tag_value, COUNT(DISTINCT st.event_id) FROM search_tags st JOIN search_meta m ON st.event_id = m.event_id{}{and}st.tag_key = ?{tag_key_idx} GROUP BY st.tag_value ORDER BY COUNT(DISTINCT st.event_id) DESC LIMIT ?{limit_idx}",
                            where_clause,
                            and = if conditions.is_empty() { " WHERE " } else { " AND " },
                        )
                    }
                };

                let params_ref: Vec<&dyn rusqlite::types::ToSql> =
                    param_values.iter().map(|p| p.as_ref()).collect();
                let mut stmt = conn.prepare(&sql)?;
                let groups: Vec<AggregateGroup> = stmt
                    .query_map(params_ref.as_slice(), |row| {
                        let count: i64 = row.get(1)?;
                        Ok(AggregateGroup {
                            key: row.get(0)?,
                            value: count as f64,
                        })
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(AggregateResponse {
                    value: None,
                    groups,
                })
            }

            (Some(group_by), func) => {
                let (agg_fn, tag_key) = match func {
                    AggregateFunction::Sum(k) => ("SUM", k.as_str()),
                    AggregateFunction::Min(k) => ("MIN", k.as_str()),
                    AggregateFunction::Max(k) => ("MAX", k.as_str()),
                    AggregateFunction::Avg(k) => ("AVG", k.as_str()),
                    AggregateFunction::Count => unreachable!(),
                };
                let limit = if query.limit == 0 { 100 } else { query.limit };
                let limit_idx = param_values.len() + 1;
                param_values.push(Box::new(limit as i64));
                let tag_key_idx = param_values.len() + 1;
                param_values.push(Box::new(tag_key.to_string()));

                let (group_col, group_expr) = match group_by {
                    GroupBy::Kind => ("CAST(m.kind AS TEXT)", "m.kind"),
                    GroupBy::Author => ("m.author", "m.author"),
                    GroupBy::Tag(group_tag) => {
                        let group_tag_idx = param_values.len() + 1;
                        param_values.push(Box::new(group_tag.clone()));

                        let sql = format!(
                            "SELECT gt.tag_value, {agg_fn}(CAST(at.tag_value AS REAL)) \
                             FROM search_tags gt \
                             JOIN search_meta m ON gt.event_id = m.event_id \
                             JOIN search_tags at ON at.event_id = m.event_id AND at.tag_key = ?{tag_key_idx} \
                             {}{and}gt.tag_key = ?{group_tag_idx} \
                             GROUP BY gt.tag_value \
                             ORDER BY {agg_fn}(CAST(at.tag_value AS REAL)) DESC \
                             LIMIT ?{limit_idx}",
                            where_clause,
                            and = if conditions.is_empty() { " WHERE " } else { " AND " },
                        );

                        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
                            param_values.iter().map(|p| p.as_ref()).collect();
                        let mut stmt = conn.prepare(&sql)?;
                        let groups: Vec<AggregateGroup> = stmt
                            .query_map(params_ref.as_slice(), |row| {
                                Ok(AggregateGroup {
                                    key: row.get(0)?,
                                    value: row.get::<_, f64>(1).unwrap_or(0.0),
                                })
                            })?
                            .filter_map(|r| r.ok())
                            .collect();

                        return Ok(AggregateResponse {
                            value: None,
                            groups,
                        });
                    }
                };

                let sql = format!(
                    "SELECT {group_col}, {agg_fn}(CAST(st.tag_value AS REAL)) \
                     FROM search_tags st \
                     JOIN search_meta m ON st.event_id = m.event_id \
                     {where_clause}{and}st.tag_key = ?{tag_key_idx} \
                     GROUP BY {group_expr} \
                     ORDER BY {agg_fn}(CAST(st.tag_value AS REAL)) DESC \
                     LIMIT ?{limit_idx}",
                    and = if conditions.is_empty() { " WHERE " } else { " AND " },
                );

                let params_ref: Vec<&dyn rusqlite::types::ToSql> =
                    param_values.iter().map(|p| p.as_ref()).collect();
                let mut stmt = conn.prepare(&sql)?;
                let groups: Vec<AggregateGroup> = stmt
                    .query_map(params_ref.as_slice(), |row| {
                        Ok(AggregateGroup {
                            key: row.get(0)?,
                            value: row.get::<_, f64>(1).unwrap_or(0.0),
                        })
                    })?
                    .filter_map(|r| r.ok())
                    .collect();

                Ok(AggregateResponse {
                    value: None,
                    groups,
                })
            }
        }
    }
}

impl SearchIndex for KeywordIndex {
    fn index_event(&self, event: &OmniEvent) -> Result<(), MagicalError> {
        let content = Self::extract_content(event);
        let tags = Self::extract_tags(event);

        // Skip events with no searchable text.
        if content.is_empty() && tags.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().map_err(|e| MagicalError::Index(format!("lock poisoned: {e}")))?;

        // Check for duplicate.
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM search_meta WHERE event_id = ?)",
                params![event.id],
                |row| row.get(0),
            )?;
        if exists {
            return Ok(());
        }

        // Insert into FTS5.
        conn.execute(
            "INSERT INTO search_fts(content, tags) VALUES (?, ?)",
            params![content, tags],
        )?;
        let rowid = conn.last_insert_rowid();

        // Insert metadata.
        conn.execute(
            "INSERT INTO search_meta(event_id, author, kind, created_at, fts_rowid)
             VALUES (?, ?, ?, ?, ?)",
            params![event.id, event.author, event.kind, event.created_at, rowid],
        )?;

        // Insert structured tags.
        let tag_pairs = Self::extract_tag_pairs(event);
        for (key, value) in &tag_pairs {
            conn.execute(
                "INSERT INTO search_tags(event_id, tag_key, tag_value) VALUES (?, ?, ?)",
                params![event.id, key, value],
            )?;
        }

        Ok(())
    }

    fn remove_event(&self, event_id: &str) -> Result<bool, MagicalError> {
        let conn = self.conn.lock().map_err(|e| MagicalError::Index(format!("lock poisoned: {e}")))?;

        let rowid: Option<i64> = conn
            .query_row(
                "SELECT fts_rowid FROM search_meta WHERE event_id = ?",
                params![event_id],
                |row| row.get(0),
            )
            .ok();

        match rowid {
            Some(rid) => {
                conn.execute(
                    "DELETE FROM search_fts WHERE rowid = ?",
                    params![rid],
                )?;
                conn.execute(
                    "DELETE FROM search_tags WHERE event_id = ?",
                    params![event_id],
                )?;
                conn.execute(
                    "DELETE FROM search_meta WHERE event_id = ?",
                    params![event_id],
                )?;
                Ok(true)
            }
            None => Ok(false),
        }
    }

    fn search(&self, query: &SearchQuery) -> Result<SearchResponse, MagicalError> {
        if query.text.is_empty() {
            return Ok(SearchResponse::default());
        }

        let conn = self.conn.lock().map_err(|e| MagicalError::Index(format!("lock poisoned: {e}")))?;

        // Build FTS5 query. Escape user input for safety.
        let fts_query = sanitize_fts_query(&query.text);
        if fts_query.is_empty() {
            return Ok(SearchResponse::default());
        }

        // Query FTS5 with BM25 ranking + metadata join + optional filters.
        let mut sql = String::from(
            "SELECT m.event_id, m.author, m.kind, m.created_at,
                    bm25(search_fts) AS rank,
                    snippet(search_fts, 0, '**', '**', '...', 32) AS snip
             FROM search_fts f
             JOIN search_meta m ON m.fts_rowid = f.rowid
             WHERE search_fts MATCH ?",
        );
        let mut conditions = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        param_values.push(Box::new(fts_query.clone()));

        // Kind filter.
        if let Some(ref kinds) = query.kinds {
            if !kinds.is_empty() {
                let placeholders: Vec<String> = kinds
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("?{}", i + 2 + conditions.len()))
                    .collect();
                conditions.push(format!("m.kind IN ({})", placeholders.join(",")));
                for k in kinds {
                    param_values.push(Box::new(*k));
                }
            }
        }

        // Author filter.
        if let Some(ref authors) = query.authors {
            if !authors.is_empty() {
                let start = param_values.len() + 1;
                let placeholders: Vec<String> = authors
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format!("?{}", start + i))
                    .collect();
                conditions.push(format!("m.author IN ({})", placeholders.join(",")));
                for a in authors {
                    param_values.push(Box::new(a.clone()));
                }
            }
        }

        // Time range.
        if let Some(since) = query.since {
            let idx = param_values.len() + 1;
            conditions.push(format!("m.created_at >= ?{idx}"));
            param_values.push(Box::new(since));
        }
        if let Some(until) = query.until {
            let idx = param_values.len() + 1;
            conditions.push(format!("m.created_at <= ?{idx}"));
            param_values.push(Box::new(until));
        }

        for cond in &conditions {
            sql.push_str(" AND ");
            sql.push_str(cond);
        }

        sql.push_str(" ORDER BY rank");

        let limit = if query.limit == 0 { 20 } else { query.limit };
        let limit_idx = param_values.len() + 1;
        sql.push_str(&format!(" LIMIT ?{limit_idx}"));
        param_values.push(Box::new(limit as i64));

        let params_ref: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let results: Vec<SearchResult> = stmt
            .query_map(params_ref.as_slice(), |row| {
                let rank: f64 = row.get(4)?;
                let snippet: Option<String> = row.get(5)?;
                Ok(SearchResult {
                    event_id: row.get(0)?,
                    author: row.get(1)?,
                    kind: row.get(2)?,
                    created_at: row.get(3)?,
                    // BM25 returns negative values (lower = better).
                    // Invert to 0..1 range: -0 is best (1.0), -inf is worst (0.0).
                    relevance: bm25_to_score(rank),
                    snippet,
                    suggestions: Vec::new(),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        let total = results.len();
        Ok(SearchResponse {
            results,
            total_matches: total,
            suggestions: Vec::new(),
        })
    }

    fn indexed_count(&self) -> Result<usize, MagicalError> {
        let conn = self.conn.lock().map_err(|e| MagicalError::Index(format!("lock poisoned: {e}")))?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM search_meta",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}

/// Convert BM25 score (negative, lower = better) to a 0.0-1.0 relevance score.
///
/// BM25 returns 0 for perfect match and increasingly negative for worse matches.
/// We map this to: 0.0 -> 1.0, -10.0 -> ~0.5, -inf -> 0.0.
fn bm25_to_score(bm25: f64) -> f64 {
    // Sigmoid-style mapping: score = 1 / (1 + |bm25| / 5)
    1.0 / (1.0 + bm25.abs() / 5.0)
}

/// Sanitize user input for FTS5 MATCH queries.
///
/// Strips FTS5 operators and wraps remaining terms so they're treated
/// as plain word matches. Returns empty string if no valid terms remain.
fn sanitize_fts_query(input: &str) -> String {
    // Remove FTS5 special characters that could cause syntax errors.
    let cleaned: String = input
        .chars()
        .map(|c| match c {
            '"' | '\'' | '*' | '(' | ')' | '{' | '}' | '^' | '~' => ' ',
            _ => c,
        })
        .collect();

    // Split into words, filter empty, rejoin.
    let terms: Vec<&str> = cleaned
        .split_whitespace()
        .filter(|t| !t.is_empty())
        // Strip bare operators
        .filter(|t| !matches!(*t, "AND" | "OR" | "NOT" | "NEAR"))
        .collect();

    if terms.is_empty() {
        return String::new();
    }

    // Join with implicit AND (FTS5 default).
    terms.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aggregation::{AggregateQuery, GroupBy};
    use crate::compound::{CompoundQuery, FacetRequest, SortClause, SortDirection};
    use crate::query::SearchQuery;

    fn make_event(id: &str, content: &str, kind: u32) -> OmniEvent {
        OmniEvent {
            id: id.into(),
            author: "a".repeat(64),
            created_at: 1000,
            kind,
            tags: vec![],
            content: content.into(),
            sig: "c".repeat(128),
        }
    }

    fn make_tagged_event(id: &str, content: &str, tags: Vec<Vec<String>>) -> OmniEvent {
        OmniEvent {
            id: id.into(),
            author: "a".repeat(64),
            created_at: 1000,
            kind: 1,
            tags,
            content: content.into(),
            sig: "c".repeat(128),
        }
    }

    fn make_full_event(
        id: &str,
        author: &str,
        content: &str,
        kind: u32,
        created_at: i64,
        tags: Vec<Vec<String>>,
    ) -> OmniEvent {
        OmniEvent {
            id: id.into(),
            author: author.into(),
            created_at,
            kind,
            tags,
            content: content.into(),
            sig: "c".repeat(128),
        }
    }

    // ---- Original SearchIndex tests (preserved) ----

    #[test]
    fn index_and_search_basic() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "woodworking with dovetail joints", 1))
            .unwrap();
        idx.index_event(&make_event("e2", "cooking pasta recipes", 1))
            .unwrap();
        idx.index_event(&make_event("e3", "advanced joinery techniques", 1))
            .unwrap();

        let response = idx.search(&SearchQuery::new("woodworking")).unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].event_id, "e1");
        assert!(response.results[0].relevance > 0.0);
    }

    #[test]
    fn search_multiple_results() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "rust programming language", 1))
            .unwrap();
        idx.index_event(&make_event("e2", "rust compiler internals", 1))
            .unwrap();
        idx.index_event(&make_event("e3", "python programming", 1))
            .unwrap();

        let response = idx.search(&SearchQuery::new("rust")).unwrap();
        assert_eq!(response.results.len(), 2);
        let ids: Vec<&str> = response.results.iter().map(|r| r.event_id.as_str()).collect();
        assert!(ids.contains(&"e1"));
        assert!(ids.contains(&"e2"));
    }

    #[test]
    fn search_with_kind_filter() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "hello world", 1)).unwrap();
        idx.index_event(&make_event("e2", "hello universe", 7030)).unwrap();

        let response = idx
            .search(&SearchQuery::new("hello").with_kinds(vec![1]))
            .unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].event_id, "e1");
    }

    #[test]
    fn search_with_limit() {
        let idx = KeywordIndex::in_memory().unwrap();
        for i in 0..10 {
            idx.index_event(&make_event(
                &format!("e{i}"),
                &format!("test content number {i}"),
                1,
            ))
            .unwrap();
        }

        let response = idx
            .search(&SearchQuery::new("test").with_limit(3))
            .unwrap();
        assert_eq!(response.results.len(), 3);
    }

    #[test]
    fn search_tags() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_tagged_event(
            "e1",
            "a post about stuff",
            vec![vec!["t".into(), "woodworking".into()]],
        ))
        .unwrap();
        idx.index_event(&make_tagged_event(
            "e2",
            "another post",
            vec![vec!["t".into(), "cooking".into()]],
        ))
        .unwrap();

        let response = idx.search(&SearchQuery::new("woodworking")).unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].event_id, "e1");
    }

    #[test]
    fn search_d_tags() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_tagged_event(
            "e1",
            "",
            vec![vec!["d".into(), "sam.idea".into()]],
        ))
        .unwrap();

        let response = idx.search(&SearchQuery::new("sam")).unwrap();
        assert_eq!(response.results.len(), 1);
    }

    #[test]
    fn search_json_content() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event(
            "e1",
            r#"{"name":"Alice","about":"I love woodworking"}"#,
            0,
        ))
        .unwrap();

        let response = idx.search(&SearchQuery::new("woodworking")).unwrap();
        assert_eq!(response.results.len(), 1);
    }

    #[test]
    fn duplicate_event_ignored() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "hello world", 1)).unwrap();
        idx.index_event(&make_event("e1", "hello world", 1)).unwrap();

        assert_eq!(idx.indexed_count().unwrap(), 1);
    }

    #[test]
    fn remove_event_works() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "hello world", 1)).unwrap();
        assert_eq!(idx.indexed_count().unwrap(), 1);

        assert!(idx.remove_event("e1").unwrap());
        assert_eq!(idx.indexed_count().unwrap(), 0);

        let response = idx.search(&SearchQuery::new("hello")).unwrap();
        assert!(response.results.is_empty());
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let idx = KeywordIndex::in_memory().unwrap();
        assert!(!idx.remove_event("nonexistent").unwrap());
    }

    #[test]
    fn empty_content_not_indexed() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "", 1)).unwrap();
        assert_eq!(idx.indexed_count().unwrap(), 0);
    }

    #[test]
    fn empty_query_returns_empty() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "hello world", 1)).unwrap();

        let response = idx.search(&SearchQuery::new("")).unwrap();
        assert!(response.results.is_empty());
    }

    #[test]
    fn snippet_present() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event(
            "e1",
            "the quick brown fox jumps over the lazy dog in the woodworking shop",
            1,
        ))
        .unwrap();

        let response = idx.search(&SearchQuery::new("woodworking")).unwrap();
        assert_eq!(response.results.len(), 1);
        assert!(response.results[0].snippet.is_some());
    }

    #[test]
    fn relevance_scores_are_positive() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "rust programming language", 1))
            .unwrap();
        idx.index_event(&make_event("e2", "I tried rust once", 1))
            .unwrap();

        let response = idx.search(&SearchQuery::new("rust")).unwrap();
        assert_eq!(response.results.len(), 2);
        for result in &response.results {
            assert!(result.relevance > 0.0, "relevance should be positive");
            assert!(result.relevance <= 1.0, "relevance should be <= 1.0");
        }
    }

    #[test]
    fn time_range_filter() {
        let idx = KeywordIndex::in_memory().unwrap();
        let mut e1 = make_event("e1", "hello from the past", 1);
        e1.created_at = 1000;
        let mut e2 = make_event("e2", "hello from the future", 1);
        e2.created_at = 5000;

        idx.index_event(&e1).unwrap();
        idx.index_event(&e2).unwrap();

        let response = idx
            .search(&SearchQuery::new("hello").with_time_range(Some(3000), None))
            .unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].event_id, "e2");
    }

    #[test]
    fn sanitize_fts_strips_operators() {
        assert_eq!(sanitize_fts_query("hello world"), "hello world");
        assert_eq!(sanitize_fts_query("hello \"world\""), "hello world");
        assert_eq!(sanitize_fts_query("AND OR NOT"), "");
        assert_eq!(sanitize_fts_query("rust*"), "rust");
        assert_eq!(sanitize_fts_query(""), "");
        assert_eq!(sanitize_fts_query("  hello  "), "hello");
    }

    #[test]
    fn bm25_score_conversion() {
        assert!((bm25_to_score(0.0) - 1.0).abs() < f64::EPSILON);
        assert!(bm25_to_score(-5.0) > 0.3);
        assert!(bm25_to_score(-5.0) < 0.7);
        assert!(bm25_to_score(-50.0) < 0.2);
    }

    #[test]
    fn indexed_count_tracks_correctly() {
        let idx = KeywordIndex::in_memory().unwrap();
        assert_eq!(idx.indexed_count().unwrap(), 0);

        idx.index_event(&make_event("e1", "hello", 1)).unwrap();
        assert_eq!(idx.indexed_count().unwrap(), 1);

        idx.index_event(&make_event("e2", "world", 1)).unwrap();
        assert_eq!(idx.indexed_count().unwrap(), 2);

        idx.remove_event("e1").unwrap();
        assert_eq!(idx.indexed_count().unwrap(), 1);
    }

    #[test]
    fn porter_stemming_works() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "running and jumping", 1))
            .unwrap();

        let response = idx.search(&SearchQuery::new("run")).unwrap();
        assert_eq!(response.results.len(), 1);

        let response = idx.search(&SearchQuery::new("jump")).unwrap();
        assert_eq!(response.results.len(), 1);
    }

    // ---- Tag storage tests ----

    #[test]
    fn tags_stored_in_search_tags() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_tagged_event(
            "e1",
            "a post",
            vec![
                vec!["t".into(), "logo".into()],
                vec!["status".into(), "approved".into()],
            ],
        ))
        .unwrap();

        // Verify tags are stored by doing a compound query.
        let response = idx
            .compound_search(
                &CompoundQuery::new().with_tag_exact("t", "logo"),
            )
            .unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].event_id, "e1");
    }

    #[test]
    fn remove_event_cleans_tags() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_tagged_event(
            "e1",
            "a post",
            vec![vec!["t".into(), "logo".into()]],
        ))
        .unwrap();

        idx.remove_event("e1").unwrap();

        // Tag-based search should find nothing.
        let response = idx
            .compound_search(
                &CompoundQuery::new().with_tag_exact("t", "logo"),
            )
            .unwrap();
        assert!(response.results.is_empty());
    }

    // ---- Compound query tests ----

    #[test]
    fn compound_no_filters_returns_all() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "hello", 1)).unwrap();
        idx.index_event(&make_event("e2", "world", 1)).unwrap();

        let response = idx.compound_search(&CompoundQuery::new()).unwrap();
        assert_eq!(response.total_matches, 2);
        assert_eq!(response.results.len(), 2);
    }

    #[test]
    fn compound_text_search() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "rust programming", 1)).unwrap();
        idx.index_event(&make_event("e2", "python scripting", 1)).unwrap();

        let response = idx
            .compound_search(&CompoundQuery::text("rust"))
            .unwrap();
        assert_eq!(response.total_matches, 1);
        assert_eq!(response.results[0].event_id, "e1");
        assert!(response.results[0].relevance.is_some());
    }

    #[test]
    fn compound_tag_filter() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_full_event(
            "e1", &author, "logo design", 1, 1000,
            vec![vec!["t".into(), "logo".into()], vec!["status".into(), "approved".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e2", &author, "icon design", 1, 2000,
            vec![vec!["t".into(), "icon".into()], vec!["status".into(), "draft".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e3", &author, "banner design", 1, 3000,
            vec![vec!["t".into(), "banner".into()], vec!["status".into(), "approved".into()]],
        )).unwrap();

        // Filter: status=approved
        let response = idx
            .compound_search(
                &CompoundQuery::new().with_tag_exact("status", "approved"),
            )
            .unwrap();
        assert_eq!(response.total_matches, 2);
        let ids: Vec<&str> = response.results.iter().map(|r| r.event_id.as_str()).collect();
        assert!(ids.contains(&"e1"));
        assert!(ids.contains(&"e3"));
    }

    #[test]
    fn compound_multiple_tag_filters_and() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_full_event(
            "e1", &author, "approved logo", 1, 1000,
            vec![vec!["t".into(), "logo".into()], vec!["status".into(), "approved".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e2", &author, "draft logo", 1, 2000,
            vec![vec!["t".into(), "logo".into()], vec!["status".into(), "draft".into()]],
        )).unwrap();

        // Filter: t=logo AND status=approved
        let response = idx
            .compound_search(
                &CompoundQuery::new()
                    .with_tag_exact("t", "logo")
                    .with_tag_exact("status", "approved"),
            )
            .unwrap();
        assert_eq!(response.total_matches, 1);
        assert_eq!(response.results[0].event_id, "e1");
    }

    #[test]
    fn compound_tag_filter_or_values() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_full_event(
            "e1", &author, "a logo", 1, 1000,
            vec![vec!["t".into(), "logo".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e2", &author, "an icon", 1, 2000,
            vec![vec!["t".into(), "icon".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e3", &author, "a banner", 1, 3000,
            vec![vec!["t".into(), "banner".into()]],
        )).unwrap();

        // Filter: t IN (logo, icon)
        let response = idx
            .compound_search(
                &CompoundQuery::new()
                    .with_tag("t", vec!["logo".into(), "icon".into()]),
            )
            .unwrap();
        assert_eq!(response.total_matches, 2);
        let ids: Vec<&str> = response.results.iter().map(|r| r.event_id.as_str()).collect();
        assert!(ids.contains(&"e1"));
        assert!(ids.contains(&"e2"));
    }

    #[test]
    fn compound_text_plus_tag() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_full_event(
            "e1", &author, "rust logo design", 1, 1000,
            vec![vec!["t".into(), "logo".into()], vec!["status".into(), "approved".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e2", &author, "rust compiler internals", 1, 2000,
            vec![vec!["t".into(), "programming".into()], vec!["status".into(), "approved".into()]],
        )).unwrap();

        // Text "rust" + tag status=approved + tag t=logo
        let response = idx
            .compound_search(
                &CompoundQuery::text("rust")
                    .with_tag_exact("status", "approved")
                    .with_tag_exact("t", "logo"),
            )
            .unwrap();
        assert_eq!(response.total_matches, 1);
        assert_eq!(response.results[0].event_id, "e1");
    }

    #[test]
    fn compound_sort_by_created_at() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_full_event("e1", &author, "first post", 1, 1000, vec![])).unwrap();
        idx.index_event(&make_full_event("e2", &author, "second post", 1, 2000, vec![])).unwrap();
        idx.index_event(&make_full_event("e3", &author, "third post", 1, 3000, vec![])).unwrap();

        // Newest first.
        let response = idx
            .compound_search(
                &CompoundQuery::new().sorted_by(SortClause::newest_first()),
            )
            .unwrap();
        assert_eq!(response.results[0].event_id, "e3");
        assert_eq!(response.results[2].event_id, "e1");

        // Oldest first.
        let response = idx
            .compound_search(
                &CompoundQuery::new().sorted_by(SortClause::oldest_first()),
            )
            .unwrap();
        assert_eq!(response.results[0].event_id, "e1");
        assert_eq!(response.results[2].event_id, "e3");
    }

    #[test]
    fn compound_sort_by_tag_value() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_full_event(
            "e1", &author, "popular asset", 1, 1000,
            vec![vec!["downloads".into(), "500".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e2", &author, "viral asset", 1, 2000,
            vec![vec!["downloads".into(), "10000".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e3", &author, "niche asset", 1, 3000,
            vec![vec!["downloads".into(), "50".into()]],
        )).unwrap();

        // Sort by downloads descending (lexicographic for now — numeric sort
        // requires CAST which we apply in SQL).
        let response = idx
            .compound_search(
                &CompoundQuery::new()
                    .sorted_by(SortClause::by_tag("downloads", SortDirection::Desc)),
            )
            .unwrap();
        assert_eq!(response.results.len(), 3);
        // Tag value is string-sorted, so "500" > "50" > "10000" in string order.
        // This is expected — sorting by tag is lexicographic.
        // For numeric sorting, users should zero-pad or use aggregation.
        assert!(response.results[0].sort_tag_value.is_some());
    }

    #[test]
    fn compound_pagination() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        for i in 0..10 {
            idx.index_event(&make_full_event(
                &format!("e{i}"), &author, &format!("post number {i}"), 1, 1000 + i, vec![],
            )).unwrap();
        }

        // Page 1: 3 results.
        let page1 = idx
            .compound_search(
                &CompoundQuery::new()
                    .sorted_by(SortClause::oldest_first())
                    .with_limit(3)
                    .with_offset(0),
            )
            .unwrap();
        assert_eq!(page1.total_matches, 10);
        assert_eq!(page1.results.len(), 3);
        assert_eq!(page1.results[0].event_id, "e0");

        // Page 2.
        let page2 = idx
            .compound_search(
                &CompoundQuery::new()
                    .sorted_by(SortClause::oldest_first())
                    .with_limit(3)
                    .with_offset(3),
            )
            .unwrap();
        assert_eq!(page2.total_matches, 10);
        assert_eq!(page2.results.len(), 3);
        assert_eq!(page2.results[0].event_id, "e3");
    }

    #[test]
    fn compound_kind_filter() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "post one", 1)).unwrap();
        idx.index_event(&make_event("e2", "beacon one", 7030)).unwrap();
        idx.index_event(&make_event("e3", "post two", 1)).unwrap();

        let response = idx
            .compound_search(&CompoundQuery::new().with_kinds(vec![7030]))
            .unwrap();
        assert_eq!(response.total_matches, 1);
        assert_eq!(response.results[0].event_id, "e2");
    }

    #[test]
    fn compound_time_range() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_full_event("e1", &author, "old post", 1, 1000, vec![])).unwrap();
        idx.index_event(&make_full_event("e2", &author, "new post", 1, 5000, vec![])).unwrap();

        let response = idx
            .compound_search(
                &CompoundQuery::new().with_time_range(Some(3000), None),
            )
            .unwrap();
        assert_eq!(response.total_matches, 1);
        assert_eq!(response.results[0].event_id, "e2");
    }

    // ---- Faceted search tests ----

    #[test]
    fn facet_by_kind() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "post one", 1)).unwrap();
        idx.index_event(&make_event("e2", "post two", 1)).unwrap();
        idx.index_event(&make_event("e3", "beacon", 7030)).unwrap();

        let response = idx
            .compound_search(
                &CompoundQuery::new().with_facet(FacetRequest::ByKind),
            )
            .unwrap();
        assert_eq!(response.facets.len(), 1);
        assert_eq!(response.facets[0].dimension, "kind");
        assert_eq!(response.facets[0].buckets.len(), 2);
        // Kind 1 has 2 events, kind 7030 has 1.
        let kind_1 = response.facets[0].buckets.iter().find(|b| b.value == "1").unwrap();
        assert_eq!(kind_1.count, 2);
    }

    #[test]
    fn facet_by_tag() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_full_event(
            "e1", &author, "logo one", 1, 1000,
            vec![vec!["t".into(), "logo".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e2", &author, "logo two", 1, 2000,
            vec![vec!["t".into(), "logo".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e3", &author, "an icon", 1, 3000,
            vec![vec!["t".into(), "icon".into()]],
        )).unwrap();

        let response = idx
            .compound_search(
                &CompoundQuery::new().with_facet(FacetRequest::ByTag("t".into())),
            )
            .unwrap();
        assert_eq!(response.facets.len(), 1);
        assert_eq!(response.facets[0].dimension, "tag:t");
        let logo_bucket = response.facets[0].buckets.iter().find(|b| b.value == "logo").unwrap();
        assert_eq!(logo_bucket.count, 2);
        let icon_bucket = response.facets[0].buckets.iter().find(|b| b.value == "icon").unwrap();
        assert_eq!(icon_bucket.count, 1);
    }

    #[test]
    fn facet_with_filters() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_full_event(
            "e1", &author, "approved logo", 1, 1000,
            vec![vec!["t".into(), "logo".into()], vec!["status".into(), "approved".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e2", &author, "draft logo", 1, 2000,
            vec![vec!["t".into(), "logo".into()], vec!["status".into(), "draft".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e3", &author, "approved icon", 1, 3000,
            vec![vec!["t".into(), "icon".into()], vec!["status".into(), "approved".into()]],
        )).unwrap();

        // Facet by tag "t" with filter status=approved.
        let response = idx
            .compound_search(
                &CompoundQuery::new()
                    .with_tag_exact("status", "approved")
                    .with_facet(FacetRequest::ByTag("t".into())),
            )
            .unwrap();
        assert_eq!(response.total_matches, 2);
        assert_eq!(response.facets[0].buckets.len(), 2);
        // Both "logo" and "icon" have 1 approved event each.
        for bucket in &response.facets[0].buckets {
            assert_eq!(bucket.count, 1);
        }
    }

    // ---- Aggregation tests ----

    #[test]
    fn aggregate_count_all() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "hello", 1)).unwrap();
        idx.index_event(&make_event("e2", "world", 1)).unwrap();
        idx.index_event(&make_event("e3", "test", 7030)).unwrap();

        let response = idx.aggregate(&AggregateQuery::count()).unwrap();
        assert_eq!(response.value, Some(3.0));
    }

    #[test]
    fn aggregate_count_with_kind_filter() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "hello", 1)).unwrap();
        idx.index_event(&make_event("e2", "world", 1)).unwrap();
        idx.index_event(&make_event("e3", "test", 7030)).unwrap();

        let response = idx
            .aggregate(&AggregateQuery::count().with_kinds(vec![1]))
            .unwrap();
        assert_eq!(response.value, Some(2.0));
    }

    #[test]
    fn aggregate_count_by_kind() {
        let idx = KeywordIndex::in_memory().unwrap();
        idx.index_event(&make_event("e1", "hello", 1)).unwrap();
        idx.index_event(&make_event("e2", "world", 1)).unwrap();
        idx.index_event(&make_event("e3", "test", 7030)).unwrap();

        let response = idx
            .aggregate(&AggregateQuery::count_by(GroupBy::Kind))
            .unwrap();
        assert!(response.value.is_none());
        assert_eq!(response.groups.len(), 2);
        let kind_1 = response.groups.iter().find(|g| g.key == "1").unwrap();
        assert_eq!(kind_1.value, 2.0);
        let kind_7030 = response.groups.iter().find(|g| g.key == "7030").unwrap();
        assert_eq!(kind_7030.value, 1.0);
    }

    #[test]
    fn aggregate_count_by_tag() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_full_event(
            "e1", &author, "a logo", 1, 1000,
            vec![vec!["t".into(), "logo".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e2", &author, "another logo", 1, 2000,
            vec![vec!["t".into(), "logo".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e3", &author, "an icon", 1, 3000,
            vec![vec!["t".into(), "icon".into()]],
        )).unwrap();

        let response = idx
            .aggregate(&AggregateQuery::count_by(GroupBy::Tag("t".into())))
            .unwrap();
        assert_eq!(response.groups.len(), 2);
        let logo = response.groups.iter().find(|g| g.key == "logo").unwrap();
        assert_eq!(logo.value, 2.0);
    }

    #[test]
    fn aggregate_sum() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_full_event(
            "e1", &author, "asset one", 1, 1000,
            vec![vec!["downloads".into(), "100".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e2", &author, "asset two", 1, 2000,
            vec![vec!["downloads".into(), "250".into()]],
        )).unwrap();

        let response = idx.aggregate(&AggregateQuery::sum("downloads")).unwrap();
        assert_eq!(response.value, Some(350.0));
    }

    #[test]
    fn aggregate_min_max() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_full_event(
            "e1", &author, "cheap item", 1, 1000,
            vec![vec!["price".into(), "5".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e2", &author, "expensive item", 1, 2000,
            vec![vec!["price".into(), "100".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e3", &author, "mid item", 1, 3000,
            vec![vec!["price".into(), "50".into()]],
        )).unwrap();

        let min_response = idx.aggregate(&AggregateQuery::min("price")).unwrap();
        assert_eq!(min_response.value, Some(5.0));

        let max_response = idx.aggregate(&AggregateQuery::max("price")).unwrap();
        assert_eq!(max_response.value, Some(100.0));
    }

    #[test]
    fn aggregate_avg() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_full_event(
            "e1", &author, "item one", 1, 1000,
            vec![vec!["rating".into(), "4".into()]],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e2", &author, "item two", 1, 2000,
            vec![vec!["rating".into(), "6".into()]],
        )).unwrap();

        let response = idx.aggregate(&AggregateQuery::avg("rating")).unwrap();
        assert_eq!(response.value, Some(5.0));
    }

    #[test]
    fn aggregate_count_by_author() {
        let idx = KeywordIndex::in_memory().unwrap();
        let alice = "a".repeat(64);
        let bob = "b".repeat(64);
        idx.index_event(&make_full_event("e1", &alice, "post one", 1, 1000, vec![])).unwrap();
        idx.index_event(&make_full_event("e2", &alice, "post two", 1, 2000, vec![])).unwrap();
        idx.index_event(&make_full_event("e3", &bob, "post three", 1, 3000, vec![])).unwrap();

        let response = idx
            .aggregate(&AggregateQuery::count_by(GroupBy::Author))
            .unwrap();
        assert_eq!(response.groups.len(), 2);
        let alice_group = response.groups.iter().find(|g| g.key == alice).unwrap();
        assert_eq!(alice_group.value, 2.0);
    }

    #[test]
    fn aggregate_with_time_range() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_full_event("e1", &author, "old post", 1, 1000, vec![])).unwrap();
        idx.index_event(&make_full_event("e2", &author, "new post", 1, 5000, vec![])).unwrap();

        let response = idx
            .aggregate(
                &AggregateQuery::count().with_time_range(Some(3000), None),
            )
            .unwrap();
        assert_eq!(response.value, Some(1.0));
    }

    #[test]
    fn aggregate_no_function_errors() {
        let idx = KeywordIndex::in_memory().unwrap();
        let result = idx.aggregate(&AggregateQuery::default());
        assert!(result.is_err());
    }

    // ---- The big compound query: "assets tagged 'logo' approved in last 30 days, sorted by most downloaded" ----

    #[test]
    fn compound_query_the_todo_example() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        let now = 100_000;
        let thirty_days_ago = now - (30 * 86400);

        // Old approved logo (outside 30 days).
        idx.index_event(&make_full_event(
            "e1", &author, "old logo", 1, thirty_days_ago - 1000,
            vec![
                vec!["t".into(), "logo".into()],
                vec!["status".into(), "approved".into()],
                vec!["downloads".into(), "999".into()],
            ],
        )).unwrap();

        // Recent approved logo with many downloads.
        idx.index_event(&make_full_event(
            "e2", &author, "popular logo", 1, now - 1000,
            vec![
                vec!["t".into(), "logo".into()],
                vec!["status".into(), "approved".into()],
                vec!["downloads".into(), "500".into()],
            ],
        )).unwrap();

        // Recent approved logo with few downloads.
        idx.index_event(&make_full_event(
            "e3", &author, "niche logo", 1, now - 2000,
            vec![
                vec!["t".into(), "logo".into()],
                vec!["status".into(), "approved".into()],
                vec!["downloads".into(), "10".into()],
            ],
        )).unwrap();

        // Recent draft logo (not approved).
        idx.index_event(&make_full_event(
            "e4", &author, "draft logo", 1, now - 500,
            vec![
                vec!["t".into(), "logo".into()],
                vec!["status".into(), "draft".into()],
                vec!["downloads".into(), "0".into()],
            ],
        )).unwrap();

        // Recent approved icon (wrong tag).
        idx.index_event(&make_full_event(
            "e5", &author, "approved icon", 1, now - 100,
            vec![
                vec!["t".into(), "icon".into()],
                vec!["status".into(), "approved".into()],
                vec!["downloads".into(), "200".into()],
            ],
        )).unwrap();

        // The query: "assets tagged 'logo', approved, in last 30 days, sorted by most downloaded"
        let response = idx
            .compound_search(
                &CompoundQuery::new()
                    .with_tag_exact("t", "logo")
                    .with_tag_exact("status", "approved")
                    .with_time_range(Some(thirty_days_ago), Some(now))
                    .sorted_by(SortClause::by_tag("downloads", SortDirection::Desc)),
            )
            .unwrap();

        assert_eq!(response.total_matches, 2, "should match 2 recent approved logos");
        assert_eq!(response.results[0].event_id, "e2", "most downloaded first");
        assert_eq!(response.results[1].event_id, "e3", "fewer downloads second");
    }

    // ---- Compound query with faceted search ----

    #[test]
    fn compound_full_example_with_facets() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_full_event(
            "e1", &author, "design logo assets", 1, 1000,
            vec![
                vec!["t".into(), "logo".into()],
                vec!["status".into(), "approved".into()],
            ],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e2", &author, "design icon assets", 7030, 2000,
            vec![
                vec!["t".into(), "icon".into()],
                vec!["status".into(), "approved".into()],
            ],
        )).unwrap();
        idx.index_event(&make_full_event(
            "e3", &author, "design banner assets", 1, 3000,
            vec![
                vec!["t".into(), "banner".into()],
                vec!["status".into(), "draft".into()],
            ],
        )).unwrap();

        let response = idx
            .compound_search(
                &CompoundQuery::text("design")
                    .with_facet(FacetRequest::ByKind)
                    .with_facet(FacetRequest::ByTag("t".into()))
                    .with_facet(FacetRequest::ByTag("status".into())),
            )
            .unwrap();

        assert_eq!(response.total_matches, 3);
        assert_eq!(response.facets.len(), 3);

        // Kind facets.
        let kind_facet = &response.facets[0];
        assert_eq!(kind_facet.dimension, "kind");

        // Tag "t" facets.
        let t_facet = &response.facets[1];
        assert_eq!(t_facet.dimension, "tag:t");
        assert_eq!(t_facet.buckets.len(), 3);

        // Tag "status" facets.
        let status_facet = &response.facets[2];
        assert_eq!(status_facet.dimension, "tag:status");
        assert_eq!(status_facet.buckets.len(), 2);
    }

    // ---- Federation scope tests ----

    /// Helper: create an event with a community tag.
    fn make_community_event(
        id: &str,
        author: &str,
        content: &str,
        kind: u32,
        created_at: i64,
        community: &str,
        extra_tags: Vec<Vec<String>>,
    ) -> OmniEvent {
        let mut tags = vec![vec!["community".into(), community.into()]];
        tags.extend(extra_tags);
        OmniEvent {
            id: id.into(),
            author: author.into(),
            created_at,
            kind,
            tags,
            content: content.into(),
            sig: "c".repeat(128),
        }
    }

    #[test]
    fn scoped_search_unrestricted_returns_all() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_community_event(
            "e1", &author, "rust programming", 1, 1000, "guild-a", vec![],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e2", &author, "rust compiler", 1, 2000, "guild-b", vec![],
        )).unwrap();

        let scope = FederationScope::new();
        let response = idx
            .scoped_search(&SearchQuery::new("rust"), &scope)
            .unwrap();
        assert_eq!(response.results.len(), 2);
    }

    #[test]
    fn scoped_search_filters_by_community() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_community_event(
            "e1", &author, "rust programming", 1, 1000, "guild-a", vec![],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e2", &author, "rust compiler", 1, 2000, "guild-b", vec![],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e3", &author, "rust macros", 1, 3000, "guild-c", vec![],
        )).unwrap();

        let scope = FederationScope::from_communities(["guild-a", "guild-c"]);
        let response = idx
            .scoped_search(&SearchQuery::new("rust"), &scope)
            .unwrap();
        assert_eq!(response.results.len(), 2);
        let ids: Vec<&str> = response.results.iter().map(|r| r.event_id.as_str()).collect();
        assert!(ids.contains(&"e1"));
        assert!(ids.contains(&"e3"));
        assert!(!ids.contains(&"e2"));
    }

    #[test]
    fn scoped_search_excludes_untagged_events() {
        let idx = KeywordIndex::in_memory().unwrap();
        // Event with community tag.
        let author = "a".repeat(64);
        idx.index_event(&make_community_event(
            "e1", &author, "rust programming", 1, 1000, "guild-a", vec![],
        )).unwrap();
        // Event without community tag.
        idx.index_event(&make_event("e2", "rust compiler", 1)).unwrap();

        let scope = FederationScope::from_communities(["guild-a"]);
        let response = idx
            .scoped_search(&SearchQuery::new("rust"), &scope)
            .unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].event_id, "e1");
    }

    #[test]
    fn scoped_search_empty_query_returns_empty() {
        let idx = KeywordIndex::in_memory().unwrap();
        let scope = FederationScope::from_communities(["guild-a"]);
        let response = idx.scoped_search(&SearchQuery::new(""), &scope).unwrap();
        assert!(response.results.is_empty());
    }

    #[test]
    fn scoped_search_with_kind_filter() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_community_event(
            "e1", &author, "hello world", 1, 1000, "guild-a", vec![],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e2", &author, "hello beacon", 7030, 2000, "guild-a", vec![],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e3", &author, "hello other", 1, 3000, "guild-b", vec![],
        )).unwrap();

        let scope = FederationScope::from_communities(["guild-a"]);
        let response = idx
            .scoped_search(&SearchQuery::new("hello").with_kinds(vec![1]), &scope)
            .unwrap();
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].event_id, "e1");
    }

    #[test]
    fn scoped_compound_search_filters_by_community() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_community_event(
            "e1", &author, "logo design", 1, 1000, "guild-a",
            vec![vec!["t".into(), "logo".into()]],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e2", &author, "icon design", 1, 2000, "guild-b",
            vec![vec!["t".into(), "icon".into()]],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e3", &author, "banner design", 1, 3000, "guild-a",
            vec![vec!["t".into(), "banner".into()]],
        )).unwrap();

        let scope = FederationScope::from_communities(["guild-a"]);
        let response = idx
            .scoped_compound_search(&CompoundQuery::new(), &scope)
            .unwrap();
        assert_eq!(response.total_matches, 2);
        let ids: Vec<&str> = response.results.iter().map(|r| r.event_id.as_str()).collect();
        assert!(ids.contains(&"e1"));
        assert!(ids.contains(&"e3"));
    }

    #[test]
    fn scoped_compound_search_with_text_and_tags() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_community_event(
            "e1", &author, "rust logo", 1, 1000, "guild-a",
            vec![vec!["t".into(), "logo".into()]],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e2", &author, "rust icon", 1, 2000, "guild-a",
            vec![vec!["t".into(), "icon".into()]],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e3", &author, "rust banner", 1, 3000, "guild-b",
            vec![vec!["t".into(), "logo".into()]],
        )).unwrap();

        let scope = FederationScope::from_communities(["guild-a"]);
        let response = idx
            .scoped_compound_search(
                &CompoundQuery::text("rust").with_tag_exact("t", "logo"),
                &scope,
            )
            .unwrap();
        assert_eq!(response.total_matches, 1);
        assert_eq!(response.results[0].event_id, "e1");
    }

    #[test]
    fn scoped_compound_search_unrestricted_matches_all() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_community_event(
            "e1", &author, "hello", 1, 1000, "guild-a", vec![],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e2", &author, "world", 1, 2000, "guild-b", vec![],
        )).unwrap();

        let scope = FederationScope::new();
        let response = idx
            .scoped_compound_search(&CompoundQuery::new(), &scope)
            .unwrap();
        assert_eq!(response.total_matches, 2);
    }

    #[test]
    fn scoped_compound_search_facets_respect_scope() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_community_event(
            "e1", &author, "post one", 1, 1000, "guild-a",
            vec![vec!["t".into(), "topic-1".into()]],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e2", &author, "post two", 1, 2000, "guild-b",
            vec![vec!["t".into(), "topic-2".into()]],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e3", &author, "post three", 7030, 3000, "guild-a",
            vec![vec!["t".into(), "topic-1".into()]],
        )).unwrap();

        let scope = FederationScope::from_communities(["guild-a"]);
        let response = idx
            .scoped_compound_search(
                &CompoundQuery::new()
                    .with_facet(FacetRequest::ByKind)
                    .with_facet(FacetRequest::ByTag("t".into())),
                &scope,
            )
            .unwrap();

        assert_eq!(response.total_matches, 2);

        // Kind facets should only count guild-a events.
        let kind_facet = &response.facets[0];
        assert_eq!(kind_facet.dimension, "kind");
        assert_eq!(kind_facet.buckets.len(), 2); // kind 1 and 7030

        // Tag facets should only show guild-a tags.
        let t_facet = &response.facets[1];
        assert_eq!(t_facet.dimension, "tag:t");
        assert_eq!(t_facet.buckets.len(), 1); // only topic-1
        assert_eq!(t_facet.buckets[0].value, "topic-1");
        assert_eq!(t_facet.buckets[0].count, 2);
    }

    #[test]
    fn scoped_aggregate_count_filters_by_community() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_community_event(
            "e1", &author, "post one", 1, 1000, "guild-a", vec![],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e2", &author, "post two", 1, 2000, "guild-b", vec![],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e3", &author, "post three", 1, 3000, "guild-a", vec![],
        )).unwrap();

        let scope = FederationScope::from_communities(["guild-a"]);
        let response = idx
            .scoped_aggregate(&AggregateQuery::count(), &scope)
            .unwrap();
        assert_eq!(response.value, Some(2.0));
    }

    #[test]
    fn scoped_aggregate_sum_filters_by_community() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_community_event(
            "e1", &author, "asset one", 1, 1000, "guild-a",
            vec![vec!["downloads".into(), "100".into()]],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e2", &author, "asset two", 1, 2000, "guild-b",
            vec![vec!["downloads".into(), "250".into()]],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e3", &author, "asset three", 1, 3000, "guild-a",
            vec![vec!["downloads".into(), "50".into()]],
        )).unwrap();

        let scope = FederationScope::from_communities(["guild-a"]);
        let response = idx
            .scoped_aggregate(&AggregateQuery::sum("downloads"), &scope)
            .unwrap();
        assert_eq!(response.value, Some(150.0));
    }

    #[test]
    fn scoped_aggregate_count_by_kind_filters_by_community() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_community_event(
            "e1", &author, "post", 1, 1000, "guild-a", vec![],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e2", &author, "beacon", 7030, 2000, "guild-a", vec![],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e3", &author, "post", 1, 3000, "guild-b", vec![],
        )).unwrap();

        let scope = FederationScope::from_communities(["guild-a"]);
        let response = idx
            .scoped_aggregate(&AggregateQuery::count_by(GroupBy::Kind), &scope)
            .unwrap();
        assert_eq!(response.groups.len(), 2);
        let kind_1 = response.groups.iter().find(|g| g.key == "1").unwrap();
        assert_eq!(kind_1.value, 1.0);
        let kind_7030 = response.groups.iter().find(|g| g.key == "7030").unwrap();
        assert_eq!(kind_7030.value, 1.0);
    }

    #[test]
    fn scoped_aggregate_unrestricted_matches_all() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_community_event(
            "e1", &author, "post one", 1, 1000, "guild-a", vec![],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e2", &author, "post two", 1, 2000, "guild-b", vec![],
        )).unwrap();

        let scope = FederationScope::new();
        let response = idx
            .scoped_aggregate(&AggregateQuery::count(), &scope)
            .unwrap();
        assert_eq!(response.value, Some(2.0));
    }

    #[test]
    fn scoped_search_no_matching_community_returns_empty() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_community_event(
            "e1", &author, "rust programming", 1, 1000, "guild-a", vec![],
        )).unwrap();

        let scope = FederationScope::from_communities(["guild-z"]);
        let response = idx
            .scoped_search(&SearchQuery::new("rust"), &scope)
            .unwrap();
        assert!(response.results.is_empty());
    }

    #[test]
    fn scoped_compound_search_pagination_with_scope() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        for i in 0..10 {
            idx.index_event(&make_community_event(
                &format!("e{i}"), &author, &format!("post number {i}"), 1,
                1000 + i, "guild-a", vec![],
            )).unwrap();
        }
        // Events from another community (should be excluded).
        for i in 10..15 {
            idx.index_event(&make_community_event(
                &format!("e{i}"), &author, &format!("post number {i}"), 1,
                1000 + i, "guild-b", vec![],
            )).unwrap();
        }

        let scope = FederationScope::from_communities(["guild-a"]);
        let page1 = idx
            .scoped_compound_search(
                &CompoundQuery::new()
                    .sorted_by(SortClause::oldest_first())
                    .with_limit(3)
                    .with_offset(0),
                &scope,
            )
            .unwrap();
        assert_eq!(page1.total_matches, 10, "total should be 10 guild-a events");
        assert_eq!(page1.results.len(), 3);

        let page2 = idx
            .scoped_compound_search(
                &CompoundQuery::new()
                    .sorted_by(SortClause::oldest_first())
                    .with_limit(3)
                    .with_offset(3),
                &scope,
            )
            .unwrap();
        assert_eq!(page2.total_matches, 10);
        assert_eq!(page2.results.len(), 3);
    }

    #[test]
    fn scoped_multi_community_scope() {
        let idx = KeywordIndex::in_memory().unwrap();
        let author = "a".repeat(64);
        idx.index_event(&make_community_event(
            "e1", &author, "hello from alpha", 1, 1000, "alpha", vec![],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e2", &author, "hello from beta", 1, 2000, "beta", vec![],
        )).unwrap();
        idx.index_event(&make_community_event(
            "e3", &author, "hello from gamma", 1, 3000, "gamma", vec![],
        )).unwrap();

        // Scope to alpha + gamma.
        let scope = FederationScope::from_communities(["alpha", "gamma"]);
        let response = idx
            .scoped_search(&SearchQuery::new("hello"), &scope)
            .unwrap();
        assert_eq!(response.results.len(), 2);
        let ids: Vec<&str> = response.results.iter().map(|r| r.event_id.as_str()).collect();
        assert!(ids.contains(&"e1"));
        assert!(ids.contains(&"e3"));
    }
}
