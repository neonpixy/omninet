//! Signals — query signals for Zeitgeist integration.
//!
//! MagicalIndex emits `QuerySignal` when searches happen. Zeitgeist's
//! TrendTracker consumes these to build trending topics. This is the
//! data bridge between "what people search for" and "what's trending."

use serde::{Deserialize, Serialize};

/// A signal emitted when a query is executed.
///
/// Zeitgeist's TrendTracker can consume these via `record_query()`.
/// The signal captures what was searched and how many results came back,
/// without exposing who searched.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QuerySignal {
    /// The search terms (normalized, lowercase).
    pub terms: Vec<String>,
    /// Number of results returned.
    pub result_count: usize,
    /// When the query happened (Unix timestamp).
    pub timestamp: i64,
    /// Which kinds were filtered (if any).
    pub kinds: Option<Vec<u32>>,
}

impl QuerySignal {
    /// Create a signal from a search query and its results.
    pub fn from_search(text: &str, result_count: usize, timestamp: i64) -> Self {
        let terms: Vec<String> = text
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .filter(|w| w.len() > 2)
            .collect();

        Self {
            terms,
            result_count,
            timestamp,
            kinds: None,
        }
    }

    /// Attach kind filters to the signal.
    pub fn with_kinds(mut self, kinds: Vec<u32>) -> Self {
        self.kinds = Some(kinds);
        self
    }
}

/// Collects query signals for later consumption.
///
/// Thread-safe. Call `drain()` to take all pending signals
/// and feed them to Zeitgeist's TrendTracker.
#[derive(Debug, Default)]
pub struct SignalCollector {
    signals: std::sync::Mutex<Vec<QuerySignal>>,
}

impl SignalCollector {
    /// Create a new empty collector.
    pub fn new() -> Self {
        Self {
            signals: std::sync::Mutex::new(Vec::new()),
        }
    }

    /// Record a query signal.
    pub fn record(&self, signal: QuerySignal) {
        if let Ok(mut signals) = self.signals.lock() {
            signals.push(signal);
        }
    }

    /// Take all pending signals, clearing the buffer.
    ///
    /// Returns an empty vec if no signals are pending.
    pub fn drain(&self) -> Vec<QuerySignal> {
        if let Ok(mut signals) = self.signals.lock() {
            std::mem::take(&mut *signals)
        } else {
            Vec::new()
        }
    }

    /// Number of pending signals.
    pub fn pending_count(&self) -> usize {
        self.signals.lock().map(|s| s.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signal_from_search() {
        let signal = QuerySignal::from_search("woodworking joints", 5, 1000);
        assert_eq!(signal.terms, vec!["woodworking", "joints"]);
        assert_eq!(signal.result_count, 5);
        assert_eq!(signal.timestamp, 1000);
        assert!(signal.kinds.is_none());
    }

    #[test]
    fn signal_filters_short_words() {
        let signal = QuerySignal::from_search("a is the woodworking", 1, 1000);
        // "a" (1 char) and "is" (2 chars) are filtered, "the" (3 chars) stays.
        assert_eq!(signal.terms, vec!["the", "woodworking"]);
    }

    #[test]
    fn signal_with_kinds() {
        let signal = QuerySignal::from_search("test", 0, 1000).with_kinds(vec![1, 7030]);
        assert_eq!(signal.kinds, Some(vec![1, 7030]));
    }

    #[test]
    fn signal_serde_round_trip() {
        let signal = QuerySignal::from_search("rust programming", 10, 5000);
        let json = serde_json::to_string(&signal).unwrap();
        let loaded: QuerySignal = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.terms, vec!["rust", "programming"]);
        assert_eq!(loaded.result_count, 10);
    }

    #[test]
    fn collector_record_and_drain() {
        let collector = SignalCollector::new();
        assert_eq!(collector.pending_count(), 0);

        collector.record(QuerySignal::from_search("hello", 1, 1000));
        collector.record(QuerySignal::from_search("world", 2, 2000));
        assert_eq!(collector.pending_count(), 2);

        let signals = collector.drain();
        assert_eq!(signals.len(), 2);
        assert_eq!(collector.pending_count(), 0);

        // Drain again returns empty.
        let signals = collector.drain();
        assert!(signals.is_empty());
    }

    #[test]
    fn collector_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SignalCollector>();
    }
}
