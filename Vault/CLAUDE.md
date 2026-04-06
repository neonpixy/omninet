# Vault -- Encrypted Storage

The locked treasury. Vault manages encrypted persistence -- it knows where every `.idea` lives, manages the key lifecycle, and owns the manifest database. When locked, nothing is accessible. When unlocked, keys exist only in memory.

## Architecture

```
Password + Salt
    | PBKDF2 (600K iterations, via Sentinal)
    v
Master Key (SecureData, memory-only)
    |-- HKDF(info="content-{ideaId}") -> Content Key (per .idea)
    |-- HKDF(info="content-{manifestKeyId}") -> Manifest Key
    |-- HKDF(info="vocabulary-seed") -> Vocabulary Seed (for Babel)
    |-- HKDF(salt="omnidea-soul-v1") -> Soul Key (for Crown soul.json at-rest encryption)
    +-- Collective keys (received externally, cached)
```

### Source Layout

```
Vault/src/
  lib.rs          -- Vault facade (pub struct), module declarations, re-exports
  error.rs        -- VaultError enum
  config.rs       -- VaultConfig (JSON persistence to .vault/config.json)
  state.rs        -- VaultState (lock/unlock state machine + path resolution)
  custodian.rs    -- Custodian (key lifecycle via Sentinal)
  manifest.rs     -- Manifest (rusqlite/SQLCipher + HashMap cache)
  entry.rs        -- ManifestEntry + IdeaFilter (builder pattern)
  collective.rs   -- Collective, CollectiveMember, CollectiveRole
  module_state.rs -- ModuleState methods on Manifest (private module)
  search.rs       -- Full-text search over vault notes via SQLite FTS5
```

### Key Types

- **Vault** -- Top-level facade. Owns VaultState, Custodian, Manifest, and collectives HashMap. All operations guard on `is_unlocked()`.
- **VaultState** -- Lock/unlock state machine. Holds `root_path: Option<PathBuf>`. Provides path resolution (vault_dir, config_path, manifest_path, personal_path, collectives_path, resolve_path, relative_path).
- **Custodian** -- Key lifecycle manager. Derives master key via PBKDF2, content keys via HKDF, manifest key, vocabulary seed. Caches content keys and collective keys in memory. All keys stored as `SecureData` (zeroed on drop).
- **VaultConfig** -- Persisted as cleartext JSON at `.vault/config.json`. Fields: version, created_at, last_unlocked, owner_public_key, salt, manifest_key_id. Atomic writes (temp file + rename).
- **Manifest** -- SQLCipher-encrypted database with in-memory HashMap cache. Dual-layer: all reads from cache (O(1) by ID or path), all writes go to DB first then update cache. Supports bulk_upsert with transactions.
- **ManifestEntry** -- A row in the manifest: id, path, title, extended_type, creator, created_at, modified_at, collective_id, header_cache. Has `from_header()` constructor.
- **IdeaFilter** -- Builder-pattern filter: creator, collective_id, extended_type, modified_after, modified_before, path_prefix, title_contains (case-insensitive). AND logic.
- **Collective** -- Shared space: id, name, created_at, members, our_role. Methods: create, add_member (requires Admin), remove_member (requires Owner), is_member, member_role.
- **CollectiveRole** -- Readonly(1) < Member(2) < Admin(3) < Owner(4). Derives Ord for permission checking.
- **CollectiveMember** -- public_key, joined_at, role.

### Lock/Unlock State Machine

**Unlock sequence:**
1. Guard not already unlocked
2. Set root path, create `.vault/` directory
3. Load or create `config.json` (salt, manifest_key_id)
4. Derive master key via PBKDF2 (Sentinal)
5. Derive manifest key via HKDF (Sentinal, using manifest_key_id)
6. Open SQLCipher database with manifest key (raw hex mode)
7. Load in-memory cache from database
8. Load persisted collectives from module_state

**Lock sequence:**
1. Persist collectives to module_state
2. Zero all keys (SecureData drops)
3. Close + clear manifest
4. Clear collectives
5. Mark state as locked

### Manifest Database (SQLCipher)

SQLCipher = AES-256 encrypted SQLite. Key set via `PRAGMA key = "x'<hex>'"` (raw key mode, bypasses SQLCipher's internal PBKDF2).

```sql
CREATE TABLE manifest (
    id TEXT PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    title TEXT,
    extended_type TEXT,
    creator TEXT NOT NULL,
    created_at TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    collective_id TEXT,
    header_cache TEXT
);
-- Indexes: path, extended_type, creator, modified_at

CREATE TABLE module_state (
    module_id TEXT NOT NULL,
    state_key TEXT NOT NULL,
    data TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (module_id, state_key)
);
```

### Public API (Vault facade)

- `new()`, `is_unlocked()`, `unlock(password, root_path)`, `lock()`
- Manifest: `register_idea`, `unregister_idea`, `get_idea`, `get_idea_by_path`, `list_ideas`, `list_ideas_in_folder`, `idea_count`
- Encryption: `encrypt_for_idea`, `decrypt_for_idea`, `content_key`, `vocabulary_seed`, `soul_key`
- Collectives: `create_collective`, `join_collective`, `leave_collective`, `list_collectives`, `collective_key`
- Module state: `save_module_state`, `load_module_state`, `delete_module_state`, `list_module_state_keys`
- Path resolution: `root_path`, `personal_path`, `collectives_path`, `resolve_path`

### Directory Layout

```
{vault_root}/
  .vault/
    config.json     -- VaultConfig (salt, manifest_key_id, version)
    manifest.db     -- SQLCipher encrypted database
  Personal/           -- Personal ideas
  Collectives/        -- Shared collective ideas
```

## Dependencies

```toml
sentinal = { path = "../Sentinal" }        # Encryption primitives (PBKDF2, HKDF, AES-GCM, SecureData)
ideas = { path = "../Ideas" }              # Header type for manifest cache
rusqlite = { version = "0.38", features = ["bundled-sqlcipher"] }
serde, serde_json, chrono, uuid, hex, thiserror, log
```

Zero dependency on X or Equipment at the library layer.

## What Does NOT Live Here

- **Biometric gate** -- Divinity/Apple (LAContext, Secure Enclave)
- **File watching / rescan** -- Hall (filesystem operations)
- **Reading/writing .idea files** -- Hall (Scribe/Scholar)
- **Gospel DM key distribution** -- Globe (NIP-04/NIP-44)
- **Idea re-encryption on rekey** -- Hall + Vault coordination
- **Pact API handlers** -- when Throne apps need them

## Covenant Alignment

**Sovereignty** -- you hold the password, you hold the keys. No platform can unlock what you've locked. **Consent** -- collectives require explicit membership; re-keying on removal ensures departed members lose access.
