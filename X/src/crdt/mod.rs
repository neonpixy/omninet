//! Conflict-free replicated data types (CRDTs).
//!
//! Provides a generic CRDT engine with vector clocks, idempotent operation
//! application, merge, conflict detection, and a persistent operation log.
//! Any module can implement [`CrdtOperation`] for its own operation types.

pub mod engine;
pub mod formatting;
pub mod operation_log;
pub mod sequence;
pub mod traits;
pub mod vector_clock;

pub use engine::CrdtEngine;
pub use formatting::{AnchorSide, FormatMap, FormatMark, FormatOp, MarkAction, MarkAnchor, TextSpan};
pub use operation_log::OperationLog;
pub use sequence::{SequenceAtom, SequenceId, SequenceOp, SequenceRga};
pub use traits::CrdtOperation;
pub use vector_clock::{ClockComparison, VectorClock};
