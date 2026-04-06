//! Standard identifiers for inter-program communication in Throne.
//!
//! These constants define the well-known call IDs, event IDs, and channel IDs
//! that Throne's seven programs use to communicate through Equipment's Phone,
//! Email, and Communicator primitives. Using constants instead of raw strings
//! prevents typos and makes the communication graph greppable.
//!
//! # Convention
//!
//! All IDs follow the `"program.action"` or `"program.eventName"` pattern.
//! General/cross-cutting IDs use a domain prefix (e.g., `"content."`,
//! `"collaboration."`).

/// Standard call IDs for inter-program Phone RPC.
///
/// Each constant maps to a `PhoneCall::CALL_ID` that a specific program handles.
/// Callers use these to make typed requests through `Phone::call_raw`.
pub mod call_ids {
    // ── Studio ──────────────────────────────────────────────────────

    /// Create a new frame on the Studio canvas.
    pub const STUDIO_CREATE_FRAME: &str = "studio.createFrame";

    /// Set the fill of a design element.
    pub const STUDIO_SET_FILL: &str = "studio.setFill";

    /// Export a design to a target format via Nexus.
    pub const STUDIO_EXPORT: &str = "studio.export";

    // ── Abacus ──────────────────────────────────────────────────────

    /// Get rows from a sheet, optionally filtered.
    pub const ABACUS_GET_ROWS: &str = "abacus.getRows";

    /// Set the value of a specific cell.
    pub const ABACUS_SET_CELL: &str = "abacus.setCell";

    /// Create a new view (grid, kanban, calendar, gallery) on a sheet.
    pub const ABACUS_CREATE_VIEW: &str = "abacus.createView";

    // ── Quill ───────────────────────────────────────────────────────

    /// Get the content of a Quill document as serialized .idea blocks.
    pub const QUILL_GET_CONTENT: &str = "quill.getContent";

    /// Insert a block into a Quill document at a specified position.
    pub const QUILL_INSERT_BLOCK: &str = "quill.insertBlock";

    // ── Library ─────────────────────────────────────────────────────

    /// Publish an .idea file through the Library publishing workflow.
    pub const LIBRARY_PUBLISH: &str = "library.publish";

    /// List .idea files, optionally filtered by tags or folders.
    pub const LIBRARY_LIST_IDEAS: &str = "library.listIdeas";

    /// Apply a tag to one or more .idea files.
    pub const LIBRARY_TAG: &str = "library.tag";

    // ── Courier ─────────────────────────────────────────────────────

    /// Send a composed message via Courier.
    pub const COURIER_SEND: &str = "courier.send";

    /// Open the Courier compose UI with pre-filled fields.
    pub const COURIER_COMPOSE: &str = "courier.compose";

    // ── Podium ──────────────────────────────────────────────────────

    /// Add a slide to a Podium presentation.
    pub const PODIUM_ADD_SLIDE: &str = "podium.addSlide";

    /// Set the transition type between two slides.
    pub const PODIUM_SET_TRANSITION: &str = "podium.setTransition";

    // ── Tome ────────────────────────────────────────────────────────

    /// Create a new note in Tome.
    pub const TOME_CREATE_NOTE: &str = "tome.createNote";

    /// Search notes by content or tags.
    pub const TOME_SEARCH: &str = "tome.search";
}

/// Standard event IDs for inter-program Email pub/sub.
///
/// Each constant maps to an `EmailEvent::EMAIL_ID` that programs emit or
/// subscribe to. Events are fire-and-forget broadcasts.
pub mod event_ids {
    // ── Abacus events ───────────────────────────────────────────────

    /// A cell or range of cells changed value (includes row, column, old/new).
    pub const ABACUS_DATA_CHANGED: &str = "abacus.dataChanged";

    /// A new row was added to a sheet.
    pub const ABACUS_ROW_ADDED: &str = "abacus.rowAdded";

    /// A row was deleted from a sheet.
    pub const ABACUS_ROW_DELETED: &str = "abacus.rowDeleted";

    // ── Library events ──────────────────────────────────────────────

    /// An .idea file was published through Library.
    pub const LIBRARY_PUBLISHED: &str = "library.published";

    /// A published .idea file was updated.
    pub const LIBRARY_UPDATED: &str = "library.updated";

    // ── Studio events ───────────────────────────────────────────────

    /// The selection on the Studio canvas changed.
    pub const STUDIO_SELECTION_CHANGED: &str = "studio.selectionChanged";

    /// A design element was modified on the canvas.
    pub const STUDIO_DESIGN_UPDATED: &str = "studio.designUpdated";

    // ── Courier events ──────────────────────────────────────────────

    /// A message was sent via Courier.
    pub const COURIER_SENT: &str = "courier.sent";

    /// A new message was received by Courier.
    pub const COURIER_RECEIVED: &str = "courier.received";

    // ── General / cross-cutting ─────────────────────────────────────

    /// Content was saved (any program). Payload includes the program and .idea ref.
    pub const CONTENT_SAVED: &str = "content.saved";

    /// Content was modified but not yet saved. Useful for dirty-state indicators.
    pub const CONTENT_MODIFIED: &str = "content.modified";
}

/// Standard channel IDs for Communicator real-time sessions.
///
/// Each constant maps to a `CommunicatorChannel::CHANNEL_ID` used for
/// real-time collaboration between participants.
pub mod channel_ids {
    /// Real-time collaborative editing of .idea content (CRDT operations).
    pub const COLLABORATION_EDIT: &str = "collaboration.edit";

    /// Live cursor/selection positions for collaborators.
    pub const COLLABORATION_CURSOR: &str = "collaboration.cursor";

    /// Presence awareness (who is viewing, idle, active).
    pub const COLLABORATION_PRESENCE: &str = "collaboration.presence";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_ids_follow_convention() {
        // All call IDs should follow "program.action" format.
        let ids = [
            call_ids::STUDIO_CREATE_FRAME,
            call_ids::STUDIO_SET_FILL,
            call_ids::STUDIO_EXPORT,
            call_ids::ABACUS_GET_ROWS,
            call_ids::ABACUS_SET_CELL,
            call_ids::ABACUS_CREATE_VIEW,
            call_ids::QUILL_GET_CONTENT,
            call_ids::QUILL_INSERT_BLOCK,
            call_ids::LIBRARY_PUBLISH,
            call_ids::LIBRARY_LIST_IDEAS,
            call_ids::LIBRARY_TAG,
            call_ids::COURIER_SEND,
            call_ids::COURIER_COMPOSE,
            call_ids::PODIUM_ADD_SLIDE,
            call_ids::PODIUM_SET_TRANSITION,
            call_ids::TOME_CREATE_NOTE,
            call_ids::TOME_SEARCH,
        ];

        for id in ids {
            assert!(
                id.contains('.'),
                "call ID '{id}' must follow 'program.action' convention"
            );
            assert!(
                !id.is_empty(),
                "call ID must not be empty"
            );
        }
    }

    #[test]
    fn event_ids_follow_convention() {
        let ids = [
            event_ids::ABACUS_DATA_CHANGED,
            event_ids::ABACUS_ROW_ADDED,
            event_ids::ABACUS_ROW_DELETED,
            event_ids::LIBRARY_PUBLISHED,
            event_ids::LIBRARY_UPDATED,
            event_ids::STUDIO_SELECTION_CHANGED,
            event_ids::STUDIO_DESIGN_UPDATED,
            event_ids::COURIER_SENT,
            event_ids::COURIER_RECEIVED,
            event_ids::CONTENT_SAVED,
            event_ids::CONTENT_MODIFIED,
        ];

        for id in ids {
            assert!(
                id.contains('.'),
                "event ID '{id}' must follow 'program.eventName' convention"
            );
        }
    }

    #[test]
    fn channel_ids_follow_convention() {
        let ids = [
            channel_ids::COLLABORATION_EDIT,
            channel_ids::COLLABORATION_CURSOR,
            channel_ids::COLLABORATION_PRESENCE,
        ];

        for id in ids {
            assert!(
                id.contains('.'),
                "channel ID '{id}' must follow 'domain.type' convention"
            );
        }
    }

    #[test]
    fn no_duplicate_call_ids() {
        let ids = [
            call_ids::STUDIO_CREATE_FRAME,
            call_ids::STUDIO_SET_FILL,
            call_ids::STUDIO_EXPORT,
            call_ids::ABACUS_GET_ROWS,
            call_ids::ABACUS_SET_CELL,
            call_ids::ABACUS_CREATE_VIEW,
            call_ids::QUILL_GET_CONTENT,
            call_ids::QUILL_INSERT_BLOCK,
            call_ids::LIBRARY_PUBLISH,
            call_ids::LIBRARY_LIST_IDEAS,
            call_ids::LIBRARY_TAG,
            call_ids::COURIER_SEND,
            call_ids::COURIER_COMPOSE,
            call_ids::PODIUM_ADD_SLIDE,
            call_ids::PODIUM_SET_TRANSITION,
            call_ids::TOME_CREATE_NOTE,
            call_ids::TOME_SEARCH,
        ];

        let mut seen = std::collections::HashSet::new();
        for id in ids {
            assert!(seen.insert(id), "duplicate call ID: {id}");
        }
    }

    #[test]
    fn no_duplicate_event_ids() {
        let ids = [
            event_ids::ABACUS_DATA_CHANGED,
            event_ids::ABACUS_ROW_ADDED,
            event_ids::ABACUS_ROW_DELETED,
            event_ids::LIBRARY_PUBLISHED,
            event_ids::LIBRARY_UPDATED,
            event_ids::STUDIO_SELECTION_CHANGED,
            event_ids::STUDIO_DESIGN_UPDATED,
            event_ids::COURIER_SENT,
            event_ids::COURIER_RECEIVED,
            event_ids::CONTENT_SAVED,
            event_ids::CONTENT_MODIFIED,
        ];

        let mut seen = std::collections::HashSet::new();
        for id in ids {
            assert!(seen.insert(id), "duplicate event ID: {id}");
        }
    }
}
