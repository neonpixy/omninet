pub mod accessibility;
pub mod authority;
pub mod binding;
pub mod bonds;
pub mod coinage;
pub mod commerce;
pub mod crdt;
pub mod digit;
pub mod error;
pub mod form;
pub mod header;
pub mod helpers;
pub mod interactive;
pub mod media;
pub mod package;
pub mod position;
pub mod richtext;
pub mod schema;
pub mod sheet;
pub mod textspan;
pub mod slide;
pub mod validation;

// Re-exports from X
pub use x::Value;
pub use x::VectorClock;

// Re-exports for convenience
pub use digit::Digit;
pub use header::Header;
pub use package::IdeaPackage;
pub use schema::{DigitSchema, SchemaRegistry};

// Domain digit re-exports
pub use accessibility::{AccessibilityMetadata, AccessibilityRole, LiveRegion};
pub use binding::DataBinding;
pub use bonds::BondRelationship;
pub use commerce::{CartItemMeta, OrderMeta, OrderStatus, ProductMeta, ReviewMeta, StorefrontMeta};
pub use form::{
    CheckboxMeta, DropdownMeta, FormMeta, InputFieldMeta, InputType, RadioMeta, SubmitMeta,
    ToggleMeta,
};
pub use interactive::{AccordionMeta, ButtonMeta, ButtonStyle, NavLinkMeta, TabGroupMeta};
pub use richtext::{
    BlockquoteMeta, CalloutMeta, CitationMeta, CodeBlockMeta, FootnoteMeta, HeadingMeta, ListMeta,
    ListStyle, ParagraphMeta,
};
pub use textspan::{TextAttribute, TextSpan};
pub use sheet::{CellAddress, CellMeta, CellRange, CellType, ColumnDef, SheetMeta, ViewMode};
pub use slide::{SlideLayout, SlideMeta, TransitionType};
