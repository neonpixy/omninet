//! Equipment -- the inter-program communication protocol.
//!
//! Equipment defines how isolated programs (Tome, Library, Courier, etc.)
//! discover each other and exchange messages at runtime through the orchestrator.
//! All routing is string-based and all data crosses module boundaries as JSON,
//! eliminating import cycles entirely.
//!
//! Five independent actors form the nervous system:
//!
//! - [`Phone`] -- typed request/response RPC
//! - [`Email`] -- fire-and-forget pub/sub broadcasts
//! - [`Contacts`] -- communal module registry with catalog queries
//! - [`Pager`] -- pure notification queue
//! - [`Communicator`] -- real-time session management
//!
//! Plus supporting types for mail ([`Mailbox`]), data bindings
//! ([`BindingManager`]), federation scoping ([`FederationScope`]),
//! and program registration ([`ProgramRegistration`]).

pub mod binding_wire;
pub mod catalog;
pub mod communicator;
pub mod communicator_types;
pub mod contacts;
pub mod email;
pub mod error;
pub mod federation_scope;
pub mod mail;
pub mod mail_types;
pub mod notification;
pub mod pager;
pub mod phone;
pub mod presence;
pub mod program_registry;
pub mod programs;

// Re-exports for convenience.
pub use binding_wire::{BindingManager, DataSubscription, DataUpdate};
pub use catalog::{
    CallDescriptor, ChannelDescriptor, EdgeType, EventDescriptor, MessageEdge, MessageTopology,
    ModuleCatalog,
};
pub use communicator::{Communicator, CommunicatorChannel, CommunicatorSession, SessionStatus};
pub use communicator_types::{
    CommunicatorAnswer, CommunicatorEnd, CommunicatorOffer, Confidentiality, EndReason,
    OfferEncryption, PrivacyRoute, WrappedKey,
};
pub use contacts::{Contacts, ModuleInfo, ModuleType};
pub use federation_scope::FederationScope;
pub use email::{Email, EmailEvent};
pub use error::{CommunicatorError, ContactsError, MailError, PhoneError};
pub use mail::Mailbox;
pub use mail_types::{
    BulkSend, MailDelivery, MailDeliveryStatus, MailDraft, MailMessage, MailRecipient,
    MailRecipientEntry, MailThread, MailboxState, RecipientRole,
};
pub use notification::{Notification, NotificationDelivery, NotificationPriority, PagerState};
pub use pager::Pager;
pub use phone::{Phone, PhoneCall};
pub use presence::{CollaborationSession, CursorPosition, PresenceInfo};
pub use program_registry::ProgramRegistration;
