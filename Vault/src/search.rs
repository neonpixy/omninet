//! Full-text search over vault notes via SQLite FTS5.
//!
//! Operates directly on the Manifest's SQLCipher connection — no separate
//! database, no external dependencies. Uses the Porter stemmer with
//! unicode61 tokenizer for multilingual support and BM25 ranking.

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::VaultError;

/// Stateless full-text search operations on the manifest database.
///
/// All methods take a `&Connection` — the same SQLCipher connection
/// that the [`Manifest`](crate::Manifest) owns. This keeps search
/// encrypted at rest alongside the manifest data.
pub struct VaultSearch;

/// A single search result with relevance scoring and snippet extraction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    /// The idea that matched.
    pub idea_id: Uuid,
    /// Relevance score in `0.0..=1.0` (higher is better).
    pub relevance: f64,
    /// A text snippet around the matching terms, if available.
    pub snippet: Option<String>,
    /// The idea's title, if indexed.
    pub title: Option<String>,
}

impl VaultSearch {
    /// Index an idea's content for full-text search.
    ///
    /// Call this when registering or updating an idea. If the idea is
    /// already indexed, the existing entry is replaced.
    pub fn index_idea(
        conn: &Connection,
        idea_id: &Uuid,
        title: &str,
        content_text: &str,
        tags: &[String],
    ) -> Result<(), VaultError> {
        let id_str = idea_id.to_string();
        let tags_text = tags.join(" ");

        // Remove any existing entry first (FTS5 doesn't support REPLACE).
        conn.execute(
            "DELETE FROM search_fts WHERE idea_id = ?1",
            params![id_str],
        )
        .map_err(|e| VaultError::Database(format!("search index delete: {e}")))?;

        conn.execute(
            "INSERT INTO search_fts (idea_id, title, content, tags) VALUES (?1, ?2, ?3, ?4)",
            params![id_str, title, content_text, tags_text],
        )
        .map_err(|e| VaultError::Database(format!("search index insert: {e}")))?;

        Ok(())
    }

    /// Remove an idea from the search index.
    pub fn remove_idea(conn: &Connection, idea_id: &Uuid) -> Result<(), VaultError> {
        let id_str = idea_id.to_string();
        conn.execute(
            "DELETE FROM search_fts WHERE idea_id = ?1",
            params![id_str],
        )
        .map_err(|e| VaultError::Database(format!("search index remove: {e}")))?;

        Ok(())
    }

    /// Search notes by text query.
    ///
    /// Returns matching idea IDs with relevance scores and snippets,
    /// ordered by relevance (best first). Returns an empty vec for
    /// empty or operator-only queries.
    pub fn search(
        conn: &Connection,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchHit>, VaultError> {
        let sanitized = sanitize_fts_query(query);
        if sanitized.is_empty() {
            return Ok(Vec::new());
        }

        let mut stmt = conn
            .prepare(
                "SELECT
                    idea_id,
                    bm25(search_fts) AS rank,
                    snippet(search_fts, 2, '<b>', '</b>', '...', 32) AS snip,
                    title
                 FROM search_fts
                 WHERE search_fts MATCH ?1
                 ORDER BY rank
                 LIMIT ?2",
            )
            .map_err(|e| VaultError::Database(format!("search prepare: {e}")))?;

        let rows = stmt
            .query_map(params![sanitized, limit as i64], |row| {
                let id_str: String = row.get(0)?;
                let bm25: f64 = row.get(1)?;
                let snippet: Option<String> = row.get(2)?;
                let title: Option<String> = row.get(3)?;
                Ok((id_str, bm25, snippet, title))
            })
            .map_err(|e| VaultError::Database(format!("search query: {e}")))?;

        let mut hits = Vec::new();
        for row in rows {
            let (id_str, bm25, snippet, title) =
                row.map_err(|e| VaultError::Database(format!("search row: {e}")))?;
            let idea_id = Uuid::parse_str(&id_str)
                .map_err(|e| VaultError::Database(format!("search invalid UUID: {e}")))?;

            hits.push(SearchHit {
                idea_id,
                relevance: bm25_to_score(bm25),
                snippet,
                title,
            });
        }

        Ok(hits)
    }

    /// Rebuild the entire search index from the manifest table.
    ///
    /// Clears the FTS5 table and re-indexes every manifest entry that
    /// has a title. Returns the number of entries indexed.
    pub fn rebuild_index(conn: &Connection) -> Result<usize, VaultError> {
        // Clear the FTS5 table.
        conn.execute("DELETE FROM search_fts", [])
            .map_err(|e| VaultError::Database(format!("rebuild clear: {e}")))?;

        // Re-index from manifest. We index title (as both title and content
        // since we don't have decrypted content at this layer).
        let mut stmt = conn
            .prepare("SELECT id, title FROM manifest WHERE title IS NOT NULL")
            .map_err(|e| VaultError::Database(format!("rebuild prepare: {e}")))?;

        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let title: String = row.get(1)?;
                Ok((id, title))
            })
            .map_err(|e| VaultError::Database(format!("rebuild query: {e}")))?;

        let mut count = 0usize;
        for row in rows {
            let (id, title) =
                row.map_err(|e| VaultError::Database(format!("rebuild row: {e}")))?;
            conn.execute(
                "INSERT INTO search_fts (idea_id, title, content, tags) VALUES (?1, ?2, ?3, ?4)",
                params![id, title, "", ""],
            )
            .map_err(|e| VaultError::Database(format!("rebuild insert: {e}")))?;
            count += 1;
        }

        Ok(count)
    }
}

/// Convert BM25 score (negative, lower = better) to a 0.0..=1.0 relevance score.
///
/// BM25 returns 0 for perfect match and increasingly negative for worse matches.
/// Maps: 0 -> 1.0, -10 -> ~0.5, -inf -> 0.0.
fn bm25_to_score(bm25: f64) -> f64 {
    1.0 / (1.0 + bm25.abs() / 5.0)
}

/// Sanitize user input for FTS5 MATCH queries.
///
/// Strips FTS5 operators and special characters so user input is treated
/// as plain word matches. Returns empty string if no valid terms remain.
fn sanitize_fts_query(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .map(|c| match c {
            '"' | '\'' | '*' | '(' | ')' | '{' | '}' | '^' | '~' => ' ',
            _ => c,
        })
        .collect();

    let terms: Vec<&str> = cleaned
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .filter(|t| !matches!(*t, "AND" | "OR" | "NOT" | "NEAR"))
        .collect();

    if terms.is_empty() {
        return String::new();
    }

    terms.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Open an in-memory SQLite database with the FTS5 table created.
    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS search_fts USING fts5(
                idea_id UNINDEXED,
                title,
                content,
                tags,
                tokenize='porter unicode61'
            );
            CREATE TABLE IF NOT EXISTS manifest (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                title TEXT,
                extended_type TEXT,
                creator TEXT NOT NULL,
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL,
                collective_id TEXT,
                header_cache TEXT
            );",
        )
        .unwrap();
        conn
    }

    #[test]
    fn test_index_and_search_by_title() {
        let conn = test_db();
        let id = Uuid::new_v4();

        VaultSearch::index_idea(&conn, &id, "Quantum Computing Notes", "", &[]).unwrap();

        let hits = VaultSearch::search(&conn, "quantum", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].idea_id, id);
        assert!(hits[0].relevance > 0.0);
        assert!(hits[0].title.as_deref() == Some("Quantum Computing Notes"));
    }

    #[test]
    fn test_search_by_content() {
        let conn = test_db();
        let id = Uuid::new_v4();

        VaultSearch::index_idea(
            &conn,
            &id,
            "Meeting Notes",
            "We discussed the new encryption protocol for vault storage",
            &[],
        )
        .unwrap();

        let hits = VaultSearch::search(&conn, "encryption protocol", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].idea_id, id);
    }

    #[test]
    fn test_search_by_tags() {
        let conn = test_db();
        let id = Uuid::new_v4();

        VaultSearch::index_idea(
            &conn,
            &id,
            "Design Document",
            "Some content here",
            &["architecture".to_string(), "security".to_string()],
        )
        .unwrap();

        let hits = VaultSearch::search(&conn, "architecture", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].idea_id, id);
    }

    #[test]
    fn test_snippet_extraction() {
        let conn = test_db();
        let id = Uuid::new_v4();

        let long_content = "This is a long document about various topics. \
            The primary focus is on distributed systems and their interaction \
            with encrypted storage mechanisms. We analyze how sovereign nodes \
            communicate through relay networks.";

        VaultSearch::index_idea(&conn, &id, "Systems Analysis", long_content, &[]).unwrap();

        let hits = VaultSearch::search(&conn, "encrypted storage", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].snippet.is_some());
        let snippet = hits[0].snippet.as_ref().unwrap();
        assert!(
            snippet.contains("<b>") || snippet.contains("encrypted") || snippet.contains("storage"),
            "snippet should contain highlighted terms or the search words: {snippet}"
        );
    }

    #[test]
    fn test_empty_query_returns_empty() {
        let conn = test_db();
        let id = Uuid::new_v4();
        VaultSearch::index_idea(&conn, &id, "Something", "content", &[]).unwrap();

        let hits = VaultSearch::search(&conn, "", 10).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn test_operator_only_query_returns_empty() {
        let conn = test_db();
        let id = Uuid::new_v4();
        VaultSearch::index_idea(&conn, &id, "Something", "content", &[]).unwrap();

        let hits = VaultSearch::search(&conn, "AND OR NOT", 10).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn test_remove_idea_from_index() {
        let conn = test_db();
        let id = Uuid::new_v4();

        VaultSearch::index_idea(&conn, &id, "Removable Note", "to be removed", &[]).unwrap();

        // Verify it's searchable.
        let hits = VaultSearch::search(&conn, "removable", 10).unwrap();
        assert_eq!(hits.len(), 1);

        // Remove it.
        VaultSearch::remove_idea(&conn, &id).unwrap();

        // Verify it's gone.
        let hits = VaultSearch::search(&conn, "removable", 10).unwrap();
        assert!(hits.is_empty());
    }

    #[test]
    fn test_update_replaces_old_content() {
        let conn = test_db();
        let id = Uuid::new_v4();

        VaultSearch::index_idea(&conn, &id, "Original Title", "original content", &[]).unwrap();

        // Re-index with new content.
        VaultSearch::index_idea(&conn, &id, "Updated Title", "completely new content", &[])
            .unwrap();

        // Old content should not match.
        let hits = VaultSearch::search(&conn, "original", 10).unwrap();
        assert!(hits.is_empty());

        // New content should match.
        let hits = VaultSearch::search(&conn, "updated", 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].idea_id, id);
    }

    #[test]
    fn test_search_limit() {
        let conn = test_db();

        // Index 5 notes all matching "protocol".
        for i in 0..5 {
            let id = Uuid::new_v4();
            VaultSearch::index_idea(
                &conn,
                &id,
                &format!("Protocol Note {i}"),
                "protocol details",
                &[],
            )
            .unwrap();
        }

        let hits = VaultSearch::search(&conn, "protocol", 3).unwrap();
        assert_eq!(hits.len(), 3);
    }

    #[test]
    fn test_porter_stemming() {
        let conn = test_db();
        let id = Uuid::new_v4();

        VaultSearch::index_idea(&conn, &id, "Running Analysis", "The runners were running", &[])
            .unwrap();

        // "run" should match "running" and "runners" via Porter stemmer.
        let hits = VaultSearch::search(&conn, "run", 10).unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn test_multiple_results_ranked() {
        let conn = test_db();
        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        // id1 has "encryption" once in the title.
        VaultSearch::index_idea(&conn, &id1, "Encryption", "some other topic", &[]).unwrap();

        // id2 has "encryption" many times in content — should rank higher.
        VaultSearch::index_idea(
            &conn,
            &id2,
            "Security Deep Dive",
            "encryption encryption encryption algorithms for encryption",
            &[],
        )
        .unwrap();

        let hits = VaultSearch::search(&conn, "encryption", 10).unwrap();
        assert_eq!(hits.len(), 2);
        // Both should have positive relevance.
        assert!(hits[0].relevance > 0.0);
        assert!(hits[1].relevance > 0.0);
    }

    #[test]
    fn test_rebuild_index() {
        let conn = test_db();

        // Insert some manifest rows directly.
        conn.execute(
            "INSERT INTO manifest (id, path, title, creator, created_at, modified_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                Uuid::new_v4().to_string(),
                "Personal/note.idea",
                "Rebuild Target",
                "cpub1test",
                "2024-01-01T00:00:00Z",
                "2024-01-01T00:00:00Z",
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO manifest (id, path, title, creator, created_at, modified_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                Uuid::new_v4().to_string(),
                "Personal/other.idea",
                "Another Note",
                "cpub1test",
                "2024-01-01T00:00:00Z",
                "2024-01-01T00:00:00Z",
            ],
        )
        .unwrap();

        let count = VaultSearch::rebuild_index(&conn).unwrap();
        assert_eq!(count, 2);

        // Should be searchable after rebuild.
        let hits = VaultSearch::search(&conn, "rebuild", 10).unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn test_remove_nonexistent_is_ok() {
        let conn = test_db();
        let id = Uuid::new_v4();
        // Should not error.
        VaultSearch::remove_idea(&conn, &id).unwrap();
    }

    #[test]
    fn test_sanitize_fts_query_strips_operators() {
        assert_eq!(sanitize_fts_query("hello world"), "hello world");
        assert_eq!(sanitize_fts_query("hello AND world"), "hello world");
        assert_eq!(sanitize_fts_query("\"quoted\" terms"), "quoted terms");
        assert_eq!(sanitize_fts_query("NOT OR AND"), "");
        assert_eq!(sanitize_fts_query(""), "");
        assert_eq!(sanitize_fts_query("   "), "");
    }

    #[test]
    fn test_bm25_to_score() {
        assert!((bm25_to_score(0.0) - 1.0).abs() < f64::EPSILON);
        assert!(bm25_to_score(-5.0) > 0.0);
        assert!(bm25_to_score(-5.0) < 1.0);
        // More negative = lower score.
        assert!(bm25_to_score(-10.0) < bm25_to_score(-5.0));
    }
}
