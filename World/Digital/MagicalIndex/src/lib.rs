//! MagicalIndex — demand-driven search for the Omnidea network.
//!
//! A rich query engine for Tower nodes. Indexes events as they arrive
//! (demand-driven, not crawling) and provides:
//!
//! - **Full-text search** — FTS5 with Porter stemming and BM25 ranking
//! - **Compound queries** — tag filters, multi-field sorting, pagination
//! - **Faceted search** — filter and count by multiple dimensions
//! - **Aggregation** — count, sum, min/max, avg, group-by
//! - **Zeitgeist signals** — query signals for trending/discovery
//!
//! # Architecture
//!
//! - `SearchIndex` trait — pluggable backend (keyword, semantic, composite)
//! - `KeywordIndex` — default implementation using SQLite FTS5
//! - `SearchQuery` / `SearchResponse` — simple text search
//! - `CompoundQuery` / `CompoundResponse` — rich multi-dimensional queries
//! - `AggregateQuery` / `AggregateResponse` — aggregation operations
//! - `SignalCollector` — collects query signals for Zeitgeist
//!
//! MagicalIndex is a library crate. Tower uses it. Omnibus could use it.
//! The index backend is pluggable — swap SQLite for something else without
//! changing the protocol. Add new index types (semantic, geographic,
//! temporal) by implementing `SearchIndex`.

pub mod aggregation;
pub mod compound;
pub mod error;
pub mod federation_scope;
pub mod keyword;
pub mod query;
pub mod signals;
pub mod traits;

pub use aggregation::{AggregateQuery, AggregateResponse};
pub use compound::{CompoundQuery, CompoundResponse, FacetRequest, SortClause, SortDirection};
pub use error::MagicalError;
pub use federation_scope::FederationScope;
pub use keyword::KeywordIndex;
pub use query::{SearchQuery, SearchResponse, SearchResult};
pub use signals::{QuerySignal, SignalCollector};
pub use traits::SearchIndex;
