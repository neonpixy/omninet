//! X -- shared utilities for the Omninet protocol.
//!
//! The common ground. Polymorphic values, CRDT infrastructure, geometry and
//! color primitives, geographic math, image placeholders, and pure math
//! functions. Zero UI dependencies, zero business logic. If three or more
//! letters need it, it lives here.

pub mod blurhash;
pub mod color;
pub mod crdt;
pub mod geo;
pub mod geometry;
pub mod math;
pub mod value;

pub use color::{BlendMode, Color, ColorError};
pub use crdt::vector_clock::{ClockComparison, VectorClock};
pub use crdt::{
    AnchorSide, CrdtEngine, CrdtOperation, FormatMap, FormatMark, FormatOp, MarkAction,
    MarkAnchor, OperationLog, SequenceAtom, SequenceId, SequenceOp, SequenceRga, TextSpan,
};
pub use geo::{GeoCoordinate, GeoError, point_in_polygon, polygon_area};
pub use geometry::{Anchor, EdgeInsets, Matrix3, Point, Rect, Size, Transform, Vector2};
pub use value::Value;
