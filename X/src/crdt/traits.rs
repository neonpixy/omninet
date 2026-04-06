use chrono::{DateTime, Utc};
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

use super::vector_clock::VectorClock;

/// A generic CRDT operation that any module can implement.
///
/// Ideas implements this for `DigitOperation`.
/// Kingdom, Fortune, Crown, Globe, etc. will implement it for their own
/// operation types when those modules are built.
pub trait CrdtOperation: Clone + Serialize + DeserializeOwned + Send + Sync {
    /// Unique ID for this operation (for idempotency).
    fn id(&self) -> Uuid;

    /// The ID of the target entity this operation applies to.
    fn target_id(&self) -> Uuid;

    /// The vector clock at the time of this operation.
    fn vector(&self) -> &VectorClock;

    /// When this operation was created.
    fn timestamp(&self) -> DateTime<Utc>;

    /// Who created this operation (typically an crown_id).
    fn author(&self) -> &str;
}
