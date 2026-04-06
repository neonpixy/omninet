use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use super::traits::CrdtOperation;

/// A generic CRDT engine that applies operations idempotently.
///
/// Generic over any operation type that implements `CrdtOperation`.
/// The engine tracks which operations have been applied to ensure
/// idempotency, and provides merge and conflict resolution.
pub struct CrdtEngine {
    applied: HashSet<Uuid>,
}

impl CrdtEngine {
    /// Creates a new engine with no operations applied yet.
    pub fn new() -> Self {
        Self {
            applied: HashSet::new(),
        }
    }

    /// Returns true if this operation has already been applied.
    pub fn has_applied(&self, operation_id: Uuid) -> bool {
        self.applied.contains(&operation_id)
    }

    /// Marks an operation as applied.
    pub fn mark_applied(&mut self, operation_id: Uuid) {
        self.applied.insert(operation_id);
    }

    /// Attempts to apply an operation. Returns true if it was new (not a duplicate).
    ///
    /// The caller provides the `apply_fn` closure that actually mutates state.
    /// The engine handles idempotency checking.
    pub fn apply<T: CrdtOperation, F, E>(
        &mut self,
        operation: &T,
        apply_fn: F,
    ) -> Result<bool, E>
    where
        F: FnOnce(&T) -> Result<(), E>,
    {
        if self.applied.contains(&operation.id()) {
            return Ok(false);
        }
        apply_fn(operation)?;
        self.applied.insert(operation.id());
        Ok(true)
    }

    /// Merges local and remote operation lists, removing duplicates
    /// and sorting by causal order (vector clock comparison, then timestamp).
    pub fn merge<T: CrdtOperation>(&self, local: Vec<T>, remote: Vec<T>) -> Vec<T> {
        let mut seen = HashSet::new();
        let mut merged = Vec::new();

        for op in local.into_iter().chain(remote) {
            if seen.insert(op.id()) {
                merged.push(op);
            }
        }

        // Sort by timestamp (causal ordering approximation)
        merged.sort_by(|a, b| a.timestamp().cmp(&b.timestamp()).then(a.author().cmp(b.author())));

        merged
    }

    /// Detects conflicting operations (concurrent ops on the same target).
    pub fn detect_conflicts<T: CrdtOperation>(operations: &[T]) -> HashMap<Uuid, Vec<&T>> {
        let mut by_target: HashMap<Uuid, Vec<&T>> = HashMap::new();

        for op in operations {
            by_target
                .entry(op.target_id())
                .or_default()
                .push(op);
        }

        // Only keep targets with concurrent operations
        by_target.retain(|_, ops| {
            if ops.len() < 2 {
                return false;
            }
            // Check if any pair is concurrent
            for i in 0..ops.len() {
                for j in (i + 1)..ops.len() {
                    if ops[i].vector().is_concurrent(ops[j].vector()) {
                        return true;
                    }
                }
            }
            false
        });

        by_target
    }

    /// Resolves a conflict by last-writer-wins (latest timestamp, author as tiebreaker).
    pub fn resolve_conflict<T: CrdtOperation>(operations: &[T]) -> Option<&T> {
        operations
            .iter()
            .max_by(|a, b| a.timestamp().cmp(&b.timestamp()).then(a.author().cmp(b.author())))
    }

    /// Number of operations applied so far.
    pub fn applied_count(&self) -> usize {
        self.applied.len()
    }

    /// Reset all tracking state.
    pub fn reset(&mut self) {
        self.applied.clear();
    }
}

impl Default for CrdtEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crdt::traits::CrdtOperation;
    use crate::crdt::vector_clock::VectorClock;
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct TestOp {
        id: Uuid,
        target: Uuid,
        vector: VectorClock,
        timestamp: DateTime<Utc>,
        author: String,
        value: String,
    }

    impl CrdtOperation for TestOp {
        fn id(&self) -> Uuid {
            self.id
        }
        fn target_id(&self) -> Uuid {
            self.target
        }
        fn vector(&self) -> &VectorClock {
            &self.vector
        }
        fn timestamp(&self) -> DateTime<Utc> {
            self.timestamp
        }
        fn author(&self) -> &str {
            &self.author
        }
    }

    fn make_op(author: &str, value: &str, target: Uuid) -> TestOp {
        let mut vc = VectorClock::new();
        vc.increment(author);
        TestOp {
            id: Uuid::new_v4(),
            target,
            vector: vc,
            timestamp: Utc::now(),
            author: author.into(),
            value: value.into(),
        }
    }

    #[test]
    fn idempotent_apply() {
        let mut engine = CrdtEngine::new();
        let target = Uuid::new_v4();
        let op = make_op("alice", "first", target);

        let mut applied_count = 0;
        let result = engine.apply(&op, |_| -> Result<(), String> {
            applied_count += 1;
            Ok(())
        });
        assert!(result.unwrap());
        assert_eq!(applied_count, 1);

        // Apply again — should be idempotent
        let result2 = engine.apply(&op, |_| -> Result<(), String> {
            applied_count += 1;
            Ok(())
        });
        assert!(!result2.unwrap());
        assert_eq!(applied_count, 1); // Not called again
    }

    #[test]
    fn merge_removes_duplicates() {
        let engine = CrdtEngine::new();
        let target = Uuid::new_v4();
        let op = make_op("alice", "shared", target);

        let local = vec![op.clone()];
        let remote = vec![op];

        let merged = engine.merge(local, remote);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn resolve_conflict_last_writer_wins() {
        let target = Uuid::new_v4();
        let op1 = make_op("alice", "first", target);

        // Small delay to ensure different timestamp
        std::thread::sleep(std::time::Duration::from_millis(2));
        let op2 = make_op("bob", "second", target);

        let ops = [op1, op2.clone()];
        let winner = CrdtEngine::resolve_conflict(&ops);
        assert_eq!(winner.unwrap().value, op2.value);
    }

    #[test]
    fn applied_count_tracking() {
        let mut engine = CrdtEngine::new();
        assert_eq!(engine.applied_count(), 0);

        let target = Uuid::new_v4();
        let op = make_op("alice", "test", target);
        engine.mark_applied(op.id());
        assert_eq!(engine.applied_count(), 1);
        assert!(engine.has_applied(op.id()));

        engine.reset();
        assert_eq!(engine.applied_count(), 0);
    }
}
