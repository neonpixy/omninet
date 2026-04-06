//! Hall — File I/O for encrypted .idea packages.
//!
//! The great library. Hall reads and writes `.idea` packages to and
//! from disk with AES-256-GCM encryption via Sentinal.
//!
//! # Architecture
//!
//! - **Scribe** (`hall::scribe`) — Write encrypted .idea directories
//! - **Scholar** (`hall::scholar`) — Read with graceful degradation
//! - **Archivist** (`hall::archivist`) — Binary asset pipeline
//!
//! # Key Management
//!
//! Hall takes raw `&[u8]` keys (32 bytes). It does NOT depend on Vault.
//! Vault derives keys via Sentinal and passes them to Hall.

pub mod archivist;
pub mod error;
pub mod media_utils;
pub mod scholar;
pub mod scribe;

pub use error::{HallError, HallWarning, ReadResult};
pub use media_utils::{extract_image_metadata, ImageMetadata};
