use crown::CrownKeypair;

use crate::error::GlobeError;
use crate::event::OmniEvent;
use crate::event_builder::{EventBuilder, UnsignedEvent};
use crate::kind;

/// Builds ORP events for Communicator session signaling.
///
/// Globe is transport-only — it doesn't understand the content of these events.
/// The content field carries opaque JSON (typically encrypted by the caller
/// via Crown ECDH + Sentinal AES-256-GCM). Globe just wraps it in a signed
/// event with the right kind and tags.
pub struct SignalingBuilder;

impl SignalingBuilder {
    /// Build a session offer event (kind 5100).
    ///
    /// - `session_id`: unique session identifier
    /// - `participants`: crown_id hex strings of all participants
    /// - `encrypted_content`: JSON payload (encrypted to participants by caller)
    /// - `keypair`: signer's Crown keypair
    pub fn offer(
        session_id: &str,
        participants: &[&str],
        encrypted_content: &str,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        let mut unsigned =
            UnsignedEvent::new(kind::COMMUNICATOR_OFFER, encrypted_content)
                .with_tag("session", &[session_id]);
        for crown_id in participants {
            unsigned = unsigned.with_tag("p", &[crown_id]);
        }
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Build a session answer event (kind 5101).
    ///
    /// - `session_id`: the session being answered
    /// - `initiator_crown_id`: the offer sender's crown_id (for routing)
    /// - `content`: JSON response (e.g. `{"accepted": true}`)
    /// - `keypair`: responder's Crown keypair
    pub fn answer(
        session_id: &str,
        initiator_crown_id: &str,
        content: &str,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        let unsigned = UnsignedEvent::new(kind::COMMUNICATOR_ANSWER, content)
            .with_tag("session", &[session_id])
            .with_tag("p", &[initiator_crown_id]);
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Build a session end event (kind 5102).
    ///
    /// - `session_id`: the session ending
    /// - `content`: JSON with reason (e.g. `{"reason": "normal"}`)
    /// - `keypair`: signer's Crown keypair
    pub fn end(
        session_id: &str,
        content: &str,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        let unsigned = UnsignedEvent::new(kind::COMMUNICATOR_END, content)
            .with_tag("session", &[session_id]);
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Build a stream announcement event (kind 5110).
    ///
    /// - `session_id`: unique stream session ID (used as d-tag for replaceability)
    /// - `content`: JSON stream metadata (title, kind, status, fortune config)
    /// - `keypair`: streamer's Crown keypair
    pub fn stream_announce(
        session_id: &str,
        content: &str,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        let unsigned = UnsignedEvent::new(kind::STREAM_ANNOUNCE, content)
            .with_tag("session", &[session_id])
            .with_d_tag(session_id);
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Build a stream update event (kind 5111).
    pub fn stream_update(
        session_id: &str,
        content: &str,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        let unsigned = UnsignedEvent::new(kind::STREAM_UPDATE, content)
            .with_tag("session", &[session_id])
            .with_d_tag(session_id);
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Build a stream end event (kind 5112).
    pub fn stream_end(
        session_id: &str,
        content: &str,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        let unsigned = UnsignedEvent::new(kind::STREAM_END, content)
            .with_tag("session", &[session_id]);
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Build an ICE candidate event (kind 5103).
    ///
    /// Carries a WebRTC ICE candidate for peer connection setup.
    /// Content is opaque (encrypted to the target's Crown pubkey by the caller).
    ///
    /// - `session_id`: the session this candidate belongs to
    /// - `target_crown_id`: the peer who should receive this candidate
    /// - `content`: ICE candidate JSON (encrypted by caller)
    /// - `keypair`: sender's Crown keypair
    pub fn ice_candidate(
        session_id: &str,
        target_crown_id: &str,
        content: &str,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        let unsigned = UnsignedEvent::new(kind::ICE_CANDIDATE, content)
            .with_tag("session", &[session_id])
            .with_tag("p", &[target_crown_id]);
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Build a stream recording event (kind 5113).
    ///
    /// Links a completed live stream to its chunk manifest so viewers
    /// can replay it. The streamer's app records locally during the
    /// stream, then chunks + uploads after the stream ends.
    ///
    /// - `session_id`: the stream session that was recorded
    /// - `manifest_hash`: SHA-256 content hash from ChunkManifest
    /// - `content`: JSON metadata (duration_secs, format, thumbnail_hash, etc.)
    /// - `keypair`: streamer's Crown keypair
    pub fn stream_recording(
        session_id: &str,
        manifest_hash: &str,
        content: &str,
        keypair: &CrownKeypair,
    ) -> Result<OmniEvent, GlobeError> {
        let unsigned = UnsignedEvent::new(kind::STREAM_RECORDING, content)
            .with_tag("session", &[session_id])
            .with_d_tag(manifest_hash);
        EventBuilder::sign(&unsigned, keypair)
    }

    /// Parse a session ID from a signaling event's tags.
    pub fn parse_session_id(event: &OmniEvent) -> Option<String> {
        event
            .tag_values("session")
            .first()
            .map(|s| s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_keypair() -> CrownKeypair {
        CrownKeypair::generate()
    }

    #[test]
    fn offer_event_structure() {
        let kp = test_keypair();
        let participants = vec!["cpub_alice", "cpub_bob"];
        let event = SignalingBuilder::offer(
            "sess-001",
            &participants,
            r#"{"encrypted": "data"}"#,
            &kp,
        )
        .unwrap();

        assert_eq!(event.kind, kind::COMMUNICATOR_OFFER);
        assert_eq!(event.content, r#"{"encrypted": "data"}"#);

        // Should have session tag.
        let session_ids = event.tag_values("session");
        assert_eq!(session_ids, vec!["sess-001"]);

        // Should have p tags for each participant.
        let p_tags = event.tag_values("p");
        assert!(p_tags.contains(&"cpub_alice"));
        assert!(p_tags.contains(&"cpub_bob"));
    }

    #[test]
    fn answer_event_structure() {
        let kp = test_keypair();
        let event = SignalingBuilder::answer(
            "sess-001",
            "cpub_alice",
            r#"{"accepted": true}"#,
            &kp,
        )
        .unwrap();

        assert_eq!(event.kind, kind::COMMUNICATOR_ANSWER);
        let session_ids = event.tag_values("session");
        assert_eq!(session_ids, vec!["sess-001"]);
        let p_tags = event.tag_values("p");
        assert_eq!(p_tags, vec!["cpub_alice"]);
    }

    #[test]
    fn end_event_structure() {
        let kp = test_keypair();
        let event =
            SignalingBuilder::end("sess-001", r#"{"reason": "normal"}"#, &kp).unwrap();

        assert_eq!(event.kind, kind::COMMUNICATOR_END);
        let session_ids = event.tag_values("session");
        assert_eq!(session_ids, vec!["sess-001"]);
    }

    #[test]
    fn stream_announce_has_d_tag() {
        let kp = test_keypair();
        let event = SignalingBuilder::stream_announce(
            "stream-001",
            r#"{"title": "Live Jazz"}"#,
            &kp,
        )
        .unwrap();

        assert_eq!(event.kind, kind::STREAM_ANNOUNCE);
        let d_tags = event.tag_values("d");
        assert_eq!(d_tags, vec!["stream-001"]);
    }

    #[test]
    fn stream_update_structure() {
        let kp = test_keypair();
        let event = SignalingBuilder::stream_update(
            "stream-001",
            r#"{"status": "live"}"#,
            &kp,
        )
        .unwrap();

        assert_eq!(event.kind, kind::STREAM_UPDATE);
    }

    #[test]
    fn stream_end_structure() {
        let kp = test_keypair();
        let event =
            SignalingBuilder::stream_end("stream-001", r#"{"reason": "complete"}"#, &kp)
                .unwrap();

        assert_eq!(event.kind, kind::STREAM_END);
    }

    #[test]
    fn parse_session_id_from_event() {
        let kp = test_keypair();
        let event = SignalingBuilder::offer(
            "sess-xyz",
            &["cpub_alice"],
            "{}",
            &kp,
        )
        .unwrap();

        let session_id = SignalingBuilder::parse_session_id(&event);
        assert_eq!(session_id.as_deref(), Some("sess-xyz"));
    }

    #[test]
    fn ice_candidate_event_structure() {
        let kp = test_keypair();
        let event = SignalingBuilder::ice_candidate(
            "sess-001",
            "cpub_bob",
            r#"{"candidate": "a]0..."}"#,
            &kp,
        )
        .unwrap();

        assert_eq!(event.kind, kind::ICE_CANDIDATE);
        let session_ids = event.tag_values("session");
        assert_eq!(session_ids, vec!["sess-001"]);
        let p_tags = event.tag_values("p");
        assert_eq!(p_tags, vec!["cpub_bob"]);
    }

    #[test]
    fn stream_recording_structure() {
        let kp = test_keypair();
        let event = SignalingBuilder::stream_recording(
            "stream-001",
            "abc123deadbeef",
            r#"{"duration_secs": 3600, "format": "opus"}"#,
            &kp,
        )
        .unwrap();

        assert_eq!(event.kind, kind::STREAM_RECORDING);
        let session_ids = event.tag_values("session");
        assert_eq!(session_ids, vec!["stream-001"]);
        let d_tags = event.tag_values("d");
        assert_eq!(d_tags, vec!["abc123deadbeef"]);
        assert!(event.content.contains("duration_secs"));
    }

    #[test]
    fn all_signaling_events_have_valid_signatures() {
        let kp = test_keypair();

        let events = vec![
            SignalingBuilder::offer("s1", &["a"], "{}", &kp).unwrap(),
            SignalingBuilder::answer("s1", "a", "{}", &kp).unwrap(),
            SignalingBuilder::end("s1", "{}", &kp).unwrap(),
            SignalingBuilder::ice_candidate("s1", "a", "{}", &kp).unwrap(),
            SignalingBuilder::stream_announce("s2", "{}", &kp).unwrap(),
            SignalingBuilder::stream_update("s2", "{}", &kp).unwrap(),
            SignalingBuilder::stream_end("s2", "{}", &kp).unwrap(),
            SignalingBuilder::stream_recording("s2", "abc123hash", "{}", &kp).unwrap(),
        ];

        for event in &events {
            assert!(
                EventBuilder::verify(event).unwrap(),
                "signature invalid for kind {}",
                event.kind
            );
        }
    }
}
