# Crown -- Identity

The royal identity. Crown is who you are in Omninet -- your profile, your keys, your social graph. Your identity is cryptographic (secp256k1 BIP-340 Schnorr), not platform-granted. No one can revoke it.

## Architecture

```
Crown (identity types + crypto)
    |-- CrownKeypair: secp256k1 BIP-340 Schnorr keypairs
    |   |-- generate() -> random keypair
    |   |-- from_crown_secret() -> import from bech32
    |   |-- from_private_key() -> import from raw bytes
    |   |-- from_crown_id() -> public-only (verify, no sign)
    |   |-- shared_secret() -> ECDH (secp256k1 scalar multiply + SHA-256)
    |   +-- public_key_hex() -> 64-char hex string
    |-- Signature: 64-byte Schnorr sign/verify
    |   |-- sign(data, keypair) -> SHA-256 -> BIP-340
    |   +-- verify(data, pubkey) / verify_crown_id() / verify_signer()
    |-- Keyring: primary + named personas + rotation chain
    |   |-- generate_primary() / import_primary()
    |   |-- create_persona() / import_persona() / delete_persona()
    |   |-- list_personas() / has_persona()
    |   |-- sign() / sign_as(persona) / verify()
    |   |-- export() -> JSON bytes / load(bytes)
    |   |-- export_primary_crown_secret() / export_persona_crown_secret()
    |   |-- rotate_primary() -> RotationAnnouncement (signed by OLD key)
    |   |-- rotation_chain() / rotation_chain_mut()
    |   |-- primary_keypair() -> Option<&CrownKeypair>
    |   |-- export_primary_secret() -> raw 32-byte private key
    |   |-- setup_social_recovery(config, sharer) -> Vec<KeyShare>
    |   |-- setup_encrypted_backup(encryptor, password) -> EncryptedKeyringBackup
    |   +-- lock() -> clears all keys + rotation chain from memory
    |-- Rotation: key rotation chain of custody
    |   |-- PreviousKey (pubkey hex, crown_id, rotated_at, signature)
    |   |-- RotationAnnouncement (old/new keys, signature by old key, timestamp)
    |   |-- RotationChain (push, latest_rotation, verify_chain, len, is_empty)
    |   +-- verify_rotation(old_pubkey_hex, announcement) -> bool
    |-- Recovery: account recovery (traits + types)
    |   |-- RecoveryEncryptor trait (encrypt, decrypt, derive_key_from_password/seed)
    |   |-- SecretSharer trait (split, reconstruct)
    |   |-- RecoveryMethod / RecoveryConfig / RecoveryArtifacts
    |   |-- SocialRecoveryConfig / KeyShare / EncryptedKeyringBackup
    |   |-- recover_from_secret(bytes) -> Keyring
    |   |-- recover_from_shares(shares, sharer) -> Keyring
    |   +-- recover_from_backup(backup, encryptor, password) -> Keyring
    |-- DeviceSync: credential sync between devices
    |   |-- SyncOffer (device, crown_id, nonce, expiry)
    |   |-- SyncAccept (nonce echo, signature, device)
    |   |-- SyncPayload (encrypted keyring, from/to crown_id)
    |   |-- SyncStatus (OfferSent/AcceptReceived/PayloadSent/Complete/Failed/Expired)
    |   |-- create_sync_offer(device_name, keyring) -> SyncOffer
    |   |-- verify_sync_accept(offer, accept) -> bool
    |   |-- prepare_sync_payload(keyring, recipient_hex, encryptor) -> SyncPayload
    |   +-- receive_sync_payload(payload, local_keyring, encryptor) -> Keyring
    |-- Soul: container for identity data
    |   |-- SoulEncryptor trait (encrypt, decrypt) — at-rest encryption
    |   |-- Profile (display_name, username, bio, avatar, banner, etc.)
    |   |-- Preferences (theme, language, privacy, notifications)
    |   |-- SocialGraph (following/followers/blocked/muted/trusted/lists)
    |   +-- Persistence: soul.json (encrypted via SoulEncryptor, plaintext fallback)
    |-- VerificationLevel: 0-4 (is_verified = level >= 1)
    +-- Founding Verification Tree (R2C)
        |-- FoundingTree: rooted at Crown #1, append-only
        |-- VerificationLineage: pubkey, verified_by, depth, branch_path
        |-- TreeAnomaly: 5 categories (timing, verifier overlap, behavioral, geographic, rapid branching)
        |-- AnomalyAlert: anomaly + confidence (0.0–1.0)
        +-- AnomalyThresholds: timing window, rapid branch limits
```

## Key Types

- **CrownKeypair** -- secp256k1 BIP-340 Schnorr. 32-byte x-only public key + optional 32-byte private key. Bech32 crown_id/crown_secret encoding (bech32). No Serialize/Deserialize -- private keys must not be accidentally serialized. Equality/Hash based on crown_id only. Debug redacts private key. Includes `shared_secret()` for secp256k1 ECDH: scalar multiplication of their public key by our secret key, x-only coordinate extraction (parity-independent), SHA-256 domain separation. Used by Lingo for shared Babel vocabularies.
- **Signature** -- 64-byte Schnorr signature (hex-encoded in JSON via custom serde), signer crown_id, timestamp. Data is SHA-256 hashed before signing.
- **Keyring** -- Primary keypair + named persona keypairs (HashMap) + RotationChain. Export as hex-encoded JSON (includes rotation chain). Lock clears memory + rotation chain. No double-generate (PersonaAlreadyExists). `verify()` delegates to Signature. `export_primary_crown_secret()` / `export_persona_crown_secret()` for direct crown_secret access. `rotate_primary()` generates new keypair, signs announcement with OLD key, records old key in chain. `export_primary_secret()` for raw private key export (recovery). `setup_social_recovery()` splits key via SecretSharer trait. `setup_encrypted_backup()` encrypts keyring via RecoveryEncryptor trait.
- **Profile** -- display_name, username, bio, avatar (AvatarReference), banner (AvatarReference), website, language (BCP 47), lightning_address, nip05, updated_at.
- **AvatarReference** -- Tagged enum with custom serde: Data (bytes + mime_type), Asset (idea_id + asset_name), Url(String). JSON format: `{"type": "url", "url": "..."}`.
- **Preferences** -- theme (System/Light/Dark/Cosmic), text_scale, reduce_motion, content_language, interface_language, auto_translate, default_visibility (Private/Collective/Public), show_online_status, send_read_receipts, push_enabled, notification_categories (Mentions/Replies/Endorsements/Transfers/CollectiveActivity/SystemUpdates).
- **SocialGraph** -- following/followers/blocked/muted/trusted (all HashSet<String> of crown IDs), lists (HashMap<String, HashSet<String>>). **Block auto-removes from following** (invariant). Queries: `is_following`, `is_blocked`, `is_muted`, `is_trusted`, `users_in_list`, `list_names` (sorted).
- **SoulEncryptor** -- Trait: encrypt(data) -> Vec<u8>, decrypt(data) -> Vec<u8>. Implemented by caller (Sentinal via Divinity FFI adapter). Send + Sync. Crown defines but does not implement — zero-dep on Sentinal.
- **Soul** -- Container holding Profile + Preferences + SocialGraph + optional SoulEncryptor. Persists to `{path}/soul.json` via SoulStorage (versioned JSON, version 1). When encryptor is present, data is AES-256-GCM encrypted before writing. Backward compatible: `load()` with encryptor falls back to plaintext JSON if decryption fails (migration path). Dirty tracking (mutations mark dirty, save clears). Social shortcuts: `follow()`, `unfollow()`, `block()`, `unblock()`. `create(path, encryptor)` and `load(path, encryptor)` accept optional encryptor. `set_encryptor()` / `has_encryptor()` for post-construction attachment.
- **PreviousKey** -- Record of a rotated-out primary key: public_key_hex, crown_id, rotated_at, rotation_signature (BIP-340 by old key).
- **RotationAnnouncement** -- Signed announcement of key rotation: old/new pubkey hex + crown_id, BIP-340 signature by OLD key, timestamp, optional reason. `to_signable_bytes()` produces deterministic bytes (old_hex + new_hex + RFC3339 timestamp). `verify()` self-validates.
- **RotationChain** -- Ordered list of PreviousKey records. `verify_chain()` validates each transition signature. Default = empty.
- **RecoveryEncryptor** -- Trait: encrypt/decrypt + derive_key_from_password/seed. Implemented by caller (Sentinal). Send + Sync.
- **SecretSharer** -- Trait: split(secret, threshold, shares) + reconstruct(shares). Implemented by caller (Shamir/sharks). Send + Sync.
- **RecoveryMethod** -- Enum: SeedPhrase, SocialRecovery, EncryptedBackup. Serde.
- **SocialRecoveryConfig** -- threshold (u8) + trustees (Vec<String> crown IDs). Serde.
- **KeyShare** -- trustee_crown_id + encrypted_share (raw bytes) + share_index. Serde.
- **EncryptedKeyringBackup** -- ciphertext + created_at. Serde.
- **RecoveryConfig** -- Enum: SeedPhrase, Social(SocialRecoveryConfig), Backup. Serde.
- **SyncOffer** -- from_device + from_crown_id + nonce (hex) + expires_at (5 min). Serde.
- **SyncAccept** -- echoed nonce + responder_crown_id + BIP-340 signature + device_name. Serde.
- **SyncPayload** -- encrypted_keyring + from/to crown_id + timestamp. Serde.
- **SyncStatus** -- Enum: OfferSent, AcceptReceived, PayloadSent, Complete, Failed(String), Expired. Serde + PartialEq.
- **CrownError** -- 29 variants: Locked, NoIdentity, SoulCorrupted, PersonaNotFound, PersonaAlreadyExists, CannotDeletePrimary, InvalidCrownSecret, InvalidCrownId, InvalidPrivateKey, SignatureFailed, VerificationFailed, RotationFailed, NoPrimaryKey, RecoveryFailed, InsufficientShares, DecryptionFailed, SyncFailed, SyncExpired, InvalidSyncResponse, ProfileUpdateFailed, InvalidProfileData, LoadFailed, SaveFailed, BlindingFailed, InvalidBlindingContext, BlindedKeyNotFound, BlindingProofFailed, Io (From), Serialization (From). Send + Sync.
- **VerificationLevel** -- Struct wrapping u8, clamped 0-4. Constants: NONE(0), BASIC(1), STANDARD(2), ENHANCED(3), VERIFIED(4). `is_verified() = level >= 1`. Implements Ord for comparison.

## Dependencies

```toml
secp256k1 = { version = "0.29", features = ["rand-std", "serde", "global-context"] }
bech32 = "0.11"
rand = "0.8"
sha2 = "0.10"
serde, serde_json, thiserror, uuid, chrono, log, hex, base64
```

**Zero internal Omninet deps.** Crown stands alone. Keyring.export() produces raw JSON bytes; the caller encrypts (Sentinal) and persists (Vault).

## Account Security (Phase 1D)

Three modules for account security. Crown defines types and orchestration; actual crypto is injected via traits.

- **rotation.rs** -- Key rotation with chain of custody. Old key signs announcement proving it authorized the transition. `RotationChain` records all previous keys. `Keyring::rotate_primary()` is the entry point. Chain is persisted in KeyringStorage (export/load) and cleared on lock().
- **recovery.rs** -- Account recovery via three methods: seed phrase (export raw private key, caller encodes via BIP-39), social recovery (threshold secret sharing via SecretSharer trait), encrypted backup (password-based via RecoveryEncryptor trait). Static recovery functions: `recover_from_secret()`, `recover_from_shares()`, `recover_from_backup()`.
- **device_sync.rs** -- Credential sync between devices via offer/accept/payload protocol. Uses ECDH (Crown's `shared_secret()`) for key agreement + RecoveryEncryptor trait for symmetric encryption. SyncOffer expires in 5 minutes. SyncAccept must include BIP-340 signature of the nonce.

**Design principle:** Crown has ZERO internal Omninet deps. Crypto traits (RecoveryEncryptor, SecretSharer) let the caller inject Sentinal implementations without Crown depending on Sentinal.

## Founding Verification Tree (`founding_tree.rs`) — R2C

Every Crown identity on Omninet traces back through a chain of physical verifications to Crown #1 (the founding identity). The chain is public and forms a tree. Depth indicates generation, NOT trust level -- depth 50 is as valid as depth 2.

### Key Types
- **VerificationLineage** -- A single link: pubkey, verified_by (None for root), verified_at, proximity_proof hash, depth, branch_path (full path back to Crown #1).
- **FoundingTree** -- The complete tree: root_pubkey, total_verified, max_depth, tree (HashMap<String, VerificationLineage>), excluded (HashMap<String, DateTime<Utc>>). Append-only -- verifications cannot be revoked.
  - `new(root_pubkey)` -- creates tree with Crown #1 at depth 0.
  - `verify(pubkey, verified_by, proximity_proof)` -- adds identity via physical verification. Errors if verifier not in tree, pubkey already verified, or pubkey empty.
  - `verify_at(...)` -- same but with explicit timestamp (testing/replay).
  - `lineage(pubkey)` -- lookup verification lineage.
  - `common_ancestor(a, b)` -- most recent common ancestor via branch path comparison.
  - `siblings(pubkey)` -- others verified by the same verifier.
  - `subtree(pubkey)` -- recursive collection of all verifiees down the tree.
  - `record_exclusion(pubkey)` / `record_exclusion_at(...)` -- marks identity as excluded (for anomaly detection timing).
  - `anomaly_check(pubkey)` / `anomaly_check_with_thresholds(pubkey, thresholds)` -- checks for suspicious patterns.

### Anomaly Detection
- **TreeAnomaly** -- 5 categories:
  - `TimingCorrelation` -- new identity appeared shortly after an exclusion (gap_seconds). Confidence inversely proportional to gap.
  - `VerifierOverlap` -- new identity's verification chain shares verifiers with an excluded identity (shared_verifiers). Confidence proportional to overlap.
  - `BehavioralSimilarity` -- placeholder for external comparison via Yoke's behavioral baseline (similarity_score 0.0--1.0).
  - `GeographicProximity` -- same geographic region as excluded identity (region string).
  - `RapidBranching` -- one verifier verified unusually many identities in a short window (verifier_pubkey, count, window_seconds).
- **AnomalyAlert** -- anomaly, new_pubkey, related_excluded_pubkey, confidence (0.0--1.0), detected_at.
- **AnomalyThresholds** -- Tunable: timing_window_seconds (default 30 days), rapid_branch_max (default 10), rapid_branch_window_seconds (default 7 days).

### Integration
- **KidsSphere (R2B):** Parents can view a person's VerificationLineage when making approval decisions.
- **Identity rebirth detection:** When an Immutable Exclusion is active, `anomaly_check()` runs on new identities near the excluded identity's verifiers.

## What Does NOT Live Here

- **Secure Enclave keypairs** -> Divinity/Apple overlay (LAContext + Keychain)
- **TranslationKit / Babel** -> Lingo (L)
- **Pact Phone/Email handlers** -> Equipment integration, deferred
- **Key encryption** -> Sentinal handles this. Keyring.export() produces raw JSON
- **Key storage** -> Vault handles persistence. Soul writes plaintext JSON
- **Verification mechanics** (face verification, trust BFS) -> Jail (J)
- **encrypted DMs** -> Globe (G), Globe-specific
- **address verification** -> Globe (G), network operation

## Covenant Alignment

**Sovereignty** -- your keypair is yours. No server, no corporation, no government has a copy. Export produces bytes you control. Lock clears memory. Key rotation preserves sovereignty -- old key signs the transition, not a server. Recovery is self-sovereign: seed phrases, trusted contacts (social recovery), or encrypted backups -- never platform-mediated. Device sync uses ECDH -- no intermediary sees the plaintext. **Dignity** -- profiles express who you choose to be. Verification levels reflect earned trust, not gatekeeping. **Consent** -- every social graph connection is explicit and revocable. Block is instant and absolute. Social recovery requires explicit trustee designation. Device sync requires signed acceptance.
