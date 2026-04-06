//! # Formula Engine
//!
//! A spreadsheet formula parser, evaluator, and dependency tracker for
//! Omnidea's Abacus program (and any other context that needs formulas).
//!
//! ## Architecture
//!
//! ```text
//! Input ("=SUM(A1:A10)")
//!     │
//!     ▼
//! FormulaTokenizer  (token.rs)   → Vec<(FormulaToken, Span)>
//!     │
//!     ▼
//! FormulaParser     (parser.rs)  → FormulaNode (AST)
//!     │
//!     ▼
//! FormulaEvaluator  (evaluator.rs) + CellResolver → FormulaValue
//! ```
//!
//! ## Key Design Decisions
//!
//! - **Independent of Ideas.** Uses its own `FormulaCellRef` (not Ideas' CellAddress)
//!   and `FormulaValue` (not x::Value). This keeps Lingo dependency-free from Ideas.
//! - **Hand-rolled recursive descent parser** with Pratt-style operator precedence.
//!   No parser combinator libraries.
//! - **CellResolver trait** lets the spreadsheet layer plug in cell data without
//!   Lingo knowing about the data model.
//! - **Locale-aware.** Function names translate (SUM→SOMME in French). Formulas
//!   stored in canonical English, displayed in the user's language.

pub mod ast;
pub mod dependency;
pub mod error;
pub mod evaluator;
pub mod functions;
pub mod locale;
pub mod parser;
pub mod token;
pub mod value;

// Re-export key types for convenient access.
pub use ast::{BinaryOp, FormulaCellRef, FormulaNode, UnaryOp};
pub use dependency::DependencyGraph;
pub use error::{FormulaError, Span};
pub use evaluator::{CellResolver, FormulaEvaluator};
pub use functions::FunctionRegistry;
pub use locale::FormulaLocale;
pub use parser::FormulaParser;
pub use token::{FormulaToken, FormulaTokenizer};
pub use value::{FormulaErrorKind, FormulaValue};
