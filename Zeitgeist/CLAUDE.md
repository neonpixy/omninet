# Zeitgeist — Discovery & Culture

The collective pulse. Zeitgeist is the user-facing discovery engine for Omninet. It runs on the user's device (not on Towers), reads the gospel directory, picks the right Towers for each query, queries them directly in parallel, and merges the results. Sovereignty-preserving discovery — you choose the lens; Zeitgeist provides the signal.

## Architecture

Zeitgeist is a client-side crate. It does not run on Towers — it reads Tower data (lighthouse announcements, Semantic Profiles) and routes queries to the most relevant Towers.

The pipeline:
1. **TowerDirectory** ingests gospel data (lighthouse announcements kind 7032, Semantic Profiles kind 26000) into a queryable map of Towers and their capabilities.
2. **QueryRouter** scores Towers by topic overlap with the query, Harbor preference, content count, and semantic search availability. Selects the top N most relevant.
3. The caller queries those Towers in parallel (Zeitgeist doesn't own the network layer).
4. **ResultMerger** deduplicates results by event ID, weights scores by Tower relevance, aggregates suggestions, and sorts by combined score.
5. **LocalCache** stores recent query results for instant replay (LRU eviction, configurable capacity).
6. **TrendTracker** aggregates search queries and Tower topic data into trending signals with time decay.

## Key Types

### `TowerDirectory` (`directory.rs`)
- Decoded, queryable view of gospel Tower data
- `TowerEntry` — pubkey + `TowerInfo` (from lighthouse) + `TowerCapabilities` (from Semantic Profile)
- `TowerInfo` — mode (pharos/harbor), relay URL, name, event/gospel counts, uptime, version, communities served
- `TowerCapabilities` — keyword_search, semantic_search, suggestions, topics, embedding_model, content_count
- `update(&mut self, events: &[OmniEvent])` — source-agnostic ingestion. Feed it events from any source.
- Queries: `searchable_towers()`, `harbors()`, `towers_for_community()`, `get(pubkey)`
- Newer lighthouse events replace older ones; profiles before lighthouse are silently skipped

### `QueryRouter` (`router.rs`)
- `route(query, directory) -> Vec<RoutedTower>` — selects best Towers for a text query
- Scoring: topic match (keyword overlap) + Harbor bonus (+0.1) + content bonus (log scale, max 0.15) + semantic bonus (+0.05) + fallback (0.01 for any searchable Tower)
- `RouterConfig` — max_towers (default 5), min_relevance (default 0.0), prefer_harbors (default true)
- `RoutedTower` — pubkey, relay_url, relevance score, has_semantic flag

### `ResultMerger` (`merger.rs`)
- `merge(batches: Vec<TowerResultBatch>) -> MergedResponse`
- Deduplicates by event_id, keeps best weighted score per event
- `MergedResult` — result + sources (which Towers returned it) + combined_score
- `MergedResponse` — results, aggregated suggestions, tower_count, total_raw_results
- Default max 50 results, configurable

### `LocalCache` (`cache.rs`)
- Personal result cache, LRU eviction by last_accessed
- `get(query, now)` — lookup with hit tracking. `put(query, results, now)` — store with truncation.
- Query normalization: lowercase, trim, collapse whitespace
- `snapshot()` / `from_snapshot()` for persistence
- `CacheConfig` — max_results_per_query (default 20), max_queries (default 500)
- Uses `SearchResult` from MagicalIndex

### `TrendTracker` (`trending.rs`)
- `record_query(query, now)` — extracts terms (>2 chars), boosts their scores
- `record_tower_topics(topics, content_count, now)` — weight by log of content count
- `decay()` — applies decay_factor to all scores, prunes below min_score
- `top(n)` — returns top N trending topics sorted by score
- `TrendConfig` — max_trends (default 100), decay_factor (default 0.95), min_score (default 0.01)
- `TrendSignal` — topic, score, signal_count, last_updated

### `DiscoveryProvider` trait (`traits.rs`)
- Pluggable discovery strategy: `search()`, `browse()`, `trending()`
- Object-safe (can be `Box<dyn DiscoveryProvider>`)
- `DiscoveryQuery` — builder pattern: `search(text).with_limit(n).with_kinds(vec).with_community(pk).without_cache()`
- `BrowseCategory` — name, description, approximate item_count

### `ZeitgeistError` (`error.rs`)
- NoTowersAvailable, EmptyQuery, InvalidProfile, InvalidAnnouncement, CacheError, SerializationError

### `GlobalTrendTracker` (`global_trends.rs`) — R4C Cross-Community Zeitgeist
- Extends the local `TrendTracker` with a global scope that weights trends by community diversity. A topic popular in 50 small communities scores higher than one popular in 1 large community.
- **Diversity formula:** `global_score = sum(local_scores) * simpson_diversity_index`, where `diversity_index = 1 - sum((community_share)^2)`. Single-community topics get diversity 0, so `global_score = 0` -- cannot dominate global trends.
- `record_community_trend(topic, community_id, score, sentiment)` -- record a community's signal. Topics are case-insensitive (lowercase normalized). Overwrites previous score for same topic+community.
- `register_community(id)` -- register a community as known (for "not discussing" counts).
- `top_global(n)` -- top N globally trending topics ranked by diversity-weighted score.
- `get_trend(topic)` -- single topic's global trend.
- `perspective(topic)` -- how different communities view the same topic: positive/negative/neutral/not-discussing counts + `consensus_level` (0.0 total disagreement to 1.0 total agreement).

**Types:**
- **ZeitgeistScope** -- query scope: Local(community_id), Communities(vec), Global. `includes(community_id)`.
- **TrendSentiment** -- Positive, Negative, Neutral, Mixed, Unknown.
- **CommunityTrendView** -- community_id + local_score + sentiment.
- **GlobalTrend** -- topic, global_score, community_count, diversity_index, community_breakdown.
- **TrendPerspective** -- topic, communities_discussing, communities_positive/negative/neutral, communities_not_discussing, consensus_level.

## Dependencies

```toml
globe = { path = "../Globe" }           # OmniEvent, kind constants
crown = { path = "../Crown" }           # Identity
magical-index = { path = "../World/Digital/MagicalIndex" }  # SearchResult type
serde, serde_json, thiserror, log
```

**Zero async.** Zeitgeist is pure data structures and synchronous logic. The caller owns the network layer.

## Covenant Alignment

**Sovereignty** — you choose the lens, not the algorithm. Discovery runs on your device. **Dignity** — every creation has equal chance of discovery; no pay-to-play. **Consent** — recommendations are opt-in and transparent. No individual data exposed — only aggregate signals.
