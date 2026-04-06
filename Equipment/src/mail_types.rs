use std::collections::HashSet;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A mail recipient identified by their Crown public key.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct MailRecipient {
    /// The recipient's Crown public key identifier.
    pub crown_id: String,
    /// Optional human-readable name for display.
    pub display_name: Option<String>,
}

/// The role a recipient plays on a message.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RecipientRole {
    /// Primary recipient.
    To,
    /// Carbon copy -- visible to all recipients.
    Cc,
    /// Blind carbon copy -- hidden from other recipients.
    Bcc,
}

/// A recipient paired with their role on a message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MailRecipientEntry {
    /// The person receiving the message.
    pub recipient: MailRecipient,
    /// Their role on this message (To, Cc, or Bcc).
    pub role: RecipientRole,
}

/// A mail message — the fundamental unit of asynchronous communication.
///
/// The `body` field holds serialized .idea content as JSON. Equipment treats
/// it as opaque — Ideas is responsible for interpreting it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MailMessage {
    /// Unique message identifier.
    pub id: Uuid,
    /// Sender's Crown ID.
    pub from: String,
    /// Who this message is addressed to (To, Cc, Bcc).
    pub recipients: Vec<MailRecipientEntry>,
    /// Subject line.
    pub subject: String,
    /// Serialized .idea content as JSON (opaque to Equipment).
    pub body: String,
    /// References to .idea attachments.
    pub attachments: Vec<String>,
    /// Thread this message belongs to, if part of a conversation.
    pub thread_id: Option<String>,
    /// The message this is replying to, if any.
    pub in_reply_to: Option<Uuid>,
    /// When this message was sent.
    pub timestamp: DateTime<Utc>,
    /// Whether the owner of this mailbox has read this message.
    pub read: bool,
}

/// A draft message being composed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MailDraft {
    /// Unique draft identifier.
    pub id: Uuid,
    /// Recipients added so far.
    pub recipients: Vec<MailRecipientEntry>,
    /// Draft subject line.
    pub subject: String,
    /// Draft body content.
    pub body: String,
    /// Attached .idea references.
    pub attachments: Vec<String>,
    /// Thread this draft will join when sent.
    pub thread_id: Option<String>,
    /// Message this draft is replying to.
    pub in_reply_to: Option<Uuid>,
    /// When this draft was first created.
    pub created_at: DateTime<Utc>,
    /// When this draft was last modified.
    pub updated_at: DateTime<Utc>,
}

/// A conversation thread grouping related messages.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MailThread {
    /// Unique thread identifier.
    pub thread_id: String,
    /// Subject line from the first message in the thread.
    pub subject: String,
    /// IDs of all messages in this thread, in arrival order.
    pub messages: Vec<Uuid>,
    /// Crown IDs of everyone who has sent or received in this thread.
    pub participants: HashSet<String>,
    /// Timestamp of the most recent message in the thread.
    pub latest_timestamp: DateTime<Utc>,
}

/// Delivery status for a message to a specific recipient.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MailDeliveryStatus {
    /// Message is waiting in the outbound queue.
    Queued,
    /// Message is currently being transmitted.
    Sending,
    /// Message reached the recipient's relay.
    Delivered,
    /// Recipient has opened the message.
    Read,
    /// Delivery failed with the given reason.
    Failed {
        /// Human-readable explanation of the failure.
        reason: String,
    },
}

/// Tracks delivery of a specific message to a specific recipient.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MailDelivery {
    /// The message being tracked.
    pub message_id: Uuid,
    /// Recipient's Crown ID.
    pub recipient: String,
    /// Current delivery status.
    pub status: MailDeliveryStatus,
    /// When this delivery was first queued.
    pub queued_at: DateTime<Utc>,
    /// When the status was last updated.
    pub updated_at: DateTime<Utc>,
}

/// A bulk send operation — one template to many recipients.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BulkSend {
    /// Unique identifier for this bulk operation.
    pub id: Uuid,
    /// Subject line template applied to every message.
    pub template_subject: String,
    /// Body template applied to every message.
    pub template_body: String,
    /// All recipients who will receive this message.
    pub recipients: Vec<MailRecipient>,
    /// When this bulk send was initiated.
    pub created_at: DateTime<Utc>,
}

/// Serializable snapshot of mailbox state (same pattern as PagerState).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MailboxState {
    /// All stored messages.
    pub messages: Vec<MailMessage>,
    /// All in-progress drafts.
    pub drafts: Vec<MailDraft>,
    /// All conversation threads.
    pub threads: Vec<MailThread>,
    /// All delivery tracking records.
    pub deliveries: Vec<MailDelivery>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mail_recipient_serde_round_trip() {
        let r = MailRecipient {
            crown_id: "cpub_alice".into(),
            display_name: Some("Alice".into()),
        };
        let json = serde_json::to_string(&r).unwrap();
        let loaded: MailRecipient = serde_json::from_str(&json).unwrap();
        assert_eq!(r, loaded);
    }

    #[test]
    fn recipient_role_serde() {
        for role in [RecipientRole::To, RecipientRole::Cc, RecipientRole::Bcc] {
            let json = serde_json::to_string(&role).unwrap();
            let loaded: RecipientRole = serde_json::from_str(&json).unwrap();
            assert_eq!(role, loaded);
        }
    }

    #[test]
    fn mail_message_serde_round_trip() {
        let msg = MailMessage {
            id: Uuid::new_v4(),
            from: "cpub_alice".into(),
            recipients: vec![MailRecipientEntry {
                recipient: MailRecipient {
                    crown_id: "cpub_bob".into(),
                    display_name: None,
                },
                role: RecipientRole::To,
            }],
            subject: "Hello".into(),
            body: r#"{"type":"text","content":"Hi Bob"}"#.into(),
            attachments: vec!["idea://doc/123".into()],
            thread_id: Some("thread-001".into()),
            in_reply_to: None,
            timestamp: Utc::now(),
            read: false,
        };

        let json = serde_json::to_string(&msg).unwrap();
        let loaded: MailMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.id, msg.id);
        assert_eq!(loaded.subject, "Hello");
        assert_eq!(loaded.recipients.len(), 1);
    }

    #[test]
    fn mail_draft_serde_round_trip() {
        let now = Utc::now();
        let draft = MailDraft {
            id: Uuid::new_v4(),
            recipients: vec![],
            subject: "Draft".into(),
            body: String::new(),
            attachments: vec![],
            thread_id: None,
            in_reply_to: None,
            created_at: now,
            updated_at: now,
        };

        let json = serde_json::to_string(&draft).unwrap();
        let loaded: MailDraft = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.id, draft.id);
        assert_eq!(loaded.subject, "Draft");
    }

    #[test]
    fn mail_thread_serde_round_trip() {
        let thread = MailThread {
            thread_id: "thread-001".into(),
            subject: "Discussion".into(),
            messages: vec![Uuid::new_v4(), Uuid::new_v4()],
            participants: HashSet::from(["cpub_alice".into(), "cpub_bob".into()]),
            latest_timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&thread).unwrap();
        let loaded: MailThread = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.thread_id, "thread-001");
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.participants.len(), 2);
    }

    #[test]
    fn delivery_status_serde() {
        let statuses = vec![
            MailDeliveryStatus::Queued,
            MailDeliveryStatus::Sending,
            MailDeliveryStatus::Delivered,
            MailDeliveryStatus::Read,
            MailDeliveryStatus::Failed {
                reason: "timeout".into(),
            },
        ];

        for status in statuses {
            let json = serde_json::to_string(&status).unwrap();
            let loaded: MailDeliveryStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(status, loaded);
        }
    }

    #[test]
    fn bulk_send_serde_round_trip() {
        let bulk = BulkSend {
            id: Uuid::new_v4(),
            template_subject: "Announcement".into(),
            template_body: "Hello everyone".into(),
            recipients: vec![MailRecipient {
                crown_id: "cpub_alice".into(),
                display_name: Some("Alice".into()),
            }],
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&bulk).unwrap();
        let loaded: BulkSend = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.id, bulk.id);
        assert_eq!(loaded.template_subject, "Announcement");
    }

    #[test]
    fn mailbox_state_default_is_empty() {
        let state = MailboxState::default();
        assert!(state.messages.is_empty());
        assert!(state.drafts.is_empty());
        assert!(state.threads.is_empty());
        assert!(state.deliveries.is_empty());
    }

    #[test]
    fn mailbox_state_serde_round_trip() {
        let state = MailboxState {
            messages: vec![MailMessage {
                id: Uuid::new_v4(),
                from: "cpub_alice".into(),
                recipients: vec![],
                subject: "Test".into(),
                body: "{}".into(),
                attachments: vec![],
                thread_id: None,
                in_reply_to: None,
                timestamp: Utc::now(),
                read: false,
            }],
            drafts: vec![],
            threads: vec![],
            deliveries: vec![],
        };

        let json = serde_json::to_string(&state).unwrap();
        let loaded: MailboxState = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].subject, "Test");
    }
}
