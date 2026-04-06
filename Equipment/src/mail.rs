use std::collections::{HashMap, HashSet};

use chrono::Utc;
use uuid::Uuid;

use crate::error::MailError;
use crate::mail_types::{
    MailDelivery, MailDeliveryStatus, MailDraft, MailMessage, MailThread, MailboxState,
};

/// User mailbox — stores messages, drafts, threads, and delivery tracking.
///
/// Follows the Pager pattern: plain struct with mutable methods,
/// export/restore for persistence. The caller owns the instance and
/// can wrap it in a Mutex if needed.
pub struct Mailbox {
    messages: HashMap<Uuid, MailMessage>,
    drafts: HashMap<Uuid, MailDraft>,
    threads: HashMap<String, MailThread>,
    deliveries: Vec<MailDelivery>,
    /// Insertion order for messages — newest at the end.
    insertion_order: Vec<Uuid>,
}

impl Mailbox {
    /// Create an empty mailbox with no messages, drafts, or threads.
    pub fn new() -> Self {
        Self {
            messages: HashMap::new(),
            drafts: HashMap::new(),
            threads: HashMap::new(),
            deliveries: Vec::new(),
            insertion_order: Vec::new(),
        }
    }

    // ── Message operations ──────────────────────────────────────────

    /// Store an incoming message and update its thread.
    pub fn receive_message(&mut self, message: MailMessage) {
        let id = message.id;
        self.update_thread_for_message(&message);
        self.insertion_order.push(id);
        self.messages.insert(id, message);
    }

    /// Look up a message by ID.
    pub fn get_message(&self, id: &Uuid) -> Option<&MailMessage> {
        self.messages.get(id)
    }

    /// Mark a message as read. Returns true if the message existed.
    pub fn mark_read(&mut self, id: &Uuid) -> bool {
        if let Some(msg) = self.messages.get_mut(id) {
            msg.read = true;
            true
        } else {
            false
        }
    }

    /// Get inbox messages, newest first. Paginated with limit and offset.
    pub fn inbox(&self, limit: usize, offset: usize) -> Vec<&MailMessage> {
        self.insertion_order
            .iter()
            .rev()
            .filter_map(|id| self.messages.get(id))
            .skip(offset)
            .take(limit)
            .collect()
    }

    /// Get messages sent by a specific crown_id, newest first.
    pub fn sent(&self, from_crown_id: &str, limit: usize, offset: usize) -> Vec<&MailMessage> {
        self.insertion_order
            .iter()
            .rev()
            .filter_map(|id| {
                let msg = self.messages.get(id)?;
                if msg.from == from_crown_id {
                    Some(msg)
                } else {
                    None
                }
            })
            .skip(offset)
            .take(limit)
            .collect()
    }

    /// Count of unread messages.
    pub fn unread_count(&self) -> usize {
        self.messages.values().filter(|m| !m.read).count()
    }

    // ── Draft operations ────────────────────────────────────────────

    /// Create an empty draft with a generated ID. Returns a reference to it.
    pub fn create_draft(&mut self) -> &MailDraft {
        let now = Utc::now();
        let draft = MailDraft {
            id: Uuid::new_v4(),
            recipients: vec![],
            subject: String::new(),
            body: String::new(),
            attachments: vec![],
            thread_id: None,
            in_reply_to: None,
            created_at: now,
            updated_at: now,
        };
        let id = draft.id;
        self.drafts.insert(id, draft);
        self.drafts.get(&id).expect("draft just inserted")
    }

    /// Update an existing draft. Fails if the draft doesn't exist.
    pub fn update_draft(&mut self, draft: MailDraft) -> Result<(), MailError> {
        if !self.drafts.contains_key(&draft.id) {
            return Err(MailError::DraftNotFound(draft.id.to_string()));
        }
        self.drafts.insert(draft.id, draft);
        Ok(())
    }

    /// Delete a draft. Returns true if it existed.
    pub fn delete_draft(&mut self, id: &Uuid) -> bool {
        self.drafts.remove(id).is_some()
    }

    /// Convert a draft into a message. Removes the draft and returns the new message.
    ///
    /// Fails if the draft doesn't exist or has an empty subject and body.
    pub fn send_draft(&mut self, id: &Uuid, from: &str) -> Result<MailMessage, MailError> {
        let draft = self
            .drafts
            .get(id)
            .ok_or_else(|| MailError::DraftNotFound(id.to_string()))?;

        if draft.subject.is_empty() && draft.body.is_empty() {
            return Err(MailError::EmptyDraft);
        }

        let draft = self.drafts.remove(id).expect("draft existence confirmed above");

        let message = MailMessage {
            id: Uuid::new_v4(),
            from: from.to_string(),
            recipients: draft.recipients,
            subject: draft.subject,
            body: draft.body,
            attachments: draft.attachments,
            thread_id: draft.thread_id,
            in_reply_to: draft.in_reply_to,
            timestamp: Utc::now(),
            read: true, // Sender has read their own message.
        };

        self.receive_message(message.clone());

        Ok(message)
    }

    /// List all drafts.
    pub fn drafts(&self) -> Vec<&MailDraft> {
        self.drafts.values().collect()
    }

    // ── Thread operations ───────────────────────────────────────────

    /// Look up a thread by ID.
    pub fn get_thread(&self, thread_id: &str) -> Option<&MailThread> {
        self.threads.get(thread_id)
    }

    /// List all threads, sorted by latest_timestamp descending.
    pub fn list_threads(&self) -> Vec<&MailThread> {
        let mut threads: Vec<&MailThread> = self.threads.values().collect();
        threads.sort_by(|a, b| b.latest_timestamp.cmp(&a.latest_timestamp));
        threads
    }

    // ── Delivery tracking ───────────────────────────────────────────

    /// Record a delivery event.
    pub fn record_delivery(&mut self, delivery: MailDelivery) {
        self.deliveries.push(delivery);
    }

    /// Update delivery status for a specific message/recipient pair.
    pub fn mark_delivery_status(
        &mut self,
        message_id: &Uuid,
        recipient: &str,
        status: MailDeliveryStatus,
    ) {
        for d in &mut self.deliveries {
            if d.message_id == *message_id && d.recipient == recipient {
                d.status = status.clone();
                d.updated_at = Utc::now();
                return;
            }
        }
    }

    /// Get all delivery records for a specific message.
    pub fn delivery_status(&self, message_id: &Uuid) -> Vec<&MailDelivery> {
        self.deliveries
            .iter()
            .filter(|d| d.message_id == *message_id)
            .collect()
    }

    // ── Persistence ─────────────────────────────────────────────────

    /// Export all mailbox state for persistence.
    pub fn export_state(&self) -> MailboxState {
        let messages = self
            .insertion_order
            .iter()
            .filter_map(|id| self.messages.get(id).cloned())
            .collect();

        let drafts = self.drafts.values().cloned().collect();
        let threads = self.threads.values().cloned().collect();
        let deliveries = self.deliveries.clone();

        MailboxState {
            messages,
            drafts,
            threads,
            deliveries,
        }
    }

    /// Restore mailbox state from persistence.
    pub fn restore_state(&mut self, state: MailboxState) {
        self.messages.clear();
        self.drafts.clear();
        self.threads.clear();
        self.deliveries.clear();
        self.insertion_order.clear();

        for msg in state.messages {
            self.insertion_order.push(msg.id);
            self.messages.insert(msg.id, msg);
        }

        for draft in state.drafts {
            self.drafts.insert(draft.id, draft);
        }

        for thread in state.threads {
            self.threads.insert(thread.thread_id.clone(), thread);
        }

        self.deliveries = state.deliveries;
    }

    // ── Stats ───────────────────────────────────────────────────────

    /// Total number of messages.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Total number of drafts.
    pub fn draft_count(&self) -> usize {
        self.drafts.len()
    }

    /// Total number of threads.
    pub fn thread_count(&self) -> usize {
        self.threads.len()
    }

    // ── Internal ────────────────────────────────────────────────────

    /// Update or create a thread for a message.
    fn update_thread_for_message(&mut self, message: &MailMessage) {
        let thread_id = message
            .thread_id
            .clone()
            .unwrap_or_else(|| message.id.to_string());

        let thread = self
            .threads
            .entry(thread_id.clone())
            .or_insert_with(|| MailThread {
                thread_id,
                subject: message.subject.clone(),
                messages: Vec::new(),
                participants: HashSet::new(),
                latest_timestamp: message.timestamp,
            });

        thread.messages.push(message.id);
        thread.participants.insert(message.from.clone());
        for entry in &message.recipients {
            thread.participants.insert(entry.recipient.crown_id.clone());
        }
        if message.timestamp > thread.latest_timestamp {
            thread.latest_timestamp = message.timestamp;
        }
    }
}

impl Default for Mailbox {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mail_types::{MailRecipient, MailRecipientEntry, RecipientRole};
    use chrono::Duration;

    /// Helper to create a simple message for testing.
    fn make_message(from: &str, subject: &str, thread_id: Option<&str>) -> MailMessage {
        MailMessage {
            id: Uuid::new_v4(),
            from: from.into(),
            recipients: vec![MailRecipientEntry {
                recipient: MailRecipient {
                    crown_id: "cpub_bob".into(),
                    display_name: None,
                },
                role: RecipientRole::To,
            }],
            subject: subject.into(),
            body: "{}".into(),
            attachments: vec![],
            thread_id: thread_id.map(String::from),
            in_reply_to: None,
            timestamp: Utc::now(),
            read: false,
        }
    }

    #[test]
    fn create_draft_update_send() {
        let mut mailbox = Mailbox::new();

        // Create draft.
        let draft_id = mailbox.create_draft().id;
        assert_eq!(mailbox.draft_count(), 1);

        // Update draft.
        let mut draft = mailbox.drafts.get(&draft_id).unwrap().clone();
        draft.subject = "Hello".into();
        draft.body = "World".into();
        draft.recipients = vec![MailRecipientEntry {
            recipient: MailRecipient {
                crown_id: "cpub_bob".into(),
                display_name: Some("Bob".into()),
            },
            role: RecipientRole::To,
        }];
        draft.updated_at = Utc::now();
        mailbox.update_draft(draft).unwrap();

        // Send draft.
        let msg = mailbox.send_draft(&draft_id, "cpub_alice").unwrap();
        assert_eq!(msg.subject, "Hello");
        assert_eq!(msg.body, "World");
        assert_eq!(msg.from, "cpub_alice");
        assert!(msg.read); // Sender has read their own message.

        // Draft is gone, message is stored.
        assert_eq!(mailbox.draft_count(), 0);
        assert_eq!(mailbox.message_count(), 1);
    }

    #[test]
    fn receive_message_and_mark_read() {
        let mut mailbox = Mailbox::new();

        let msg = make_message("cpub_alice", "Test", None);
        let msg_id = msg.id;
        mailbox.receive_message(msg);

        assert_eq!(mailbox.unread_count(), 1);
        assert!(!mailbox.get_message(&msg_id).unwrap().read);

        assert!(mailbox.mark_read(&msg_id));
        assert_eq!(mailbox.unread_count(), 0);
        assert!(mailbox.get_message(&msg_id).unwrap().read);

        // Marking nonexistent returns false.
        assert!(!mailbox.mark_read(&Uuid::new_v4()));
    }

    #[test]
    fn thread_grouping() {
        let mut mailbox = Mailbox::new();

        let thread_id = "thread-abc";
        let msg1 = make_message("cpub_alice", "Discussion", Some(thread_id));
        let msg2 = make_message("cpub_bob", "Re: Discussion", Some(thread_id));
        let msg3 = make_message("cpub_charlie", "Re: Re: Discussion", Some(thread_id));

        let id1 = msg1.id;
        let id2 = msg2.id;
        let id3 = msg3.id;

        mailbox.receive_message(msg1);
        mailbox.receive_message(msg2);
        mailbox.receive_message(msg3);

        assert_eq!(mailbox.thread_count(), 1);

        let thread = mailbox.get_thread(thread_id).unwrap();
        assert_eq!(thread.messages.len(), 3);
        assert!(thread.messages.contains(&id1));
        assert!(thread.messages.contains(&id2));
        assert!(thread.messages.contains(&id3));

        // 3 senders + the common recipient "cpub_bob" (who is both a sender in
        // msg2 and a recipient in all messages).
        assert!(thread.participants.contains("cpub_alice"));
        assert!(thread.participants.contains("cpub_bob"));
        assert!(thread.participants.contains("cpub_charlie"));
    }

    #[test]
    fn thread_auto_created_without_thread_id() {
        let mut mailbox = Mailbox::new();

        let msg = make_message("cpub_alice", "Standalone", None);
        let msg_id = msg.id;
        mailbox.receive_message(msg);

        // Thread is auto-created using the message UUID as thread_id.
        assert_eq!(mailbox.thread_count(), 1);
        let thread = mailbox.get_thread(&msg_id.to_string()).unwrap();
        assert_eq!(thread.messages.len(), 1);
    }

    #[test]
    fn inbox_pagination() {
        let mut mailbox = Mailbox::new();

        for i in 0..10 {
            let mut msg = make_message("cpub_alice", &format!("Message {i}"), None);
            // Spread timestamps so ordering is deterministic.
            msg.timestamp = Utc::now() + Duration::seconds(i as i64);
            mailbox.receive_message(msg);
        }

        // First page: 3 messages, offset 0.
        let page1 = mailbox.inbox(3, 0);
        assert_eq!(page1.len(), 3);
        assert_eq!(page1[0].subject, "Message 9"); // Newest first.
        assert_eq!(page1[1].subject, "Message 8");
        assert_eq!(page1[2].subject, "Message 7");

        // Second page: 3 messages, offset 3.
        let page2 = mailbox.inbox(3, 3);
        assert_eq!(page2.len(), 3);
        assert_eq!(page2[0].subject, "Message 6");

        // Last partial page.
        let last = mailbox.inbox(5, 8);
        assert_eq!(last.len(), 2);
    }

    #[test]
    fn sent_messages() {
        let mut mailbox = Mailbox::new();

        mailbox.receive_message(make_message("cpub_alice", "From Alice", None));
        mailbox.receive_message(make_message("cpub_bob", "From Bob", None));
        mailbox.receive_message(make_message("cpub_alice", "From Alice 2", None));

        let alice_sent = mailbox.sent("cpub_alice", 10, 0);
        assert_eq!(alice_sent.len(), 2);

        let bob_sent = mailbox.sent("cpub_bob", 10, 0);
        assert_eq!(bob_sent.len(), 1);
    }

    #[test]
    fn delivery_tracking_lifecycle() {
        let mut mailbox = Mailbox::new();

        let msg_id = Uuid::new_v4();
        let now = Utc::now();

        // Record initial delivery.
        mailbox.record_delivery(MailDelivery {
            message_id: msg_id,
            recipient: "cpub_bob".into(),
            status: MailDeliveryStatus::Queued,
            queued_at: now,
            updated_at: now,
        });

        // Check status.
        let statuses = mailbox.delivery_status(&msg_id);
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].status, MailDeliveryStatus::Queued);

        // Update to Sending.
        mailbox.mark_delivery_status(&msg_id, "cpub_bob", MailDeliveryStatus::Sending);
        let statuses = mailbox.delivery_status(&msg_id);
        assert_eq!(statuses[0].status, MailDeliveryStatus::Sending);

        // Update to Delivered.
        mailbox.mark_delivery_status(&msg_id, "cpub_bob", MailDeliveryStatus::Delivered);
        let statuses = mailbox.delivery_status(&msg_id);
        assert_eq!(statuses[0].status, MailDeliveryStatus::Delivered);

        // Update to Read.
        mailbox.mark_delivery_status(&msg_id, "cpub_bob", MailDeliveryStatus::Read);
        let statuses = mailbox.delivery_status(&msg_id);
        assert_eq!(statuses[0].status, MailDeliveryStatus::Read);
    }

    #[test]
    fn delivery_failure() {
        let mut mailbox = Mailbox::new();

        let msg_id = Uuid::new_v4();
        let now = Utc::now();

        mailbox.record_delivery(MailDelivery {
            message_id: msg_id,
            recipient: "cpub_bob".into(),
            status: MailDeliveryStatus::Queued,
            queued_at: now,
            updated_at: now,
        });

        mailbox.mark_delivery_status(
            &msg_id,
            "cpub_bob",
            MailDeliveryStatus::Failed {
                reason: "relay offline".into(),
            },
        );

        let statuses = mailbox.delivery_status(&msg_id);
        assert_eq!(
            statuses[0].status,
            MailDeliveryStatus::Failed {
                reason: "relay offline".into()
            }
        );
    }

    #[test]
    fn export_restore_round_trip() {
        let mut mailbox = Mailbox::new();

        // Add a message.
        let msg = make_message("cpub_alice", "Persisted", Some("thread-persist"));
        let msg_id = msg.id;
        mailbox.receive_message(msg);
        mailbox.mark_read(&msg_id);

        // Add a draft.
        let draft_id = mailbox.create_draft().id;

        // Add a delivery.
        let now = Utc::now();
        mailbox.record_delivery(MailDelivery {
            message_id: msg_id,
            recipient: "cpub_bob".into(),
            status: MailDeliveryStatus::Delivered,
            queued_at: now,
            updated_at: now,
        });

        // Export.
        let state = mailbox.export_state();
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.drafts.len(), 1);
        assert_eq!(state.threads.len(), 1);
        assert_eq!(state.deliveries.len(), 1);

        // Verify serde round-trip.
        let json = serde_json::to_string(&state).unwrap();
        let restored_state: MailboxState = serde_json::from_str(&json).unwrap();

        // Restore into a fresh mailbox.
        let mut mailbox2 = Mailbox::new();
        mailbox2.restore_state(restored_state);

        assert_eq!(mailbox2.message_count(), 1);
        assert_eq!(mailbox2.draft_count(), 1);
        assert_eq!(mailbox2.thread_count(), 1);

        let restored_msg = mailbox2.get_message(&msg_id).unwrap();
        assert_eq!(restored_msg.subject, "Persisted");
        assert!(restored_msg.read);

        assert!(mailbox2.drafts.contains_key(&draft_id));
        assert!(mailbox2.get_thread("thread-persist").is_some());

        let restored_deliveries = mailbox2.delivery_status(&msg_id);
        assert_eq!(restored_deliveries.len(), 1);
        assert_eq!(
            restored_deliveries[0].status,
            MailDeliveryStatus::Delivered
        );
    }

    #[test]
    fn send_empty_draft_fails() {
        let mut mailbox = Mailbox::new();
        let draft_id = mailbox.create_draft().id;

        let result = mailbox.send_draft(&draft_id, "cpub_alice");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MailError::EmptyDraft));
        // Draft should still exist since send failed.
        assert_eq!(mailbox.draft_count(), 1);
    }

    #[test]
    fn send_nonexistent_draft_fails() {
        let mut mailbox = Mailbox::new();
        let result = mailbox.send_draft(&Uuid::new_v4(), "cpub_alice");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MailError::DraftNotFound(_)));
    }

    #[test]
    fn update_nonexistent_draft_fails() {
        let mut mailbox = Mailbox::new();
        let draft = MailDraft {
            id: Uuid::new_v4(),
            recipients: vec![],
            subject: "Phantom".into(),
            body: String::new(),
            attachments: vec![],
            thread_id: None,
            in_reply_to: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let result = mailbox.update_draft(draft);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), MailError::DraftNotFound(_)));
    }

    #[test]
    fn delete_draft() {
        let mut mailbox = Mailbox::new();
        let draft_id = mailbox.create_draft().id;
        assert_eq!(mailbox.draft_count(), 1);

        assert!(mailbox.delete_draft(&draft_id));
        assert_eq!(mailbox.draft_count(), 0);

        // Deleting again returns false.
        assert!(!mailbox.delete_draft(&draft_id));
    }

    #[test]
    fn list_threads_sorted_by_latest() {
        let mut mailbox = Mailbox::new();

        let mut msg1 = make_message("cpub_alice", "Old thread", Some("thread-old"));
        msg1.timestamp = Utc::now() - Duration::hours(2);

        let mut msg2 = make_message("cpub_alice", "New thread", Some("thread-new"));
        msg2.timestamp = Utc::now();

        let mut msg3 = make_message("cpub_alice", "Mid thread", Some("thread-mid"));
        msg3.timestamp = Utc::now() - Duration::hours(1);

        mailbox.receive_message(msg1);
        mailbox.receive_message(msg2);
        mailbox.receive_message(msg3);

        let threads = mailbox.list_threads();
        assert_eq!(threads.len(), 3);
        assert_eq!(threads[0].thread_id, "thread-new");
        assert_eq!(threads[1].thread_id, "thread-mid");
        assert_eq!(threads[2].thread_id, "thread-old");
    }

    #[test]
    fn stats() {
        let mut mailbox = Mailbox::new();
        assert_eq!(mailbox.message_count(), 0);
        assert_eq!(mailbox.draft_count(), 0);
        assert_eq!(mailbox.thread_count(), 0);

        mailbox.receive_message(make_message("cpub_alice", "M1", None));
        mailbox.receive_message(make_message("cpub_alice", "M2", None));
        mailbox.create_draft();

        assert_eq!(mailbox.message_count(), 2);
        assert_eq!(mailbox.draft_count(), 1);
        assert_eq!(mailbox.thread_count(), 2); // Each message without thread_id gets its own.
    }
}
