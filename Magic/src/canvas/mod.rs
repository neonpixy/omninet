//! Canvas interaction layer — viewport, zoom, selection, snapping,
//! coordinate transforms, and drag handles.
//!
//! The canvas module provides the spatial infrastructure that sits between
//! the tool system (which generates actions) and the rendering system
//! (which draws the document). `CanvasState` is the central hub.
//!
//! # Architecture
//!
//! ```text
//! Tool events ──→ Tool (tool module) ──→ ToolAction
//!                                            │
//!                                            ▼
//!                                    CanvasState (canvas module)
//!                                       │         │
//!                                       ▼         ▼
//!                              DocumentState   Viewport/Zoom
//!                              (ideation)      coordinate transforms
//! ```

mod state;
mod types;

pub use state::CanvasState;
pub use types::{
    compute_handles, hit_test_handles, DragHandle, HandlePosition,
};
