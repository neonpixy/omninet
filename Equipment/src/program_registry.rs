//! Convenience helpers for registering Throne programs with Equipment.
//!
//! Each Throne program advertises its capabilities: what Phone calls it
//! handles, what Email events it emits or subscribes to, and what
//! Communicator channels it supports. [`ProgramRegistration`] is a
//! builder that collects these declarations and converts them into a
//! [`ModuleCatalog`] suitable for registering with Contacts.
//!
//! Pre-built registrations for all seven Throne programs are provided
//! via `studio_registration()`, `abacus_registration()`, etc.
//!
//! # Example
//!
//! ```
//! use equipment::program_registry::{ProgramRegistration, studio_registration};
//!
//! // Use a pre-built registration:
//! let studio = studio_registration();
//! let catalog = studio.to_catalog();
//! assert!(!catalog.calls_handled().is_empty());
//!
//! // Or build a custom one:
//! let custom = ProgramRegistration::new("my_plugin")
//!     .handles_call("my_plugin.doThing")
//!     .emits_event("my_plugin.thingDone");
//! let catalog = custom.to_catalog();
//! assert_eq!(catalog.calls_handled().len(), 1);
//! assert_eq!(catalog.events_emitted().len(), 1);
//! ```

use crate::catalog::{CallDescriptor, ChannelDescriptor, EventDescriptor, ModuleCatalog};
use crate::programs::{call_ids, channel_ids, event_ids};

/// Builder for declaring a program's communication capabilities.
///
/// Collects call IDs, event IDs, and channel IDs, then converts them
/// into a [`ModuleCatalog`] for registration with Contacts.
#[derive(Clone, Debug)]
pub struct ProgramRegistration {
    /// The module identifier (e.g., `"studio"`, `"abacus"`).
    pub module_id: String,
    /// Phone call IDs this program handles.
    pub calls: Vec<String>,
    /// Email event IDs this program emits.
    pub emits: Vec<String>,
    /// Email event IDs this program listens for.
    pub subscribes: Vec<String>,
    /// Communicator channel IDs this program supports.
    pub channels: Vec<String>,
}

impl ProgramRegistration {
    /// Create a new registration for the given module ID.
    pub fn new(module_id: &str) -> Self {
        Self {
            module_id: module_id.to_string(),
            calls: Vec::new(),
            emits: Vec::new(),
            subscribes: Vec::new(),
            channels: Vec::new(),
        }
    }

    /// Declare that this program handles the given Phone call ID.
    pub fn handles_call(mut self, call_id: &str) -> Self {
        self.calls.push(call_id.to_string());
        self
    }

    /// Declare that this program emits the given Email event ID.
    pub fn emits_event(mut self, event_id: &str) -> Self {
        self.emits.push(event_id.to_string());
        self
    }

    /// Declare that this program subscribes to the given Email event ID.
    pub fn subscribes_to(mut self, event_id: &str) -> Self {
        self.subscribes.push(event_id.to_string());
        self
    }

    /// Declare that this program supports the given Communicator channel ID.
    pub fn supports_channel(mut self, channel_id: &str) -> Self {
        self.channels.push(channel_id.to_string());
        self
    }

    /// Build a [`ModuleCatalog`] from this registration.
    ///
    /// Each call, event, and channel is converted to its respective descriptor
    /// with a generated description based on the ID.
    pub fn to_catalog(&self) -> ModuleCatalog {
        let calls: Vec<CallDescriptor> = self
            .calls
            .iter()
            .map(|id| {
                CallDescriptor::new(
                    id.as_str(),
                    format!("{} call: {}", self.module_id, id),
                )
            })
            .collect();

        let emitted: Vec<EventDescriptor> = self
            .emits
            .iter()
            .map(|id| {
                EventDescriptor::new(
                    id.as_str(),
                    format!("{} emits: {}", self.module_id, id),
                )
            })
            .collect();

        let subscribed: Vec<EventDescriptor> = self
            .subscribes
            .iter()
            .map(|id| {
                EventDescriptor::new(
                    id.as_str(),
                    format!("{} subscribes: {}", self.module_id, id),
                )
            })
            .collect();

        let channels: Vec<ChannelDescriptor> = self
            .channels
            .iter()
            .map(|id| {
                ChannelDescriptor::new(
                    id.as_str(),
                    format!("{} channel: {}", self.module_id, id),
                )
            })
            .collect();

        ModuleCatalog::new()
            .with_calls(calls)
            .with_emitted_events(emitted)
            .with_subscribed_events(subscribed)
            .with_channels(channels)
    }
}

/// Pre-built registration for Studio (design canvas + Genius design system).
///
/// Studio handles design manipulation calls, emits selection/design events,
/// subscribes to data changes for live-bound elements, and supports all
/// collaboration channels.
pub fn studio_registration() -> ProgramRegistration {
    ProgramRegistration::new("studio")
        // Calls Studio handles
        .handles_call(call_ids::STUDIO_CREATE_FRAME)
        .handles_call(call_ids::STUDIO_SET_FILL)
        .handles_call(call_ids::STUDIO_EXPORT)
        // Events Studio emits
        .emits_event(event_ids::STUDIO_SELECTION_CHANGED)
        .emits_event(event_ids::STUDIO_DESIGN_UPDATED)
        .emits_event(event_ids::CONTENT_SAVED)
        .emits_event(event_ids::CONTENT_MODIFIED)
        // Events Studio subscribes to
        .subscribes_to(event_ids::ABACUS_DATA_CHANGED)
        .subscribes_to(event_ids::LIBRARY_UPDATED)
        // Channels Studio supports
        .supports_channel(channel_ids::COLLABORATION_EDIT)
        .supports_channel(channel_ids::COLLABORATION_CURSOR)
        .supports_channel(channel_ids::COLLABORATION_PRESENCE)
}

/// Pre-built registration for Abacus (sheets and databases).
///
/// Abacus handles data access calls, emits change events that drive
/// live data bindings, and supports collaborative editing.
pub fn abacus_registration() -> ProgramRegistration {
    ProgramRegistration::new("abacus")
        // Calls Abacus handles
        .handles_call(call_ids::ABACUS_GET_ROWS)
        .handles_call(call_ids::ABACUS_SET_CELL)
        .handles_call(call_ids::ABACUS_CREATE_VIEW)
        // Events Abacus emits
        .emits_event(event_ids::ABACUS_DATA_CHANGED)
        .emits_event(event_ids::ABACUS_ROW_ADDED)
        .emits_event(event_ids::ABACUS_ROW_DELETED)
        .emits_event(event_ids::CONTENT_SAVED)
        .emits_event(event_ids::CONTENT_MODIFIED)
        // Events Abacus subscribes to (none by default)
        // Channels Abacus supports
        .supports_channel(channel_ids::COLLABORATION_EDIT)
        .supports_channel(channel_ids::COLLABORATION_CURSOR)
        .supports_channel(channel_ids::COLLABORATION_PRESENCE)
}

/// Pre-built registration for Quill (long-form documents).
///
/// Quill handles document content calls, emits content events,
/// subscribes to Abacus data changes for embedded references,
/// and supports collaborative editing.
pub fn quill_registration() -> ProgramRegistration {
    ProgramRegistration::new("quill")
        // Calls Quill handles
        .handles_call(call_ids::QUILL_GET_CONTENT)
        .handles_call(call_ids::QUILL_INSERT_BLOCK)
        // Events Quill emits
        .emits_event(event_ids::CONTENT_SAVED)
        .emits_event(event_ids::CONTENT_MODIFIED)
        // Events Quill subscribes to
        .subscribes_to(event_ids::ABACUS_DATA_CHANGED)
        .subscribes_to(event_ids::LIBRARY_UPDATED)
        // Channels Quill supports
        .supports_channel(channel_ids::COLLABORATION_EDIT)
        .supports_channel(channel_ids::COLLABORATION_CURSOR)
        .supports_channel(channel_ids::COLLABORATION_PRESENCE)
}

/// Pre-built registration for Library (digital idea and asset management).
///
/// Library handles asset management calls, emits publish/update events
/// that other programs subscribe to for live content references.
pub fn library_registration() -> ProgramRegistration {
    ProgramRegistration::new("library")
        // Calls Library handles
        .handles_call(call_ids::LIBRARY_PUBLISH)
        .handles_call(call_ids::LIBRARY_LIST_IDEAS)
        .handles_call(call_ids::LIBRARY_TAG)
        // Events Library emits
        .emits_event(event_ids::LIBRARY_PUBLISHED)
        .emits_event(event_ids::LIBRARY_UPDATED)
        .emits_event(event_ids::CONTENT_SAVED)
        // Events Library subscribes to
        .subscribes_to(event_ids::CONTENT_SAVED)
        // Channels (Library does not need real-time collaboration)
}

/// Pre-built registration for Courier (mail and messaging).
///
/// Courier handles send/compose calls, emits sent/received events,
/// and subscribes to content events for triggered sends
/// (e.g., "send confirmation when form submitted").
pub fn courier_registration() -> ProgramRegistration {
    ProgramRegistration::new("courier")
        // Calls Courier handles
        .handles_call(call_ids::COURIER_SEND)
        .handles_call(call_ids::COURIER_COMPOSE)
        // Events Courier emits
        .emits_event(event_ids::COURIER_SENT)
        .emits_event(event_ids::COURIER_RECEIVED)
        // Events Courier subscribes to
        .subscribes_to(event_ids::ABACUS_ROW_ADDED)
        .subscribes_to(event_ids::LIBRARY_PUBLISHED)
        // Channels (Courier does not need real-time collaboration)
}

/// Pre-built registration for Podium (presentations).
///
/// Podium handles slide manipulation calls, emits content events,
/// subscribes to Abacus data changes for live data in slides,
/// and supports collaborative editing for shared presentations.
pub fn podium_registration() -> ProgramRegistration {
    ProgramRegistration::new("podium")
        // Calls Podium handles
        .handles_call(call_ids::PODIUM_ADD_SLIDE)
        .handles_call(call_ids::PODIUM_SET_TRANSITION)
        // Events Podium emits
        .emits_event(event_ids::CONTENT_SAVED)
        .emits_event(event_ids::CONTENT_MODIFIED)
        // Events Podium subscribes to
        .subscribes_to(event_ids::ABACUS_DATA_CHANGED)
        .subscribes_to(event_ids::STUDIO_DESIGN_UPDATED)
        .subscribes_to(event_ids::LIBRARY_UPDATED)
        // Channels Podium supports
        .supports_channel(channel_ids::COLLABORATION_EDIT)
        .supports_channel(channel_ids::COLLABORATION_CURSOR)
        .supports_channel(channel_ids::COLLABORATION_PRESENCE)
}

/// Pre-built registration for Tome (notes).
///
/// Tome handles note creation and search calls, emits content events.
/// Lightweight -- no real-time collaboration channels by default.
pub fn tome_registration() -> ProgramRegistration {
    ProgramRegistration::new("tome")
        // Calls Tome handles
        .handles_call(call_ids::TOME_CREATE_NOTE)
        .handles_call(call_ids::TOME_SEARCH)
        // Events Tome emits
        .emits_event(event_ids::CONTENT_SAVED)
        .emits_event(event_ids::CONTENT_MODIFIED)
        // Events Tome subscribes to (none by default)
        // Channels (Tome does not need real-time collaboration)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn studio_registration_has_expected_capabilities() {
        let reg = studio_registration();
        assert_eq!(reg.module_id, "studio");
        assert_eq!(reg.calls.len(), 3);
        assert_eq!(reg.emits.len(), 4);
        assert_eq!(reg.subscribes.len(), 2);
        assert_eq!(reg.channels.len(), 3);
    }

    #[test]
    fn abacus_registration_has_expected_capabilities() {
        let reg = abacus_registration();
        assert_eq!(reg.module_id, "abacus");
        assert_eq!(reg.calls.len(), 3);
        assert_eq!(reg.emits.len(), 5);
        assert!(reg.subscribes.is_empty());
        assert_eq!(reg.channels.len(), 3);
    }

    #[test]
    fn quill_registration_has_expected_capabilities() {
        let reg = quill_registration();
        assert_eq!(reg.module_id, "quill");
        assert_eq!(reg.calls.len(), 2);
        assert_eq!(reg.emits.len(), 2);
        assert_eq!(reg.subscribes.len(), 2);
        assert_eq!(reg.channels.len(), 3);
    }

    #[test]
    fn library_registration_has_expected_capabilities() {
        let reg = library_registration();
        assert_eq!(reg.module_id, "library");
        assert_eq!(reg.calls.len(), 3);
        assert_eq!(reg.emits.len(), 3);
        assert_eq!(reg.subscribes.len(), 1);
        assert!(reg.channels.is_empty());
    }

    #[test]
    fn courier_registration_has_expected_capabilities() {
        let reg = courier_registration();
        assert_eq!(reg.module_id, "courier");
        assert_eq!(reg.calls.len(), 2);
        assert_eq!(reg.emits.len(), 2);
        assert_eq!(reg.subscribes.len(), 2);
        assert!(reg.channels.is_empty());
    }

    #[test]
    fn podium_registration_has_expected_capabilities() {
        let reg = podium_registration();
        assert_eq!(reg.module_id, "podium");
        assert_eq!(reg.calls.len(), 2);
        assert_eq!(reg.emits.len(), 2);
        assert_eq!(reg.subscribes.len(), 3);
        assert_eq!(reg.channels.len(), 3);
    }

    #[test]
    fn tome_registration_has_expected_capabilities() {
        let reg = tome_registration();
        assert_eq!(reg.module_id, "tome");
        assert_eq!(reg.calls.len(), 2);
        assert_eq!(reg.emits.len(), 2);
        assert!(reg.subscribes.is_empty());
        assert!(reg.channels.is_empty());
    }

    #[test]
    fn to_catalog_produces_correct_descriptors() {
        let reg = studio_registration();
        let catalog = reg.to_catalog();

        assert_eq!(catalog.calls_handled().len(), 3);
        assert_eq!(catalog.events_emitted().len(), 4);
        assert_eq!(catalog.events_subscribed().len(), 2);
        assert_eq!(catalog.channels_supported().len(), 3);

        // Verify call IDs round-trip correctly.
        assert_eq!(
            catalog.calls_handled()[0].call_id(),
            call_ids::STUDIO_CREATE_FRAME
        );
        assert_eq!(
            catalog.calls_handled()[1].call_id(),
            call_ids::STUDIO_SET_FILL
        );
        assert_eq!(
            catalog.calls_handled()[2].call_id(),
            call_ids::STUDIO_EXPORT
        );

        // Verify event IDs round-trip correctly.
        assert_eq!(
            catalog.events_emitted()[0].email_id(),
            event_ids::STUDIO_SELECTION_CHANGED
        );

        // Verify channel IDs round-trip correctly.
        assert_eq!(
            catalog.channels_supported()[0].channel_id(),
            channel_ids::COLLABORATION_EDIT
        );
    }

    #[test]
    fn custom_registration_builder() {
        let reg = ProgramRegistration::new("custom_plugin")
            .handles_call("custom.doThing")
            .handles_call("custom.undoThing")
            .emits_event("custom.thingDone")
            .subscribes_to("content.saved")
            .supports_channel("collaboration.edit");

        assert_eq!(reg.module_id, "custom_plugin");
        assert_eq!(reg.calls.len(), 2);
        assert_eq!(reg.emits.len(), 1);
        assert_eq!(reg.subscribes.len(), 1);
        assert_eq!(reg.channels.len(), 1);

        let catalog = reg.to_catalog();
        assert_eq!(catalog.calls_handled().len(), 2);
        assert_eq!(catalog.calls_handled()[0].call_id(), "custom.doThing");
    }

    #[test]
    fn empty_registration_produces_empty_catalog() {
        let reg = ProgramRegistration::new("empty");
        let catalog = reg.to_catalog();

        assert!(catalog.calls_handled().is_empty());
        assert!(catalog.events_emitted().is_empty());
        assert!(catalog.events_subscribed().is_empty());
        assert!(catalog.channels_supported().is_empty());
    }

    #[test]
    fn registration_clone() {
        let reg = studio_registration();
        let cloned = reg.clone();

        assert_eq!(reg.module_id, cloned.module_id);
        assert_eq!(reg.calls.len(), cloned.calls.len());
        assert_eq!(reg.emits.len(), cloned.emits.len());
    }

    #[test]
    fn all_programs_have_unique_call_ids() {
        let all_registrations = vec![
            studio_registration(),
            abacus_registration(),
            quill_registration(),
            library_registration(),
            courier_registration(),
            podium_registration(),
            tome_registration(),
        ];

        let mut seen = std::collections::HashSet::new();
        for reg in &all_registrations {
            for call_id in &reg.calls {
                assert!(
                    seen.insert(call_id.clone()),
                    "duplicate call ID across programs: {call_id}"
                );
            }
        }
    }

    #[test]
    fn catalog_round_trips_through_serde() {
        let reg = studio_registration();
        let catalog = reg.to_catalog();

        let json = serde_json::to_string(&catalog).unwrap();
        let loaded: ModuleCatalog = serde_json::from_str(&json).unwrap();
        assert_eq!(catalog, loaded);
    }

    #[test]
    fn all_registrations_produce_valid_catalogs() {
        let registrations = vec![
            studio_registration(),
            abacus_registration(),
            quill_registration(),
            library_registration(),
            courier_registration(),
            podium_registration(),
            tome_registration(),
        ];

        for reg in registrations {
            let catalog = reg.to_catalog();

            // Every call/event/channel should have a non-empty ID and description.
            for call in catalog.calls_handled() {
                assert!(!call.call_id().is_empty());
                assert!(!call.description().is_empty());
            }
            for event in catalog.events_emitted() {
                assert!(!event.email_id().is_empty());
                assert!(!event.description().is_empty());
            }
            for event in catalog.events_subscribed() {
                assert!(!event.email_id().is_empty());
                assert!(!event.description().is_empty());
            }
            for channel in catalog.channels_supported() {
                assert!(!channel.channel_id().is_empty());
                assert!(!channel.description().is_empty());
            }
        }
    }

    #[test]
    fn integration_with_contacts() {
        use crate::contacts::{Contacts, ModuleInfo, ModuleType};

        let contacts = Contacts::new();

        // Register all seven Throne programs.
        let registrations = vec![
            studio_registration(),
            abacus_registration(),
            quill_registration(),
            library_registration(),
            courier_registration(),
            podium_registration(),
            tome_registration(),
        ];

        for reg in &registrations {
            let info = ModuleInfo::new(&reg.module_id, &reg.module_id, ModuleType::App)
                .with_catalog(reg.to_catalog());
            contacts.register(info).unwrap();
        }

        // Studio handles studio.createFrame.
        assert_eq!(
            contacts.who_handles(call_ids::STUDIO_CREATE_FRAME),
            Some("studio".to_string())
        );

        // Abacus handles abacus.getRows.
        assert_eq!(
            contacts.who_handles(call_ids::ABACUS_GET_ROWS),
            Some("abacus".to_string())
        );

        // Abacus emits dataChanged, Studio subscribes to it.
        let emitters = contacts.who_emits(event_ids::ABACUS_DATA_CHANGED);
        assert!(emitters.contains(&"abacus".to_string()));

        let subscribers = contacts.who_subscribes(event_ids::ABACUS_DATA_CHANGED);
        assert!(subscribers.contains(&"studio".to_string()));
        assert!(subscribers.contains(&"quill".to_string()));
        assert!(subscribers.contains(&"podium".to_string()));

        // Topology shows the wiring.
        let topo = contacts.topology();
        assert!(!topo.edges.is_empty());

        // All 17 call IDs should appear as call edges.
        let total_calls: usize = registrations.iter().map(|r| r.calls.len()).sum();
        let call_edge_count = topo
            .edges
            .iter()
            .filter(|e| e.edge_type == crate::catalog::EdgeType::Call)
            .count();
        assert_eq!(call_edge_count, total_calls);
    }
}
