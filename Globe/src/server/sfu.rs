use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::GlobeError;

/// Default max participants per SFU session.
pub const DEFAULT_MAX_PARTICIPANTS: usize = 25;

/// Default max simulcast layers per sender.
pub const DEFAULT_MAX_LAYERS: usize = 3;

/// Which quality layer a receiver prefers from a sender.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum LayerPreference {
    /// Best quality available.
    #[default]
    Highest,
    /// Middle quality (balanced).
    Middle,
    /// Lowest quality (bandwidth-saving).
    Lowest,
    /// Exact layer index.
    Specific(u8),
}

/// A simulcast video layer published by a sender.
///
/// Senders can publish the same video at multiple quality levels
/// (e.g., 180p/360p/720p). The SFU picks which layer each receiver gets.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct MediaLayer {
    /// Layer identifier (0 = lowest, increasing = higher quality).
    pub layer_id: u8,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Target bitrate in bits per second.
    pub bitrate: u32,
    /// Target framerate.
    pub framerate: u8,
}

/// SFU session configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SfuConfig {
    /// Max participants in one call.
    pub max_participants: usize,
    /// Max simulcast layers a sender can publish.
    pub max_layers_per_sender: usize,
    /// Default layer preference for receivers who haven't set one.
    pub default_layer: LayerPreference,
}

impl Default for SfuConfig {
    fn default() -> Self {
        Self {
            max_participants: DEFAULT_MAX_PARTICIPANTS,
            max_layers_per_sender: DEFAULT_MAX_LAYERS,
            default_layer: LayerPreference::Highest,
        }
    }
}

/// A participant in an SFU session.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SfuParticipant {
    /// WebSocket connection session ID (for frame routing).
    pub connection_session_id: u64,
    /// Participant's Crown public key (hex).
    pub crown_id: String,
    /// Simulcast layers this participant is sending.
    pub publishing: Vec<MediaLayer>,
    /// Per-sender layer preferences: sender_crown_id -> preference.
    pub receiving: HashMap<String, LayerPreference>,
}

/// An active SFU session (one group call).
#[derive(Clone, Debug)]
pub struct SfuSession {
    /// Communicator session ID (from Equipment).
    pub call_session_id: String,
    /// crown_id -> participant.
    pub participants: HashMap<String, SfuParticipant>,
    /// Session configuration.
    pub config: SfuConfig,
    /// Unix timestamp when session was created.
    pub created_at: i64,
}

/// Where to forward a binary frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForwardTarget {
    /// Which WebSocket connection to send to.
    pub connection_session_id: u64,
    /// Which layer they should receive.
    pub layer_id: u8,
}

/// Selective Forwarding Unit -- routes video streams between group call
/// participants.
///
/// Each participant publishes one or more `MediaLayer`s (simulcast).
/// The SFU selects which layer each receiver gets based on their
/// `LayerPreference`. This replaces broadcast-all binary frame routing
/// for group video calls.
///
/// For 1-to-1 calls, WebRTC is peer-to-peer (no SFU needed). The SFU
/// activates when a session has 3+ participants.
pub struct SfuRouter {
    sessions: HashMap<String, SfuSession>,
}

impl SfuRouter {
    /// Create a new SFU router with no active sessions.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    /// Create a new SFU session.
    pub fn create_session(
        &mut self,
        call_session_id: &str,
        config: SfuConfig,
        created_at: i64,
    ) -> Result<(), GlobeError> {
        if self.sessions.contains_key(call_session_id) {
            return Err(GlobeError::InvalidConfig(format!(
                "SFU session '{call_session_id}' already exists"
            )));
        }
        self.sessions.insert(
            call_session_id.to_string(),
            SfuSession {
                call_session_id: call_session_id.to_string(),
                participants: HashMap::new(),
                config,
                created_at,
            },
        );
        Ok(())
    }

    /// Add a participant to a session.
    pub fn add_participant(
        &mut self,
        call_session_id: &str,
        participant: SfuParticipant,
    ) -> Result<(), GlobeError> {
        let session = self
            .sessions
            .get_mut(call_session_id)
            .ok_or_else(|| {
                GlobeError::InvalidConfig(format!(
                    "SFU session '{call_session_id}' not found"
                ))
            })?;

        if session.participants.len() >= session.config.max_participants {
            return Err(GlobeError::InvalidConfig(format!(
                "SFU session '{call_session_id}' is full ({} participants)",
                session.config.max_participants
            )));
        }

        let participant_id = participant.crown_id.clone();
        session.participants.insert(participant_id, participant);
        Ok(())
    }

    /// Remove a participant from a session.
    ///
    /// Also cleans up other participants' layer preferences for the
    /// removed participant.
    pub fn remove_participant(
        &mut self,
        call_session_id: &str,
        crown_id: &str,
    ) -> Result<(), GlobeError> {
        let session = self
            .sessions
            .get_mut(call_session_id)
            .ok_or_else(|| {
                GlobeError::InvalidConfig(format!(
                    "SFU session '{call_session_id}' not found"
                ))
            })?;

        // Clean up preferences that reference the departing participant.
        for p in session.participants.values_mut() {
            p.receiving.remove(crown_id);
        }

        session.participants.remove(crown_id).ok_or_else(|| {
            GlobeError::InvalidConfig(format!(
                "participant '{crown_id}' not in session"
            ))
        })?;

        Ok(())
    }

    /// Update a participant's published simulcast layers.
    pub fn publish_layers(
        &mut self,
        call_session_id: &str,
        crown_id: &str,
        layers: Vec<MediaLayer>,
    ) -> Result<(), GlobeError> {
        let session = self
            .sessions
            .get_mut(call_session_id)
            .ok_or_else(|| {
                GlobeError::InvalidConfig(format!(
                    "SFU session '{call_session_id}' not found"
                ))
            })?;

        if layers.len() > session.config.max_layers_per_sender {
            return Err(GlobeError::InvalidConfig(format!(
                "too many layers ({}, max {})",
                layers.len(),
                session.config.max_layers_per_sender
            )));
        }

        let participant = session
            .participants
            .get_mut(crown_id)
            .ok_or_else(|| {
                GlobeError::InvalidConfig(format!(
                    "participant '{crown_id}' not in session"
                ))
            })?;

        participant.publishing = layers;
        Ok(())
    }

    /// Set a receiver's layer preference for a specific sender.
    pub fn set_preference(
        &mut self,
        call_session_id: &str,
        receiver_crown_id: &str,
        sender_crown_id: &str,
        preference: LayerPreference,
    ) -> Result<(), GlobeError> {
        let session = self
            .sessions
            .get_mut(call_session_id)
            .ok_or_else(|| {
                GlobeError::InvalidConfig(format!(
                    "SFU session '{call_session_id}' not found"
                ))
            })?;

        let receiver = session
            .participants
            .get_mut(receiver_crown_id)
            .ok_or_else(|| {
                GlobeError::InvalidConfig(format!(
                    "receiver '{receiver_crown_id}' not in session"
                ))
            })?;

        receiver
            .receiving
            .insert(sender_crown_id.to_string(), preference);
        Ok(())
    }

    /// The core routing decision.
    ///
    /// Given a binary frame from `sender_crown_id` at `layer_id`, determine
    /// which receivers should get it. A receiver is included only if their
    /// preferred layer (resolved against the sender's available layers)
    /// matches the incoming `layer_id`.
    pub fn route(
        &self,
        call_session_id: &str,
        sender_crown_id: &str,
        layer_id: u8,
    ) -> Vec<ForwardTarget> {
        let session = match self.sessions.get(call_session_id) {
            Some(s) => s,
            None => return vec![],
        };

        let sender = match session.participants.get(sender_crown_id) {
            Some(s) => s,
            None => return vec![],
        };

        let sender_layers = &sender.publishing;
        let mut targets = Vec::new();

        for (crown_id, participant) in &session.participants {
            // Don't forward to sender.
            if crown_id == sender_crown_id {
                continue;
            }

            let preference = participant
                .receiving
                .get(sender_crown_id)
                .unwrap_or(&session.config.default_layer);

            let target_layer = resolve_layer(preference, sender_layers);

            if target_layer == layer_id {
                targets.push(ForwardTarget {
                    connection_session_id: participant.connection_session_id,
                    layer_id,
                });
            }
        }

        targets
    }

    /// End and remove a session.
    pub fn end_session(&mut self, call_session_id: &str) -> Result<(), GlobeError> {
        self.sessions.remove(call_session_id).ok_or_else(|| {
            GlobeError::InvalidConfig(format!(
                "SFU session '{call_session_id}' not found"
            ))
        })?;
        Ok(())
    }

    /// List active session IDs.
    pub fn active_sessions(&self) -> Vec<&str> {
        self.sessions.keys().map(|s| s.as_str()).collect()
    }

    /// Get a session by ID.
    pub fn session(&self, call_session_id: &str) -> Option<&SfuSession> {
        self.sessions.get(call_session_id)
    }

    /// Number of active sessions.
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}

impl Default for SfuRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve which layer_id a receiver should get based on their preference
/// and the sender's available layers.
fn resolve_layer(preference: &LayerPreference, layers: &[MediaLayer]) -> u8 {
    if layers.is_empty() {
        return 0;
    }

    match preference {
        LayerPreference::Highest => layers
            .iter()
            .max_by_key(|l| l.bitrate)
            .map(|l| l.layer_id)
            .unwrap_or(0),
        LayerPreference::Lowest => layers
            .iter()
            .min_by_key(|l| l.bitrate)
            .map(|l| l.layer_id)
            .unwrap_or(0),
        LayerPreference::Middle => {
            let mut sorted: Vec<_> = layers.to_vec();
            sorted.sort_by_key(|l| l.bitrate);
            sorted
                .get(sorted.len() / 2)
                .map(|l| l.layer_id)
                .unwrap_or(0)
        }
        LayerPreference::Specific(id) => {
            if layers.iter().any(|l| l.layer_id == *id) {
                *id
            } else {
                // Fallback to highest if requested layer doesn't exist.
                layers
                    .iter()
                    .max_by_key(|l| l.bitrate)
                    .map(|l| l.layer_id)
                    .unwrap_or(0)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_layers() -> Vec<MediaLayer> {
        vec![
            MediaLayer {
                layer_id: 0,
                width: 320,
                height: 180,
                bitrate: 150_000,
                framerate: 15,
            },
            MediaLayer {
                layer_id: 1,
                width: 640,
                height: 360,
                bitrate: 500_000,
                framerate: 30,
            },
            MediaLayer {
                layer_id: 2,
                width: 1280,
                height: 720,
                bitrate: 1_500_000,
                framerate: 30,
            },
        ]
    }

    fn make_participant(crown_id: &str, conn_id: u64) -> SfuParticipant {
        SfuParticipant {
            connection_session_id: conn_id,
            crown_id: crown_id.to_string(),
            publishing: vec![],
            receiving: HashMap::new(),
        }
    }

    #[test]
    fn create_and_end_session() {
        let mut router = SfuRouter::new();
        router
            .create_session("call-1", SfuConfig::default(), 1000)
            .unwrap();
        assert_eq!(router.session_count(), 1);
        assert!(router.session("call-1").is_some());
        router.end_session("call-1").unwrap();
        assert_eq!(router.session_count(), 0);
    }

    #[test]
    fn duplicate_session_rejected() {
        let mut router = SfuRouter::new();
        router
            .create_session("call-1", SfuConfig::default(), 1000)
            .unwrap();
        assert!(router
            .create_session("call-1", SfuConfig::default(), 2000)
            .is_err());
    }

    #[test]
    fn add_and_remove_participant() {
        let mut router = SfuRouter::new();
        router
            .create_session("call-1", SfuConfig::default(), 1000)
            .unwrap();

        router
            .add_participant("call-1", make_participant("alice", 100))
            .unwrap();

        let session = router.session("call-1").unwrap();
        assert_eq!(session.participants.len(), 1);

        router.remove_participant("call-1", "alice").unwrap();
        let session = router.session("call-1").unwrap();
        assert!(session.participants.is_empty());
    }

    #[test]
    fn session_full_rejected() {
        let config = SfuConfig {
            max_participants: 2,
            ..Default::default()
        };
        let mut router = SfuRouter::new();
        router.create_session("call-1", config, 1000).unwrap();
        router
            .add_participant("call-1", make_participant("alice", 1))
            .unwrap();
        router
            .add_participant("call-1", make_participant("bob", 2))
            .unwrap();
        assert!(router
            .add_participant("call-1", make_participant("charlie", 3))
            .is_err());
    }

    #[test]
    fn publish_layers_ok() {
        let mut router = SfuRouter::new();
        router
            .create_session("call-1", SfuConfig::default(), 1000)
            .unwrap();
        router
            .add_participant("call-1", make_participant("alice", 1))
            .unwrap();

        router
            .publish_layers("call-1", "alice", make_layers())
            .unwrap();

        let session = router.session("call-1").unwrap();
        assert_eq!(session.participants["alice"].publishing.len(), 3);
    }

    #[test]
    fn too_many_layers_rejected() {
        let config = SfuConfig {
            max_layers_per_sender: 2,
            ..Default::default()
        };
        let mut router = SfuRouter::new();
        router.create_session("call-1", config, 1000).unwrap();
        router
            .add_participant("call-1", make_participant("alice", 1))
            .unwrap();
        assert!(router
            .publish_layers("call-1", "alice", make_layers())
            .is_err());
    }

    #[test]
    fn route_broadcasts_to_all_except_sender() {
        let mut router = SfuRouter::new();
        router
            .create_session("call-1", SfuConfig::default(), 1000)
            .unwrap();

        let mut alice = make_participant("alice", 1);
        alice.publishing = make_layers();
        router.add_participant("call-1", alice).unwrap();
        router
            .add_participant("call-1", make_participant("bob", 2))
            .unwrap();
        router
            .add_participant("call-1", make_participant("charlie", 3))
            .unwrap();

        // Default preference is Highest -> layer 2 (1.5 Mbps).
        let targets = router.route("call-1", "alice", 2);
        assert_eq!(targets.len(), 2);
        let conn_ids: Vec<u64> =
            targets.iter().map(|t| t.connection_session_id).collect();
        assert!(conn_ids.contains(&2));
        assert!(conn_ids.contains(&3));
    }

    #[test]
    fn route_respects_layer_preference() {
        let mut router = SfuRouter::new();
        router
            .create_session("call-1", SfuConfig::default(), 1000)
            .unwrap();

        let mut alice = make_participant("alice", 1);
        alice.publishing = make_layers();
        router.add_participant("call-1", alice).unwrap();
        router
            .add_participant("call-1", make_participant("bob", 2))
            .unwrap();
        router
            .add_participant("call-1", make_participant("charlie", 3))
            .unwrap();

        // Bob wants lowest quality from Alice.
        router
            .set_preference("call-1", "bob", "alice", LayerPreference::Lowest)
            .unwrap();

        // Route layer 0 (lowest) -- only Bob should get it.
        let targets = router.route("call-1", "alice", 0);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].connection_session_id, 2);

        // Route layer 2 (highest) -- only Charlie should get it (default).
        let targets = router.route("call-1", "alice", 2);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].connection_session_id, 3);
    }

    #[test]
    fn route_unknown_session_returns_empty() {
        let router = SfuRouter::new();
        assert!(router.route("nonexistent", "alice", 0).is_empty());
    }

    #[test]
    fn route_unknown_sender_returns_empty() {
        let mut router = SfuRouter::new();
        router
            .create_session("call-1", SfuConfig::default(), 1000)
            .unwrap();
        assert!(router.route("call-1", "ghost", 0).is_empty());
    }

    #[test]
    fn resolve_layer_highest() {
        let layers = make_layers();
        assert_eq!(resolve_layer(&LayerPreference::Highest, &layers), 2);
    }

    #[test]
    fn resolve_layer_lowest() {
        let layers = make_layers();
        assert_eq!(resolve_layer(&LayerPreference::Lowest, &layers), 0);
    }

    #[test]
    fn resolve_layer_middle() {
        let layers = make_layers();
        assert_eq!(resolve_layer(&LayerPreference::Middle, &layers), 1);
    }

    #[test]
    fn resolve_layer_specific_exists() {
        let layers = make_layers();
        assert_eq!(resolve_layer(&LayerPreference::Specific(1), &layers), 1);
    }

    #[test]
    fn resolve_layer_specific_missing_falls_back() {
        let layers = make_layers();
        assert_eq!(resolve_layer(&LayerPreference::Specific(5), &layers), 2);
    }

    #[test]
    fn resolve_layer_empty_layers() {
        assert_eq!(resolve_layer(&LayerPreference::Highest, &[]), 0);
    }

    #[test]
    fn remove_participant_cleans_preferences() {
        let mut router = SfuRouter::new();
        router
            .create_session("call-1", SfuConfig::default(), 1000)
            .unwrap();
        router
            .add_participant("call-1", make_participant("alice", 1))
            .unwrap();
        router
            .add_participant("call-1", make_participant("bob", 2))
            .unwrap();

        router
            .set_preference("call-1", "alice", "bob", LayerPreference::Lowest)
            .unwrap();
        router.remove_participant("call-1", "bob").unwrap();

        let session = router.session("call-1").unwrap();
        assert!(!session.participants["alice"].receiving.contains_key("bob"));
    }

    #[test]
    fn active_sessions_lists_all() {
        let mut router = SfuRouter::new();
        router
            .create_session("call-1", SfuConfig::default(), 1000)
            .unwrap();
        router
            .create_session("call-2", SfuConfig::default(), 2000)
            .unwrap();
        let mut sessions = router.active_sessions();
        sessions.sort();
        assert_eq!(sessions, vec!["call-1", "call-2"]);
    }

    #[test]
    fn sfu_config_serde_round_trip() {
        let config = SfuConfig {
            max_participants: 10,
            max_layers_per_sender: 2,
            default_layer: LayerPreference::Middle,
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: SfuConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.max_participants, 10);
        assert_eq!(loaded.default_layer, LayerPreference::Middle);
    }

    #[test]
    fn media_layer_serde_round_trip() {
        let layer = MediaLayer {
            layer_id: 1,
            width: 1280,
            height: 720,
            bitrate: 1_500_000,
            framerate: 30,
        };
        let json = serde_json::to_string(&layer).unwrap();
        let loaded: MediaLayer = serde_json::from_str(&json).unwrap();
        assert_eq!(layer, loaded);
    }

    #[test]
    fn end_nonexistent_session_rejected() {
        let mut router = SfuRouter::new();
        assert!(router.end_session("ghost").is_err());
    }

    #[test]
    fn remove_nonexistent_participant_rejected() {
        let mut router = SfuRouter::new();
        router
            .create_session("call-1", SfuConfig::default(), 1000)
            .unwrap();
        assert!(router
            .remove_participant("call-1", "ghost")
            .is_err());
    }
}
