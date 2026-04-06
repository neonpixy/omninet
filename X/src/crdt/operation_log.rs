use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use super::traits::CrdtOperation;
use super::vector_clock::VectorClock;

/// An append-only operation log for CRDT operations.
///
/// Generic over any operation type. Operations are stored as newline-delimited
/// JSON in an `operations.log` file. Snapshots capture periodic state.
pub struct OperationLog<T: CrdtOperation> {
    directory: PathBuf,
    operations: Vec<T>,
    vector_clock: VectorClock,
}

impl<T: CrdtOperation> OperationLog<T> {
    /// Creates a new empty log that will persist to the given directory.
    ///
    /// Call [`load`](Self::load) to read any previously-saved operations from disk.
    pub fn new(directory: PathBuf) -> Self {
        Self {
            directory,
            operations: Vec::new(),
            vector_clock: VectorClock::new(),
        }
    }

    /// Appends an operation to the log.
    pub fn append(&mut self, operation: T) -> Result<(), std::io::Error> {
        self.vector_clock.merge(operation.vector());
        self.operations.push(operation);
        self.flush_last()
    }

    /// Returns all operations in the log.
    pub fn all_operations(&self) -> &[T] {
        &self.operations
    }

    /// Returns operations since a given vector clock.
    pub fn operations_since(&self, clock: &VectorClock) -> Vec<&T> {
        self.operations
            .iter()
            .filter(|op| clock.happened_before(op.vector()))
            .collect()
    }

    /// Returns the current merged vector clock.
    pub fn current_clock(&self) -> &VectorClock {
        &self.vector_clock
    }

    /// Number of operations in the log.
    pub fn count(&self) -> usize {
        self.operations.len()
    }

    /// Loads operations from disk.
    pub fn load(&mut self) -> Result<(), std::io::Error> {
        let log_path = self.directory.join("operations.log");
        if !log_path.exists() {
            return Ok(());
        }

        let file = std::fs::File::open(&log_path)?;
        let reader = BufReader::new(file);

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let op: T = serde_json::from_str(&line).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
            })?;
            self.vector_clock.merge(op.vector());
            self.operations.push(op);
        }

        Ok(())
    }

    /// Writes the last operation to disk (append mode).
    fn flush_last(&self) -> Result<(), std::io::Error> {
        std::fs::create_dir_all(&self.directory)?;
        let log_path = self.directory.join("operations.log");

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)?;

        if let Some(op) = self.operations.last() {
            let json = serde_json::to_string(op).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
            })?;
            writeln!(file, "{json}")?;
        }

        Ok(())
    }

    /// Saves a snapshot of arbitrary state alongside the log.
    pub fn save_snapshot<S: serde::Serialize>(
        &self,
        state: &S,
    ) -> Result<PathBuf, std::io::Error> {
        std::fs::create_dir_all(&self.directory)?;
        let path = self.directory.join("snapshot.json");
        let json = serde_json::to_string_pretty(state).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;
        std::fs::write(&path, json)?;
        Ok(path)
    }

    /// Loads a snapshot from disk.
    pub fn load_snapshot<S: serde::de::DeserializeOwned>(
        &self,
    ) -> Result<Option<S>, std::io::Error> {
        let path = self.directory.join("snapshot.json");
        if !path.exists() {
            return Ok(None);
        }
        let data = std::fs::read_to_string(&path)?;
        let state: S = serde_json::from_str(&data).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;
        Ok(Some(state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crdt::traits::CrdtOperation;
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    struct TestOp {
        id: Uuid,
        target: Uuid,
        vector: VectorClock,
        timestamp: DateTime<Utc>,
        author: String,
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

    fn make_op(author: &str) -> TestOp {
        let mut vc = VectorClock::new();
        vc.increment(author);
        TestOp {
            id: Uuid::new_v4(),
            target: Uuid::new_v4(),
            vector: vc,
            timestamp: Utc::now(),
            author: author.into(),
        }
    }

    #[test]
    fn append_and_load() {
        let dir = std::env::temp_dir().join(format!("omnidea_crdt_test_{}", Uuid::new_v4()));
        let _ = std::fs::remove_dir_all(&dir);

        let mut log = OperationLog::new(dir.clone());
        let op1 = make_op("alice");
        let op2 = make_op("bob");
        log.append(op1.clone()).unwrap();
        log.append(op2.clone()).unwrap();
        assert_eq!(log.count(), 2);

        // Load into a new log
        let mut log2 = OperationLog::<TestOp>::new(dir.clone());
        log2.load().unwrap();
        assert_eq!(log2.count(), 2);
        assert_eq!(log2.all_operations()[0].id, op1.id);
        assert_eq!(log2.all_operations()[1].id, op2.id);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn operations_since() {
        let dir = std::env::temp_dir().join(format!("omnidea_crdt_since_{}", Uuid::new_v4()));

        let mut log = OperationLog::new(dir.clone());

        let before_clock = log.current_clock().clone();
        let op1 = make_op("alice");
        log.append(op1).unwrap();

        let after = log.operations_since(&before_clock);
        assert_eq!(after.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn snapshot_round_trip() {
        let dir = std::env::temp_dir().join(format!("omnidea_crdt_snap_{}", Uuid::new_v4()));

        let log = OperationLog::<TestOp>::new(dir.clone());

        let state = vec!["hello".to_string(), "world".to_string()];
        log.save_snapshot(&state).unwrap();

        let loaded: Option<Vec<String>> = log.load_snapshot().unwrap();
        assert_eq!(loaded, Some(state));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
