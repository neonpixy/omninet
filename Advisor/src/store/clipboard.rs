use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Working memory — cross-session insights with priority decay.
///
/// Fixed capacity (default 100). When full, the lowest-priority entry is evicted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GlobalClipboard {
    entries: Vec<ClipboardEntry>,
    max_entries: usize,
}

/// A single entry in the global clipboard.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClipboardEntry {
    pub id: Uuid,
    pub content: String,
    pub created_at: DateTime<Utc>,
    /// Sessions this entry relates to
    pub related_session_ids: Vec<Uuid>,
    /// Priority (0.1..=1.0, decays over time)
    pub priority: f64,
}

impl ClipboardEntry {
    pub fn new(content: impl Into<String>, priority: f64) -> Self {
        Self {
            id: Uuid::new_v4(),
            content: content.into(),
            created_at: Utc::now(),
            related_session_ids: Vec::new(),
            priority: priority.clamp(0.1, 1.0),
        }
    }

    pub fn with_session(mut self, session_id: Uuid) -> Self {
        self.related_session_ids.push(session_id);
        self
    }

    /// Apply daily decay to priority.
    pub fn decay(&mut self, daily_rate: f64, min: f64) {
        self.priority = (self.priority - daily_rate).max(min);
    }

    /// Boost the priority.
    pub fn boost(&mut self, amount: f64, max: f64) {
        self.priority = (self.priority + amount).min(max);
    }
}

impl GlobalClipboard {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_entries,
        }
    }

    /// Add an entry. If full, evicts the lowest-priority entry.
    pub fn add(&mut self, entry: ClipboardEntry) {
        if self.entries.len() >= self.max_entries {
            // Find and remove lowest priority
            if let Some(min_idx) = self
                .entries
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    a.priority.partial_cmp(&b.priority).unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
            {
                // Only evict if new entry has higher priority
                if entry.priority > self.entries[min_idx].priority {
                    self.entries.remove(min_idx);
                } else {
                    return; // New entry is lower priority, don't add
                }
            }
        }
        self.entries.push(entry);
    }

    /// Remove an entry by ID.
    pub fn remove(&mut self, id: Uuid) -> bool {
        let before = self.entries.len();
        self.entries.retain(|e| e.id != id);
        self.entries.len() < before
    }

    /// Remove all entries related to a session.
    pub fn remove_related_to(&mut self, session_id: Uuid) {
        self.entries
            .retain(|e| !e.related_session_ids.contains(&session_id));
    }

    /// Get high-priority entries (above threshold).
    pub fn high_priority(&self, threshold: f64) -> Vec<&ClipboardEntry> {
        self.entries.iter().filter(|e| e.priority >= threshold).collect()
    }

    /// Get the N most recent entries.
    pub fn recent(&self, n: usize) -> Vec<&ClipboardEntry> {
        let mut sorted: Vec<&ClipboardEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        sorted.truncate(n);
        sorted
    }

    /// Apply decay to all entries.
    pub fn decay_all(&mut self, daily_rate: f64, min_priority: f64) {
        for entry in &mut self.entries {
            entry.decay(daily_rate, min_priority);
        }
    }

    /// Boost an entry by ID.
    pub fn boost(&mut self, id: Uuid, amount: f64) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.boost(amount, 1.0);
            true
        } else {
            false
        }
    }

    /// Search entries by keyword.
    pub fn search(&self, query: &str) -> Vec<&ClipboardEntry> {
        let query_lower = query.to_lowercase();
        self.entries
            .iter()
            .filter(|e| e.content.to_lowercase().contains(&query_lower))
            .collect()
    }

    /// Get an entry by ID.
    pub fn get(&self, id: Uuid) -> Option<&ClipboardEntry> {
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.entries.len() >= self.max_entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_add_and_get() {
        let mut cb = GlobalClipboard::new(10);
        let entry = ClipboardEntry::new("insight", 0.7);
        let id = entry.id;
        cb.add(entry);
        assert_eq!(cb.count(), 1);
        assert!(cb.get(id).is_some());
    }

    #[test]
    fn clipboard_eviction_on_full() {
        let mut cb = GlobalClipboard::new(2);
        cb.add(ClipboardEntry::new("low", 0.3));
        cb.add(ClipboardEntry::new("medium", 0.5));
        assert!(cb.is_full());

        cb.add(ClipboardEntry::new("high", 0.8));
        assert_eq!(cb.count(), 2);
        // Low priority should have been evicted
        assert!(cb.search("low").is_empty());
        assert!(!cb.search("high").is_empty());
    }

    #[test]
    fn clipboard_rejects_lower_priority_when_full() {
        let mut cb = GlobalClipboard::new(2);
        cb.add(ClipboardEntry::new("a", 0.5));
        cb.add(ClipboardEntry::new("b", 0.6));
        cb.add(ClipboardEntry::new("c", 0.3)); // lower than both, rejected
        assert_eq!(cb.count(), 2);
        assert!(cb.search("c").is_empty());
    }

    #[test]
    fn clipboard_remove() {
        let mut cb = GlobalClipboard::new(10);
        let entry = ClipboardEntry::new("remove me", 0.5);
        let id = entry.id;
        cb.add(entry);
        assert!(cb.remove(id));
        assert!(cb.is_empty());
        assert!(!cb.remove(id)); // already removed
    }

    #[test]
    fn clipboard_decay_all() {
        let mut cb = GlobalClipboard::new(10);
        cb.add(ClipboardEntry::new("a", 0.5));
        cb.add(ClipboardEntry::new("b", 0.3));
        cb.decay_all(0.02, 0.1);
        let entries: Vec<&ClipboardEntry> = cb.search("a");
        assert!((entries[0].priority - 0.48).abs() < f64::EPSILON);
    }

    #[test]
    fn clipboard_boost() {
        let mut cb = GlobalClipboard::new(10);
        let entry = ClipboardEntry::new("boost me", 0.5);
        let id = entry.id;
        cb.add(entry);
        cb.boost(id, 0.3);
        assert!((cb.get(id).unwrap().priority - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn clipboard_search() {
        let mut cb = GlobalClipboard::new(10);
        cb.add(ClipboardEntry::new("Rust is great", 0.5));
        cb.add(ClipboardEntry::new("Swift is also great", 0.5));
        cb.add(ClipboardEntry::new("Python too", 0.5));
        let results = cb.search("great");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn clipboard_high_priority() {
        let mut cb = GlobalClipboard::new(10);
        cb.add(ClipboardEntry::new("low", 0.3));
        cb.add(ClipboardEntry::new("high", 0.9));
        let high = cb.high_priority(0.5);
        assert_eq!(high.len(), 1);
        assert_eq!(high[0].content, "high");
    }

    #[test]
    fn entry_with_session() {
        let session_id = Uuid::new_v4();
        let entry = ClipboardEntry::new("scoped", 0.5).with_session(session_id);
        assert_eq!(entry.related_session_ids, vec![session_id]);
    }

    #[test]
    fn remove_related_to_session() {
        let mut cb = GlobalClipboard::new(10);
        let sid = Uuid::new_v4();
        cb.add(ClipboardEntry::new("related", 0.5).with_session(sid));
        cb.add(ClipboardEntry::new("unrelated", 0.5));
        cb.remove_related_to(sid);
        assert_eq!(cb.count(), 1);
        assert!(!cb.search("unrelated").is_empty());
    }
}
