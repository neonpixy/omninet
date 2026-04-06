//! Protocol bridges -- translate Equipment mail messages to/from external protocols.
//!
//! Each submodule implements the `ProtocolBridge` trait for a specific
//! external protocol (SMTP, ActivityPub, RSS, etc.).

pub mod smtp;

pub use smtp::SmtpBridge;
