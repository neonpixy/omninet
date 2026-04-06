use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single captured log entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEntry {
    /// When the log was recorded.
    pub timestamp: DateTime<Utc>,
    /// Log level: "ERROR", "WARN", "INFO", "DEBUG", or "TRACE".
    pub level: String,
    /// The module that produced the log (if available).
    pub module: Option<String>,
    /// The log message.
    pub message: String,
}

/// A ring buffer for capturing log entries.
///
/// When the buffer reaches capacity, the oldest entry is dropped on each push.
/// The app layer is responsible for feeding entries via `push()` — LogCapture
/// does NOT install itself as a global logger.
pub struct LogCapture {
    entries: VecDeque<LogEntry>,
    capacity: usize,
}

impl LogCapture {
    /// Create a new log capture with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(capacity.min(8192)),
            capacity,
        }
    }

    /// Push a log entry. Drops the oldest entry if at capacity.
    pub fn push(&mut self, entry: LogEntry) {
        if self.capacity == 0 {
            return;
        }
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// Get the last N entries (most recent last).
    pub fn recent(&self, count: usize) -> Vec<LogEntry> {
        let skip = self.entries.len().saturating_sub(count);
        self.entries.iter().skip(skip).cloned().collect()
    }

    /// Get all entries (oldest first).
    pub fn all(&self) -> Vec<LogEntry> {
        self.entries.iter().cloned().collect()
    }

    /// Number of entries currently stored.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// The maximum number of entries this buffer can hold.
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(level: &str, message: &str) -> LogEntry {
        LogEntry {
            timestamp: Utc::now(),
            level: level.into(),
            module: Some("test".into()),
            message: message.into(),
        }
    }

    #[test]
    fn new_capture_is_empty() {
        let cap = LogCapture::new(100);
        assert!(cap.is_empty());
        assert_eq!(cap.len(), 0);
        assert_eq!(cap.capacity(), 100);
    }

    #[test]
    fn push_and_retrieve() {
        let mut cap = LogCapture::new(10);
        cap.push(make_entry("INFO", "hello"));
        cap.push(make_entry("WARN", "watch out"));

        assert_eq!(cap.len(), 2);
        assert!(!cap.is_empty());

        let all = cap.all();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].message, "hello");
        assert_eq!(all[1].message, "watch out");
    }

    #[test]
    fn recent_returns_last_n() {
        let mut cap = LogCapture::new(100);
        for i in 0..10 {
            cap.push(make_entry("INFO", &format!("msg {i}")));
        }

        let recent = cap.recent(3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].message, "msg 7");
        assert_eq!(recent[1].message, "msg 8");
        assert_eq!(recent[2].message, "msg 9");
    }

    #[test]
    fn recent_with_count_larger_than_buffer() {
        let mut cap = LogCapture::new(100);
        cap.push(make_entry("INFO", "only one"));

        let recent = cap.recent(50);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].message, "only one");
    }

    #[test]
    fn capacity_overflow_drops_oldest() {
        let mut cap = LogCapture::new(3);
        cap.push(make_entry("INFO", "first"));
        cap.push(make_entry("INFO", "second"));
        cap.push(make_entry("INFO", "third"));
        assert_eq!(cap.len(), 3);

        // This should drop "first".
        cap.push(make_entry("INFO", "fourth"));
        assert_eq!(cap.len(), 3);

        let all = cap.all();
        assert_eq!(all[0].message, "second");
        assert_eq!(all[1].message, "third");
        assert_eq!(all[2].message, "fourth");
    }

    #[test]
    fn clear_empties_buffer() {
        let mut cap = LogCapture::new(100);
        cap.push(make_entry("INFO", "hello"));
        cap.push(make_entry("WARN", "world"));
        assert_eq!(cap.len(), 2);

        cap.clear();
        assert!(cap.is_empty());
        assert_eq!(cap.len(), 0);
    }

    #[test]
    fn log_entry_serde_round_trip() {
        let entry = make_entry("ERROR", "something broke");
        let json = serde_json::to_string(&entry).unwrap();
        let loaded: LogEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.level, "ERROR");
        assert_eq!(loaded.message, "something broke");
        assert_eq!(loaded.module, Some("test".into()));
    }

    #[test]
    fn zero_capacity() {
        let mut cap = LogCapture::new(0);
        // With zero capacity, push should still work without panicking,
        // but nothing is retained.
        cap.push(make_entry("INFO", "dropped"));
        // Pop_front on empty VecDeque returns None, so the entry just isn't added
        // because len (0) >= capacity (0).
        assert_eq!(cap.len(), 0);
    }
}
