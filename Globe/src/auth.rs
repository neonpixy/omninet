use crown::CrownKeypair;
use url::Url;

use crate::error::GlobeError;
use crate::event::OmniEvent;
use crate::event_builder::{EventBuilder, UnsignedEvent};
use crate::kind;

/// Create a signed authentication response for a relay challenge.
///
/// Produces a kind-22242 event with `["relay", url]` and `["challenge", challenge]`
/// tags, signed by the provided keypair.
pub fn create_auth_response(
    challenge: &str,
    relay_url: &Url,
    keypair: &CrownKeypair,
) -> Result<OmniEvent, GlobeError> {
    let unsigned = UnsignedEvent::new(kind::AUTH_EVENT, "")
        .with_tag("relay", &[relay_url.as_str()])
        .with_tag("challenge", &[challenge]);

    EventBuilder::sign(&unsigned, keypair)
}

/// Verify an authentication response event.
///
/// Checks:
/// 1. Event kind is 22242
/// 2. Has a `relay` tag matching the expected URL
/// 3. Has a `challenge` tag matching the expected challenge
/// 4. Signature is valid
pub fn verify_auth_response(
    event: &OmniEvent,
    expected_challenge: &str,
    relay_url: &Url,
) -> Result<bool, GlobeError> {
    if event.kind != kind::AUTH_EVENT {
        return Ok(false);
    }

    let relay_tag = event.tag_value("relay");
    if relay_tag != Some(relay_url.as_str()) {
        return Ok(false);
    }

    let challenge_tag = event.tag_value("challenge");
    if challenge_tag != Some(expected_challenge) {
        return Ok(false);
    }

    EventBuilder::verify(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_response_round_trip() {
        let kp = CrownKeypair::generate();
        let url = Url::parse("wss://relay.omnidea.co").unwrap();

        let event = create_auth_response("challenge123", &url, &kp).unwrap();
        assert_eq!(event.kind, kind::AUTH_EVENT);
        assert!(event.has_tag("relay", "wss://relay.omnidea.co/"));
        assert!(event.has_tag("challenge", "challenge123"));

        let valid = verify_auth_response(&event, "challenge123", &url).unwrap();
        assert!(valid);
    }

    #[test]
    fn wrong_challenge_fails() {
        let kp = CrownKeypair::generate();
        let url = Url::parse("wss://relay.omnidea.co").unwrap();

        let event = create_auth_response("real_challenge", &url, &kp).unwrap();
        let valid = verify_auth_response(&event, "wrong_challenge", &url).unwrap();
        assert!(!valid);
    }

    #[test]
    fn wrong_relay_fails() {
        let kp = CrownKeypair::generate();
        let url = Url::parse("wss://relay.omnidea.co").unwrap();
        let wrong_url = Url::parse("wss://evil.relay.co").unwrap();

        let event = create_auth_response("challenge", &url, &kp).unwrap();
        let valid = verify_auth_response(&event, "challenge", &wrong_url).unwrap();
        assert!(!valid);
    }

    #[test]
    fn wrong_kind_fails() {
        let kp = CrownKeypair::generate();
        let url = Url::parse("wss://relay.omnidea.co").unwrap();

        let mut event = create_auth_response("challenge", &url, &kp).unwrap();
        event.kind = 1; // Not an auth event.
        let valid = verify_auth_response(&event, "challenge", &url).unwrap();
        assert!(!valid);
    }

    #[test]
    fn auth_without_private_key_fails() {
        let kp = CrownKeypair::generate();
        let pubonly = CrownKeypair::from_crown_id(kp.crown_id()).unwrap();
        let url = Url::parse("wss://relay.omnidea.co").unwrap();

        let result = create_auth_response("challenge", &url, &pubonly);
        assert!(result.is_err());
    }

    #[test]
    fn auth_event_signature_verifies() {
        let kp = CrownKeypair::generate();
        let url = Url::parse("wss://relay.omnidea.co").unwrap();

        let event = create_auth_response("test", &url, &kp).unwrap();
        let sig_valid = EventBuilder::verify(&event).unwrap();
        assert!(sig_valid);
    }
}
