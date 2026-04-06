//! Discovery — finding people, communities, and devices on the Omnidea network.
//!
//! This module covers all the ways an Omnidea device finds things: local network
//! peers via mDNS, community beacons propagated through gospel, device pairing
//! via QR codes, invitation-based onboarding, and the Network Key that encrypts
//! relay addresses for privacy.

pub mod address;
pub mod beacon;
pub mod invitation;
pub mod local;
pub mod network_key;
pub mod pairing;
pub mod profile;
#[cfg(feature = "upnp")]
pub mod upnp;
