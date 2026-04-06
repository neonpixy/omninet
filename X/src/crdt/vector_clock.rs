use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A vector clock for tracking causality across distributed edits.
///
/// Keys are truncated author IDs (first 8 chars after `cpub1` prefix).
/// Values are operation counts for that author.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VectorClock {
    entries: HashMap<String, u64>,
}

/// Result of comparing two vector clocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockComparison {
    /// Both clocks have identical entries -- same causal history.
    Equal,
    /// This clock happened strictly before the other (the other is a causal descendant).
    Before,
    /// This clock happened strictly after the other (this is a causal descendant).
    After,
    /// Neither clock dominates -- the operations diverged and may conflict.
    Concurrent,
}

impl VectorClock {
    /// Creates an empty vector clock with no entries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the count for an author (0 if not present).
    pub fn count_for(&self, author: &str) -> u64 {
        let key = Self::truncate_author(author);
        *self.entries.get(&key).unwrap_or(&0)
    }

    /// Increments the count for an author.
    pub fn increment(&mut self, author: &str) {
        let key = Self::truncate_author(author);
        *self.entries.entry(key).or_insert(0) += 1;
    }

    /// Merges another clock into this one, taking the max of each entry.
    pub fn merge(&mut self, other: &VectorClock) {
        for (key, &value) in &other.entries {
            let entry = self.entries.entry(key.clone()).or_insert(0);
            *entry = (*entry).max(value);
        }
    }

    /// Returns a new clock merged with another.
    pub fn merged(&self, other: &VectorClock) -> VectorClock {
        let mut result = self.clone();
        result.merge(other);
        result
    }

    /// Compares this clock to another.
    pub fn compare(&self, other: &VectorClock) -> ClockComparison {
        let all_keys: std::collections::HashSet<&String> =
            self.entries.keys().chain(other.entries.keys()).collect();

        let mut self_has_greater = false;
        let mut other_has_greater = false;

        for key in all_keys {
            let self_val = self.entries.get(key).copied().unwrap_or(0);
            let other_val = other.entries.get(key).copied().unwrap_or(0);

            if self_val > other_val {
                self_has_greater = true;
            } else if other_val > self_val {
                other_has_greater = true;
            }
        }

        match (self_has_greater, other_has_greater) {
            (false, false) => ClockComparison::Equal,
            (true, false) => ClockComparison::After,
            (false, true) => ClockComparison::Before,
            (true, true) => ClockComparison::Concurrent,
        }
    }

    /// Returns true if this clock happened strictly before another.
    pub fn happened_before(&self, other: &VectorClock) -> bool {
        self.compare(other) == ClockComparison::Before
    }

    /// Returns true if this clock is concurrent with another (conflict).
    pub fn is_concurrent(&self, other: &VectorClock) -> bool {
        self.compare(other) == ClockComparison::Concurrent
    }

    /// Whether this clock has any entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Truncates author ID to 8 chars (strips cpub1 prefix if present).
    fn truncate_author(author: &str) -> String {
        let stripped = if let Some(rest) = author.strip_prefix("cpub1") {
            rest
        } else {
            author
        };
        stripped.chars().take(8).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_clock_is_empty() {
        let clock = VectorClock::new();
        assert!(clock.is_empty());
        assert_eq!(clock.count_for("anyone"), 0);
    }

    #[test]
    fn increment() {
        let mut clock = VectorClock::new();
        clock.increment("alice");
        assert_eq!(clock.count_for("alice"), 1);
        clock.increment("alice");
        assert_eq!(clock.count_for("alice"), 2);
        clock.increment("bob");
        assert_eq!(clock.count_for("bob"), 1);
    }

    #[test]
    fn merge_takes_max() {
        let mut a = VectorClock::new();
        a.increment("alice");
        a.increment("alice");

        let mut b = VectorClock::new();
        b.increment("alice");
        b.increment("bob");
        b.increment("bob");
        b.increment("bob");

        a.merge(&b);
        assert_eq!(a.count_for("alice"), 2); // max(2, 1)
        assert_eq!(a.count_for("bob"), 3); // max(0, 3)
    }

    #[test]
    fn compare_equal() {
        let mut a = VectorClock::new();
        a.increment("alice");
        let b = a.clone();
        assert_eq!(a.compare(&b), ClockComparison::Equal);
    }

    #[test]
    fn compare_before_after() {
        let mut a = VectorClock::new();
        a.increment("alice");

        let mut b = a.clone();
        b.increment("alice");

        assert_eq!(a.compare(&b), ClockComparison::Before);
        assert_eq!(b.compare(&a), ClockComparison::After);
        assert!(a.happened_before(&b));
    }

    #[test]
    fn compare_concurrent() {
        let mut a = VectorClock::new();
        a.increment("alice");

        let mut b = VectorClock::new();
        b.increment("bob");

        assert_eq!(a.compare(&b), ClockComparison::Concurrent);
        assert!(a.is_concurrent(&b));
    }

    #[test]
    fn truncate_author_crown_id_prefix() {
        let mut clock = VectorClock::new();
        clock.increment("cpub1abcdefghijklmnop");
        // Should truncate to first 8 chars after cpub1: "abcdefgh"
        assert_eq!(clock.count_for("cpub1abcdefghijklmnop"), 1);
        // Same truncated key
        assert_eq!(clock.count_for("cpub1abcdefghXXXXXXX"), 1);
    }

    #[test]
    fn serde_round_trip() {
        let mut clock = VectorClock::new();
        clock.increment("alice");
        clock.increment("bob");
        clock.increment("bob");

        let json = serde_json::to_string(&clock).unwrap();
        let rt: VectorClock = serde_json::from_str(&json).unwrap();
        assert_eq!(clock, rt);
    }
}
