# DeviceManager — Multi-Device Pairing & Fleet Management

Part of the Undercroft meta-layer. DeviceManager handles device pairing with real BIP-340 Schnorr signature verification, fleet tracking across paired devices, and sync coordination.

## Source Layout

```
Undercroft/DeviceManager/
├── Cargo.toml
└── src/
    ├── lib.rs          ← module declarations + re-exports
    ├── error.rs        ← DeviceManagerError (7 variants)
    ├── pairing.rs      ← PairingProtocol (initiate, respond, verify)
    ├── fleet.rs        ← DeviceFleet, FleetEntry, DeviceStatus, FleetHealth
    └── sync.rs         ← SyncPriority, SyncState, SyncTracker
```

## Architecture

```
DeviceManager
    ├── PairingProtocol (stateless)
    │   ├── initiate(keypair, name, relay_url) → PairingChallenge
    │   ├── respond(challenge, keypair, name) → PairingResponse
    │   └── verify(challenge, response) → DevicePair
    ├── DeviceFleet (HashMap<crown_id, FleetEntry>)
    │   ├── add/remove/get/get_mut/list
    │   ├── update_status()
    │   ├── health() → FleetHealth
    │   └── count/is_empty
    ├── SyncPriority (data_type → home device)
    │   ├── set_home/home_for/remove/all
    └── SyncTracker (device → data_type → SyncState)
        ├── set_state/get_state
        ├── states_for_device
        ├── all_synced
        └── conflicts
```

## Key Types

- **PairingProtocol** — Stateless. Wraps Globe's `PairingChallenge`/`PairingResponse`/`DevicePair` with real Crown BIP-340 Schnorr crypto. Challenge expires in 5 minutes.
- **DeviceFleet** — In-memory registry of paired devices, keyed by Crown crown_id. Tracks status (online/offline, battery, connection type). Persistence is the caller's responsibility (Vault).
- **FleetEntry** — Device record: crown_id, name, DevicePair, optional DeviceProfile, DeviceStatus, paired_at.
- **DeviceStatus** — Last seen, online flag, serving policy, connection type, battery percent.
- **FleetHealth** — Aggregate: total/online/offline counts, all_synced flag.
- **SyncPriority** — Maps data types to home devices (authoritative source for conflict resolution).
- **SyncState** — Enum: Synced, Pending, Conflict, Unknown.
- **SyncTracker** — Nested HashMap tracking sync state per device per data type.
- **DeviceManagerError** — 7 variants: PairingFailed, PairingExpired, NonceMismatch, SignatureInvalid, DeviceNotFound, AlreadyPaired, SyncConflict.

## Dependencies

```toml
crown = { path = "../../Crown" }    # BIP-340 signing/verification
globe = { path = "../../Globe" }    # PairingChallenge, PairingResponse, DevicePair, DeviceProfile
x = { path = "../../X" }            # VectorClock (available for future sync use)
serde, serde_json, chrono, thiserror, log, rand, hex
```

## Pairing Crypto Flow

1. Initiator generates 32-byte random nonce (hex-encoded).
2. Responder signs `nonce.as_bytes()` via `Crown::Signature::sign()` (SHA-256 + BIP-340 Schnorr).
3. Verifier decodes hex signature (128 chars → 64 bytes), constructs `Crown::Signature`, calls `verify_crown_id()`.
4. On success, constructs `DevicePair` with responder's crown_id as identity.

## What Does NOT Live Here

- **Actual sync transport** → Globe (ORP events carry sync data)
- **Key storage/encryption** → Vault + Sentinal
- **Device profile computation** → Globe's `DeviceProfile::compute_policy()`
- **Fleet persistence** → Vault (DeviceFleet serializes to JSON)
- **Network discovery** → Globe's mDNS/DNS-SD (LocalAdvertiser/LocalBrowser)

## Covenant Alignment

**Sovereignty** — pairing is peer-to-peer with no intermediary. Both devices prove identity via their own Crown keys. **Consent** — pairing requires active participation from both sides (challenge-response). **Dignity** — device names are self-declared, not externally assigned.
