//! # Magic — Rendering & Code Projection
//!
//! The arcane arts. Magic has five subsystems:
//!
//! - **Ideation** — Single source of truth for document state. Digits, selection,
//!   layout, vector clock. All mutations via `apply(DigitOperation)`.
//! - **Initiation** — Action dispatch. User actions become invertible Actions.
//!   DocumentHistory provides undo/redo.
//! - **Imagination** — Rendering infrastructure. DigitRenderer trait + registry +
//!   LRU cache. Platform-specific rendering happens outside this crate.
//! - **Projection** — Live code output. CodeProjection trait + CodeBuilder +
//!   ProjectionContext + NameResolver.
//! - **Tool** — Canvas tool system. Tool trait, core tools (Select, Pen, Shape,
//!   Text, Hand, Zoom), and extensible ToolRegistry.
//! - **Canvas** — Canvas interaction layer. CanvasState (viewport, zoom, pan,
//!   selection, snapping), coordinate transforms, and drag handles.
//!
//! ## Covenant Alignment
//!
//! **Sovereignty** — code is transparent and editable.
//! **Dignity** — every content type gets rendered (FallbackRenderer).
//! **Consent** — renderers, projections, and tools are registered plugins.

pub mod canvas;
pub mod error;
pub mod ideation;
pub mod imagination;
pub mod initiation;
pub mod projection;
pub mod tool;

pub use error::MagicError;

// Ideation
pub use ideation::{
    DigitCategory, DigitTypeDefinition, DigitTypeRegistry, DocumentLayout, DocumentState,
    LayoutMode, PresenterView, SelectionState, SlideSequenceState, SlideTransition, TextSelection,
    TransitionDirection, TransitionEffect,
};

// Initiation
pub use initiation::{Action, ActionHandler, ActionRegistry, DocumentHistory, HistoryEntry};

// Imagination
pub use imagination::{
    register_all_renderers, AccessibilityRole, AccessibilitySpec, AccessibilityTrait, ColorScheme,
    CustomAccessibilityAction, DigitRenderer, FallbackRenderer, LiveRegion, RenderCache,
    RenderContext, RenderMode, RenderSpec, RendererRegistry,
};

// Projection
pub use projection::{
    CodeBuilder, CodeProjection, FileContents, GeneratedFile, HtmlProjection, NameResolver,
    ProjectionContext, ReactProjection, SwiftUIProjection,
};

// Tool
pub use tool::{
    CursorStyle, DragState, HandTool, ModifierKeys, PathPoint, PenTool, SelectTool, ShapeKind,
    ShapeTool, TextTool, Tool, ToolAction, ToolRegistry, ZoomTool, default_tool_registry,
};

// Canvas
pub use canvas::{
    CanvasState, DragHandle, HandlePosition, compute_handles, hit_test_handles,
};
