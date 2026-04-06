# World/Physical — The Land

The physical bridge. World connects the digital civilization to the physical one — geographic presence, local communities, proximity verification, and Cash distribution. Sovereignty extends to the physical world.

## Architecture

Rust crate (`physical`) with eight domain modules plus event builders. No dependency on Globe directly — the builder module produces tag arrays and JSON content that Globe's `EventBuilder` and `OmniFilter` expect.

```
Physical/
├── Cargo.toml       ← depends on x, crown, bulwark, serde, uuid, chrono, thiserror
└── src/
    ├── lib.rs           ← module declarations + re-exports
    ├── error.rs         ← PhysicalError (18 variants via thiserror)
    ├── place.rs         ← Place, PlaceType, PlaceVisibility
    ├── region.rs        ← Region, RegionType, RegionBoundary, RegionDeclaration
    ├── rendezvous.rs    ← Rendezvous, RendezvousPurpose, Rsvp, RsvpResponse, RendezvousStatus
    ├── presence.rs      ← PresenceSignal, PresenceStatus, ProximityLevel, PresenceAudience, PresenceConfig
    ├── lantern.rs       ← LanternShare, LanternSos, LanternConfig, LanternAudience, LanternPurpose
    ├── handoff.rs       ← Handoff, HandoffPurpose, HandoffItem, HandoffItemType, HandoffStatus, ProximityProofRef
    ├── omnitag.rs       ← OmniTagIdentity, TagSighting, TagStream, TagStreamEntry, TrackerType
    ├── caravan.rs       ← Delivery, DeliveryStatus, DeliveryItem, DeliveryLeg, TrackerAttachment
    └── builder.rs       ← Globe event kind constants (23000-24000) + tag/content builder functions + PresenceBroadcast
```

## Key Types

### Place — Physical locations
- `Place` — Named geographic point with owner, visibility controls, and region association. Builder pattern. Owner-only mutation.
- `PlaceType` — Cafe, Park, CoOp, CommunityHub, Library, Market, Residence, Workshop, Garden, Custom(String).
- `PlaceVisibility` — Private (default), Shared(Vec crown IDs), Community(String), Public. Checked via `is_visible_to()`.

### Region — Geographic areas with nesting
- `Region` — Named area with boundary, nesting via `parent_id`. Circle, Polygon, or Named boundary types.
- `RegionBoundary` — Circle (center + radius), Polygon (vertices), Named (no geometry). `contains()` delegates to X's `GeoCoordinate::is_within` and `point_in_polygon`.
- `RegionDeclaration` — Pull-based: person declares their regions. The network does not track them.

### Rendezvous — Meetup coordination
- `Rendezvous` — Scheduled meetup with organizer, invitees, RSVPs, lifecycle (Proposed -> Confirmed -> InProgress -> Completed/Cancelled). Encrypted to participants.
- Organizer controls: reschedule, cancel, confirm, complete. RSVP replaces previous response.

### Presence — Ephemeral nearby signals
- `PresenceSignal` — **Intentionally NOT Serialize/Deserialize.** Never persisted, logged, or stored. Ghost mode (Nobody audience) is the default. TTL with extension limits.
- `ProximityLevel` — SameArea (~10km), Nearby (~1km), Here (~100m). Intentionally imprecise.
- `PresenceAudience` — Nobody, Selected(Vec crown IDs), Community(String), Trusted.
- `PresenceConfig` — Default 5min TTL, 1hr max. Default audience: Nobody. Default proximity: SameArea.

### Lantern — Voluntary precise location sharing
- `LanternShare` — Time-limited location beacon with audience, purpose, and optional Yoke recording. Live position updates. Extinguish/extend lifecycle.
- `LanternSos` — Emergency beacon. Bypasses audience controls, goes to designated emergency contacts. Active until explicitly resolved.
- `LanternConfig` — Default 30min TTL, 24hr max.

### Handoff — Physical exchange protocol
- `Handoff` — Dual-signed exchange. State machine: Initiated -> ProximityVerified -> InitiatorSigned -> FullySigned -> Completed (or Cancelled/Disputed).
- `ProximityProofRef` — Privacy-stripped reference to Bulwark's `ProximityProof`. Stores method (Ble/Nfc/Ultrasonic/Multiple/None) and timestamp only. No RSSI, NFC tokens, or ultrasonic data retained.
- `HandoffItem` — Items with type (CashNote, PhysicalGood, Document, VerificationToken, Custom) and optional reference ID.
- `signable_bytes()` — Deterministic JSON for signing (excludes signatures and status).

### OmniTag — Decentralized tracker
- `OmniTagIdentity` — Crown keypair on chip. Owner + tag pubkey. Activate/deactivate lifecycle.
- `TagSighting` — Encrypted location (opaque ciphertext via Sentinal). Sighter crown_id + optional signal strength.
- `TagStream` — Capacity-managed (default 100) FIFO history of decrypted locations. Authorized viewers + owner.

### Caravan — Delivery orchestration
- `Delivery` — State machine: Created -> CourierAssigned -> PickedUp -> InTransit -> NearDestination -> Delivered -> Confirmed. Also Cancelled, Disputed. Dual Handoffs for chain of custody. Cash on delivery. Tracker attachment.
- Can skip NearDestination (deliver directly from InTransit). Only recipient can confirm.

### Builder — Globe event helpers
- 17 kind constants (23000-24000): PLACE, PLACE_UPDATE, REGION, REGION_DECLARATION, RENDEZVOUS, RENDEZVOUS_RSVP, RENDEZVOUS_UPDATE, HANDOFF, HANDOFF_SIGNATURE, LANTERN_SHARE, LANTERN_SOS, PRESENCE_LOCAL, PRESENCE_RELAY, CARAVAN, CARAVAN_UPDATE, OMNITAG_SIGHTING, OMNITAG_REGISTRATION.
- Tag builder and content serializer functions for each type.
- `PresenceBroadcast` — Relay-safe stripped version of PresenceSignal. NO coordinates. Only status, proximity, message.

## Dependencies

- `x` — GeoCoordinate, point_in_polygon, Haversine distance
- `crown` — Keypair identity (used via crown_id strings)
- `bulwark` — ProximityProof (for Handoff's ProximityProofRef conversion)
- `serde`, `serde_json`, `thiserror`, `uuid`, `chrono`, `log`

## Covenant Alignment

**Sovereignty** — location sharing is always opt-in and granular. Region is pull-based (you declare where you are). Lantern is self-lit. **Dignity** — physical access to the economy (via Cash/Handoff) ensures sovereignty isn't limited to device owners. **Consent** — every location feature requires explicit, revocable permission. Presence is never persisted. Handoff strips proximity evidence. PresenceSignal's lack of Serialize is architectural, not accidental.
